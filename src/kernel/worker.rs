use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use tokio::sync::mpsc;

use crate::{
    adapter::{ApiMessage, ChatRequestPayload, RequestMode},
    config::Config,
    crp,
    model::ModelClient,
};

use super::context_pack::{ContextHints, ContextPack, ContextPackBuilder};
use super::patch;
use super::search::{SearchParams, search};

// ---------------------------------------------------------------------------
// Stage types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageKind {
    Interpret,
    Locate,
    Hypothesize,
    Patch,
    Report,
}

impl StageKind {
    pub fn label(&self) -> &'static str {
        match self {
            StageKind::Interpret => "interpret",
            StageKind::Locate => "locate",
            StageKind::Hypothesize => "hypothesize",
            StageKind::Patch => "patch",
            StageKind::Report => "report",
        }
    }

    fn template_name(&self) -> &'static str {
        match self {
            StageKind::Patch => "code_edit",
            _ => "code_edit_minimal",
        }
    }
}

/// Typed output produced by each stage.
#[derive(Debug, Clone, Default)]
pub struct StageOutput {
    pub search_terms: Vec<String>,
    pub relevant_files: Vec<String>,
    pub approach: Option<String>,
    pub file_edits: Option<String>,
    pub final_response: Option<String>,
    pub raw_trace: String,
    /// False when the model responded in plain text instead of CRP format.
    pub crp_valid: bool,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum WorkerEvent {
    StageStarted { stage: String },
    StageCompleted { stage: String, crp_valid: bool },
    StageRetry { stage: String, attempt: u32, reason: String },
    StageFailed { stage: String, error: String },
    ContextPackReady { stage: String, chars_used: usize, budget: usize },
    WorkflowComplete { final_response: String },
    WorkflowFailed { error: String },
    PatchApprovalRequested {
        path: String,
        preview: String,
        response_tx: tokio::sync::oneshot::Sender<bool>,
    },
    PatchApplied { files_changed: Vec<String> },
    StageSkipped { stage: String, reason: String },
    /// Full prompt/response pair for diagnostics and fine-tuning dataset generation.
    StageTrace {
        stage: String,
        attempt: u32,
        system_prompt: String,
        user_message: String,
        model_response: String,
        crp_valid: bool,
        error: Option<String>,
        finish_reason: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Worker loop
// ---------------------------------------------------------------------------

pub struct WorkerLoop {
    workspace: PathBuf,
    config: Config,
    task: String,
    event_tx: mpsc::UnboundedSender<WorkerEvent>,
    /// Override the default context pack budget (chars). None = use DEFAULT_CODE_BUDGET.
    budget: Option<usize>,
}

impl WorkerLoop {
    pub fn new(
        workspace: &Path,
        config: Config,
        task: impl Into<String>,
        event_tx: mpsc::UnboundedSender<WorkerEvent>,
    ) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            config,
            task: task.into(),
            event_tx,
            budget: None,
        }
    }

    pub fn with_budget(mut self, chars: usize) -> Self {
        self.budget = Some(chars);
        self
    }

    /// Run the full bugfix/feature pipeline.
    pub async fn run_bugfix(self) -> Result<String> {
        let task = self.task.clone();

        // ── Stage 1: INTERPRET ──────────────────────────────────────────────
        // Minimal pack — no code context yet, just the task.
        let interpret_pack = self.make_pack(&task, &ContextHints::default());
        self.emit_pack(&interpret_pack, StageKind::Interpret.label());

        self.emit(WorkerEvent::StageStarted {
            stage: StageKind::Interpret.label().into(),
        });
        let interpret_out = self
            .run_stage(StageKind::Interpret, &task, &interpret_pack)
            .await?;
        self.emit(WorkerEvent::StageCompleted {
            stage: StageKind::Interpret.label().into(),
            crp_valid: interpret_out.crp_valid,
        });

        // ── Pipeline: load index concurrently while we continue ─────────────
        // The index load is I/O — we spawn it so stage 2's pack is ready faster.
        let ws = self.workspace.clone();
        let index_task =
            tokio::task::spawn_blocking(move || ContextPackBuilder::new(&ws).with_index());

        // ── Stage 2: LOCATE ─────────────────────────────────────────────────
        let builder = index_task
            .await
            .unwrap_or_else(|_| ContextPackBuilder::new(&self.workspace).with_index());

        let locate_hints = ContextHints {
            search_terms: interpret_out.search_terms.clone(),
            ..Default::default()
        };
        let locate_pack = builder.build(&task, &locate_hints)?;
        self.emit_pack(&locate_pack, StageKind::Locate.label());

        self.emit(WorkerEvent::StageStarted {
            stage: StageKind::Locate.label().into(),
        });
        let mut locate_out = match self
            .run_stage(StageKind::Locate, &task, &locate_pack)
            .await
        {
            Ok(out) => {
                self.emit(WorkerEvent::StageCompleted {
                    stage: StageKind::Locate.label().into(),
                    crp_valid: out.crp_valid,
                });
                out
            }
            Err(e) => {
                // Locate failed — skip it and let hypothesize use interpret's
                // search terms to find code context via the context pack.
                self.emit(WorkerEvent::StageSkipped {
                    stage: StageKind::Locate.label().into(),
                    reason: e.to_string(),
                });
                StageOutput::default()
            }
        };
        if locate_out.relevant_files.is_empty() {
            locate_out.relevant_files =
                file_hints_from_search_terms(&self.workspace, &interpret_out.search_terms);
        }

        // ── Pipeline: reload index for symbol/code reads ─────────────────────
        let ws = self.workspace.clone();
        let sym_task =
            tokio::task::spawn_blocking(move || ContextPackBuilder::new(&ws).with_index());

        // ── Stage 3: HYPOTHESIZE ─────────────────────────────────────────────
        let builder = sym_task
            .await
            .unwrap_or_else(|_| ContextPackBuilder::new(&self.workspace).with_index());

        let hyp_hints = ContextHints {
            files: locate_out.relevant_files.clone(),
            search_terms: interpret_out.search_terms.clone(),
            ..Default::default()
        };
        let hyp_pack = builder.build(&task, &hyp_hints)?;
        self.emit_pack(&hyp_pack, StageKind::Hypothesize.label());

        self.emit(WorkerEvent::StageStarted {
            stage: StageKind::Hypothesize.label().into(),
        });
        let hyp_out = self
            .run_stage(StageKind::Hypothesize, &task, &hyp_pack)
            .await?;
        self.emit(WorkerEvent::StageCompleted {
            stage: StageKind::Hypothesize.label().into(),
            crp_valid: hyp_out.crp_valid,
        });

        // ── Stage 4: PATCH ───────────────────────────────────────────────────
        // Enrich the task description with the hypothesis for the patch stage.
        let patch_task = match &hyp_out.approach {
            Some(a) => format!("{task}\n\nApproach:\n{a}"),
            None => task.clone(),
        };
        let patch_hints = ContextHints {
            files: locate_out.relevant_files.clone(),
            ..Default::default()
        };
        let patch_pack = self.builder()
            .build(&patch_task, &patch_hints)?;
        self.emit_pack(&patch_pack, StageKind::Patch.label());

        self.emit(WorkerEvent::StageStarted {
            stage: StageKind::Patch.label().into(),
        });
        let patch_out = self
            .run_stage(StageKind::Patch, &patch_task, &patch_pack)
            .await?;
        self.emit(WorkerEvent::StageCompleted {
            stage: StageKind::Patch.label().into(),
            crp_valid: patch_out.crp_valid,
        });

        // ── Apply patch ──────────────────────────────────────────────────────
        let mut files_changed: Vec<String> = Vec::new();
        if let Some(ref edits) = patch_out.file_edits {
            let directives = patch::parse_edit_directives(edits);
            for directive in &directives {
                let preview = patch::preview(&self.workspace, directive);
                let (response_tx, response_rx) = tokio::sync::oneshot::channel::<bool>();
                self.emit(WorkerEvent::PatchApprovalRequested {
                    path: directive.path.clone(),
                    preview,
                    response_tx,
                });
                let approved = response_rx.await.unwrap_or(false);
                if approved {
                    match patch::apply_directive(&self.workspace, directive) {
                        Ok(summary) => files_changed.push(summary),
                        Err(e) => self.emit(WorkerEvent::StageFailed {
                            stage: "patch:apply".into(),
                            error: e.to_string(),
                        }),
                    }
                }
            }
            if !files_changed.is_empty() {
                self.emit(WorkerEvent::PatchApplied {
                    files_changed: files_changed.clone(),
                });
            }
        }

        // ── Stage 5: REPORT ──────────────────────────────────────────────────
        let patch_summary = if files_changed.is_empty() {
            "No files were changed.".to_string()
        } else {
            files_changed.join(", ")
        };
        let report_task = format!(
            "{task}\n\nChanges applied: {patch_summary}\nSummarize what was done and what to verify."
        );
        let report_pack = ContextPackBuilder::new(&self.workspace)
            .with_budget(4_000)
            .build(&report_task, &ContextHints::default())?;
        self.emit_pack(&report_pack, StageKind::Report.label());

        self.emit(WorkerEvent::StageStarted {
            stage: StageKind::Report.label().into(),
        });
        let report_out = self
            .run_stage(StageKind::Report, &report_task, &report_pack)
            .await?;
        self.emit(WorkerEvent::StageCompleted {
            stage: StageKind::Report.label().into(),
            crp_valid: report_out.crp_valid,
        });

        let final_response = report_out
            .final_response
            .unwrap_or_else(|| "Task complete.".into());

        self.emit(WorkerEvent::WorkflowComplete {
            final_response: final_response.clone(),
        });

        Ok(final_response)
    }

    // ── Single stage ─────────────────────────────────────────────────────────

    async fn run_stage(
        &self,
        kind: StageKind,
        task: &str,
        pack: &ContextPack,
    ) -> Result<StageOutput> {
        let templates = crp::TemplateSet::builtin();
        let template = templates.resolve(kind.template_name(), "");
        let system_prompt = self.config.apply_no_think_prefix(format!(
            "{}\n\n{}\n\n---\n\n{}",
            template.render_brief(),
            one_shot_example(&kind),
            pack.formatted
        ));

        let client = ModelClient::new(self.config.model.clone());
        let max_retries = self.config.harness.crp_retry_budget.max(1);

        let mut user_message = task.to_string();
        for attempt in 0..=max_retries {
            let payload = build_payload(&self.config, &system_prompt, &user_message);
            let result = client
                .complete(payload)
                .await
                .map_err(|e| anyhow!("model call failed at stage {}: {e}", kind.label()))?;
            let finish_reason = result.finish_reason.clone();

            match parse_stage_output(&result.text, &kind) {
                Ok(out) => {
                    self.emit(WorkerEvent::StageTrace {
                        stage: kind.label().into(),
                        attempt,
                        system_prompt: system_prompt.clone(),
                        user_message: user_message.clone(),
                        model_response: result.text.clone(),
                        crp_valid: out.crp_valid,
                        error: None,
                        finish_reason,
                    });
                    return Ok(out);
                }
                Err(reason) if attempt < max_retries => {
                    let reason = with_finish_reason(reason, finish_reason.as_deref());
                    self.emit(WorkerEvent::StageTrace {
                        stage: kind.label().into(),
                        attempt,
                        system_prompt: system_prompt.clone(),
                        user_message: user_message.clone(),
                        model_response: result.text.clone(),
                        crp_valid: false,
                        error: Some(reason.clone()),
                        finish_reason,
                    });
                    self.emit(WorkerEvent::StageRetry {
                        stage: kind.label().into(),
                        attempt: attempt + 1,
                        reason: reason.clone(),
                    });
                    user_message = retry_user_message(task, kind.label(), &reason);
                }
                Err(reason) => {
                    let reason = with_finish_reason(reason, finish_reason.as_deref());
                    self.emit(WorkerEvent::StageTrace {
                        stage: kind.label().into(),
                        attempt,
                        system_prompt: system_prompt.clone(),
                        user_message: user_message.clone(),
                        model_response: result.text.clone(),
                        crp_valid: false,
                        error: Some(reason.clone()),
                        finish_reason,
                    });
                    self.emit(WorkerEvent::StageFailed {
                        stage: kind.label().into(),
                        error: reason.clone(),
                    });
                    return Err(anyhow!("stage {} exhausted retries: {}", kind.label(), reason));
                }
            }
        }

        unreachable!()
    }

    fn make_pack(&self, task: &str, hints: &ContextHints) -> ContextPack {
        let mut builder = ContextPackBuilder::new(&self.workspace).with_index();
        if let Some(b) = self.budget {
            builder = builder.with_budget(b);
        }
        builder.build(task, hints).unwrap_or_else(|_| ContextPack {
            chars_used: 0,
            chars_budget: 0,
            truncated: false,
            formatted: format!("[TASK]\n{task}\n"),
        })
    }

    fn builder(&self) -> ContextPackBuilder {
        let mut b = ContextPackBuilder::new(&self.workspace).with_index();
        if let Some(budget) = self.budget {
            b = b.with_budget(budget);
        }
        b
    }

    fn emit(&self, event: WorkerEvent) {
        let _ = self.event_tx.send(event);
    }

    fn emit_pack(&self, pack: &ContextPack, stage: &str) {
        self.emit(WorkerEvent::ContextPackReady {
            stage: stage.into(),
            chars_used: pack.chars_used,
            budget: pack.chars_budget,
        });
    }
}

// ---------------------------------------------------------------------------
// One-shot format example
// ---------------------------------------------------------------------------

fn one_shot_example(kind: &StageKind) -> &'static str {
    match kind {
        StageKind::Interpret => "\
## Output format — follow this exactly

<TASK_INTERPRETATION>
One sentence restating the task in your own words.
</TASK_INTERPRETATION>

<FINAL_RESPONSE>
Brief answer or plan.
</FINAL_RESPONSE>",

        StageKind::Locate => "\
## Output format — follow this exactly

<RELEVANT_FILES>
src/lib.rs
src/main.rs
</RELEVANT_FILES>

<FINAL_RESPONSE>
The relevant files are src/lib.rs and src/main.rs.
</FINAL_RESPONSE>",

        StageKind::Hypothesize => "\
## Output format — follow this exactly

<PROPOSED_APPROACH>
- Step one of the fix.
- Step two of the fix.
</PROPOSED_APPROACH>

<FINAL_RESPONSE>
Brief summary of the proposed approach.
</FINAL_RESPONSE>",

        StageKind::Patch => "\
## Output format — follow this exactly

<FILE_EDITS>
<EDIT path=\"src/lib.rs\" mode=\"replace_function:broken_fn\">
fn broken_fn() -> i32 {
    42
}
</EDIT>
</FILE_EDITS>

<FINAL_RESPONSE>
Fixed broken_fn to return the correct value.
</FINAL_RESPONSE>",

        StageKind::Report => "\
## Output format — follow this exactly

<FINAL_RESPONSE>
Summary of what was changed and what the user should verify.
</FINAL_RESPONSE>",
    }
}

// ---------------------------------------------------------------------------
// Payload construction
// ---------------------------------------------------------------------------

fn build_payload(config: &Config, system_prompt: &str, user_message: &str) -> ChatRequestPayload {
    match config.model.format.as_str() {
        "harmony" => ChatRequestPayload {
            mode: RequestMode::HarmonyText {
                prompt: format!("{system_prompt}\n\n<|user|>{user_message}<|assistant|>"),
            },
        },
        _ => ChatRequestPayload {
            mode: RequestMode::OpenAiChat {
                messages: vec![
                    ApiMessage {
                        role: "system".into(),
                        content: Some(system_prompt.into()),
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                    },
                    ApiMessage {
                        role: "user".into(),
                        content: Some(user_message.into()),
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                    },
                ],
                tools: vec![],
            },
        },
    }
}

// ---------------------------------------------------------------------------
// Stage output parsing
// ---------------------------------------------------------------------------

/// Strip `<think>...</think>` blocks emitted by reasoning models (Qwen3, DeepSeek-R1, etc.)
/// before attempting to parse the response. Returns the text after the last closing tag,
/// or the full text if no think block is present.
fn strip_think_blocks(text: &str) -> &str {
    // Find the last </think> tag — the actual response comes after it.
    if let Some(end) = text.rfind("</think>") {
        text[end + "</think>".len()..].trim_start()
    } else {
        text
    }
}

fn parse_stage_output(raw: &str, kind: &StageKind) -> Result<StageOutput, String> {
    let raw = strip_think_blocks(raw);

    // Try structured CRP first.
    if let Ok(trace) = crp::parse(raw) {
        let final_response = trace
            .get("FINAL_RESPONSE")
            .map(|s| s.content.trim().to_string());

        if final_response.as_deref().is_some_and(|s| !s.is_empty()) {
            let mut out = StageOutput {
                raw_trace: raw.to_string(),
                final_response,
                crp_valid: true,
                ..Default::default()
            };

            if matches!(kind, StageKind::Interpret) {
                let source = trace
                    .get("TASK_INTERPRETATION")
                    .or_else(|| trace.get("PROPOSED_APPROACH"))
                    .map(|s| s.content.as_str())
                    .unwrap_or_default();
                out.search_terms = extract_search_terms(source);
            }
            if matches!(kind, StageKind::Locate) {
                if let Some(slot) = trace.get("RELEVANT_FILES") {
                    out.relevant_files = parse_bullet_paths(&slot.content);
                }
            }
            if let Some(slot) = trace.get("PROPOSED_APPROACH") {
                let t = slot.content.trim();
                if !t.is_empty() {
                    out.approach = Some(t.to_string());
                }
            }
            if let Some(slot) = trace.get("FILE_EDITS") {
                let t = slot.content.trim();
                if !t.is_empty() {
                    out.file_edits = Some(t.to_string());
                }
            }
            return Ok(out);
        }
    }

    // Plain-text fallback for models that don't follow CRP format.
    // The response is non-empty prose — salvage what we can so the
    // pipeline can continue. crp_valid = false is recorded in metrics.
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty model response".into());
    }

    let mut out = StageOutput {
        raw_trace: raw.to_string(),
        final_response: Some(trimmed.to_string()),
        ..Default::default()
    };

    // For interpret, extract identifiers from plain prose as search terms.
    if matches!(kind, StageKind::Interpret) {
        out.search_terms = extract_search_terms(trimmed);
    }

    // For locate, try to find file paths mentioned in plain text.
    if matches!(kind, StageKind::Locate) {
        out.relevant_files = parse_bullet_paths(trimmed);
        if out.relevant_files.is_empty() {
            // Scan for src/*.rs style paths directly in prose
            out.relevant_files = trimmed
                .split_whitespace()
                .filter(|w| w.contains('/') && w.contains('.'))
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.').to_string())
                .filter(|w| !w.is_empty())
                .take(6)
                .collect();
        }
    }

    if matches!(kind, StageKind::Hypothesize | StageKind::Patch) {
        out.approach = Some(trimmed.to_string());
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_search_terms(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|word| {
            let clean: String = word
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '/' && c != '.')
                .to_string();
            if clean.len() > 3
                && clean.chars().next().is_some_and(|c| c.is_alphabetic())
                && clean
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '/'))
            {
                Some(clean)
            } else {
                None
            }
        })
        .take(8)
        .collect()
}

fn parse_bullet_paths(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let t = line
                .trim()
                .trim_start_matches(['-', '*', ' '])
                .trim();
            if t.is_empty() || t.starts_with('#') {
                return None;
            }
            let path = t.split_whitespace().next().unwrap_or("").to_string();
            if path.is_empty() { None } else { Some(path) }
        })
        .collect()
}

fn with_finish_reason(reason: String, finish_reason: Option<&str>) -> String {
    let Some(finish_reason) = finish_reason.filter(|value| !value.trim().is_empty()) else {
        return reason;
    };
    format!("{reason} (finish_reason: {finish_reason})")
}

fn retry_user_message(task: &str, stage: &str, reason: &str) -> String {
    format!(
        "{task}\n\n<RETRY_REASON>\nThe previous {stage} response was not usable: {reason}. Re-emit a concise response in the requested CRP format with a non-empty <FINAL_RESPONSE>.\n</RETRY_REASON>"
    )
}

fn file_hints_from_search_terms(workspace: &Path, terms: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    for term in terms.iter().filter(|term| !term.trim().is_empty()).take(8) {
        let Ok(result) = search(workspace, SearchParams::new(term)) else {
            continue;
        };
        for file in result.files {
            if files.len() >= 6 {
                return files;
            }
            if !files.iter().any(|seen| seen == &file) {
                files.push(file);
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_hints_fall_back_to_search_matches() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
        let terms = vec!["WorkerLoop".to_string()];

        let files = file_hints_from_search_terms(workspace, &terms);

        assert!(files.iter().any(|file| file == "src/kernel/worker.rs"));
    }
}
