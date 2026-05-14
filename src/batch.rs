use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::adapter::{ApiMessage, ChatRequestPayload, RequestMode};
use crate::config::{Config, ModelConfig};
use crate::crp;
use crate::model::ModelClient;
use crate::session::{AgentSession, Channel, Role, TurnEvent};
use crate::workspace::clean_workspace;

#[derive(Debug, Deserialize)]
struct BatchTask {
    id: String,
    prompt: String,
    #[serde(default)]
    fixture_dir: Option<PathBuf>,
    #[serde(default)]
    validation_command: Option<String>,
}

#[derive(Debug, Serialize)]
struct BatchResult {
    task_id: String,
    model: String,
    crp_enabled: bool,
    metadata: BatchMetadata,
    outputs: BatchOutputs,
    metrics: BatchMetrics,
    errors: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BatchMetadata {
    cinto_version: String,
    timestamp: u64,
    temperature: f32,
    thinking_effort: String,
    system_prompt: String,
}

#[derive(Debug, Serialize, Default)]
struct BatchOutputs {
    raw_response: String,
    parsed_successfully: bool,
    required_slots_present: bool,
    filepaths_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_compiles: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tests_pass: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evaluator_score: Option<String>,
}

#[derive(Debug, Serialize, Default)]
struct BatchMetrics {
    tokens_in: usize,
    tokens_out: usize,
    duration_ms: u128,
    retries: u32,
}

pub async fn run(
    config: Config,
    tasks_path: PathBuf,
    output_path: PathBuf,
    evaluator_endpoint: Option<String>,
    evaluator_model: Option<String>,
    evaluator_api_key: Option<String>,
    dry_run: bool,
    dangerously_auto_approve: bool,
) -> Result<()> {
    if !dry_run && !dangerously_auto_approve {
        anyhow::bail!(
            "FATAL SECURITY RISK: Running batch mode will automatically execute AI-generated code and shell commands on your host system. \
            This can result in data loss or system compromise.\n\
            You must run this in an isolated VM or Docker container and pass the --dangerously-auto-approve flag to confirm you understand the risks."
        );
    }
    let file = std::fs::File::open(&tasks_path)
        .with_context(|| format!("failed to open tasks file {}", tasks_path.display()))?;
    let reader = BufReader::new(file);

    let mut output_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&output_path)
        .with_context(|| format!("failed to open output file {}", output_path.display()))?;

    let evaluator_client = if !dry_run && evaluator_endpoint.is_some() {
        let endpoint = evaluator_endpoint.unwrap();
        let eval_config = ModelConfig {
            endpoint,
            model: evaluator_model.unwrap_or_else(|| "deepseek-chat".to_string()),
            format: "openai-tools".to_string(),
            api_key_env: None,
            max_tokens: 100,
            temperature: 0.0,
            thinking_effort: "none".to_string(),
            no_think: false,
            no_think_prefix: "/no_think".to_string(),
            stream: false,
            stop: vec![],
            request_timeout_secs: 60,
            context_window: 8192,
        };
        if let Some(key) = &evaluator_api_key {
            unsafe {
                std::env::set_var("CINTO_EVAL_KEY", key);
            }
            let mut cfg = eval_config.clone();
            cfg.api_key_env = Some("CINTO_EVAL_KEY".to_string());
            Some(ModelClient::new(cfg))
        } else {
            Some(ModelClient::new(eval_config))
        }
    } else {
        None
    };

    if dry_run {
        println!("Running in DRY-RUN mode. Models will not be called.");
    }

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let task: BatchTask =
            serde_json::from_str(&line).context("failed to parse task JSON line")?;

        println!("Running task {}...", task.id);
        let mut errors = Vec::new();
        let mut metrics = BatchMetrics::default();
        let mut outputs = BatchOutputs::default();

        let mut active_workspace = config.harness.workspace.clone();
        let mut is_temp_workspace = false;

        if let Some(fixture) = &task.fixture_dir {
            if !fixture.exists() || !fixture.is_dir() {
                println!("  Fixture directory not found: {}", fixture.display());
                errors.push(format!("Fixture not found: {}", fixture.display()));
                if dry_run {
                    continue;
                }
            } else {
                let millis = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let temp_dir =
                    std::env::temp_dir().join(format!("cinto-eval-{}-{}", task.id, millis));

                // Copy fixture to temp
                let status = Command::new("cp")
                    .arg("-R")
                    .arg(fixture)
                    .arg(&temp_dir)
                    .status()?;
                if !status.success() {
                    println!("  Failed to copy fixture to temp dir.");
                    errors.push("Failed to copy fixture".to_string());
                    if dry_run {
                        continue;
                    }
                } else {
                    active_workspace = temp_dir.clone();
                    is_temp_workspace = true;

                    // Initialize git in temp dir
                    Command::new("git")
                        .current_dir(&temp_dir)
                        .args(["init"])
                        .output()?;
                    Command::new("git")
                        .current_dir(&temp_dir)
                        .args(["add", "."])
                        .output()?;
                    Command::new("git")
                        .current_dir(&temp_dir)
                        .args(["commit", "-m", "Initial"])
                        .output()?;
                }
            }
        }

        if dry_run {
            if is_temp_workspace {
                println!(
                    "  Dry-run: removing temp workspace {}",
                    active_workspace.display()
                );
                std::fs::remove_dir_all(&active_workspace).ok();
            }
            continue;
        }

        let mut task_config = config.clone();
        task_config.harness.workspace = active_workspace.clone();

        clean_workspace(&active_workspace).unwrap_or_else(|e| {
            errors.push(format!("Clean workspace failed: {}", e));
            String::new()
        });

        let mut session = AgentSession::new(task_config.clone());
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let prompt = task.prompt.clone();

        metrics.tokens_in = session.estimated_prompt_tokens() + (prompt.len() / 4);
        let start_time = Instant::now();

        let session_handle = tokio::spawn(async move {
            let res = session
                .send_user_message_streaming(prompt, event_tx.clone())
                .await;
            (session, res)
        });

        while let Some(event) = event_rx.recv().await {
            match event {
                TurnEvent::ToolApprovalRequested { response_tx, .. } => {
                    let _ = response_tx.send(true);
                }
                TurnEvent::CrpRetryRequested { attempt, .. } => {
                    metrics.retries = attempt;
                }
                _ => {}
            }
        }

        metrics.duration_ms = start_time.elapsed().as_millis();

        let (session, res) = session_handle.await?;
        if let Err(e) = res {
            errors.push(format!("Session error: {}", e));
        }

        if let Some(last_msg) = session.history().last() {
            if last_msg.role == Role::Assistant && last_msg.channel == Some(Channel::Final) {
                outputs.raw_response = last_msg.content.clone();
                metrics.tokens_out = outputs.raw_response.len() / 4;

                match crp::parse(&outputs.raw_response) {
                    Ok(trace) => {
                        outputs.parsed_successfully = true;

                        let template_name = session.config().harness.default_template.clone();
                        let effort = session.config().model.thinking_effort.clone();
                        let templates = crp::TemplateSet::load(Some(
                            crp::workspace_template_dir(&session.config().harness.workspace)
                                .as_path(),
                        ));
                        let active_template = templates.resolve(&template_name, &effort);
                        let validation_config =
                            active_template.validation_config(Some(active_workspace.as_path()));
                        let report = crp::validate(&trace, &validation_config);

                        let mut required_slots_missing = false;
                        let mut filepaths_invalid = false;
                        for outcome in &report.outcomes {
                            if outcome.status == crp::SlotStatus::Invalid {
                                if outcome.message.contains("Required slot") {
                                    required_slots_missing = true;
                                }
                                if outcome.message.contains("Path(s) listed in") {
                                    filepaths_invalid = true;
                                }
                            }
                        }

                        outputs.required_slots_present = !required_slots_missing;
                        outputs.filepaths_valid = !filepaths_invalid;

                        if !report.is_executable() {
                            errors.push(
                                "Trace failed semantic validation (slots or files missing)."
                                    .to_string(),
                            );
                        } else {
                            // Syntactically and statically valid. Let's check semantic tests if requested.
                            if let Some(cmd) = &task.validation_command {
                                // Run the validation command in the active workspace
                                let args: Vec<&str> = cmd.split_whitespace().collect();
                                if !args.is_empty() {
                                    let mut command = Command::new(args[0]);
                                    if args.len() > 1 {
                                        command.args(&args[1..]);
                                    }
                                    command.current_dir(&active_workspace);
                                    if let Ok(output) = command.output() {
                                        outputs.tests_pass = Some(output.status.success());
                                        outputs.code_compiles = Some(true); // Assuming tests compiled
                                    } else {
                                        outputs.tests_pass = Some(false);
                                        errors.push(format!(
                                            "Failed to execute validation command: {}",
                                            cmd
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        errors.push(format!("CRP parse error: {}", e));
                    }
                }
            } else {
                errors.push("Last message was not a final assistant response.".to_string());
            }
        } else {
            errors.push("No assistant response generated.".to_string());
        }

        // Run evaluator judge if we parsed successfully and have an evaluator
        if outputs.parsed_successfully && evaluator_client.is_some() {
            let eval_client = evaluator_client.as_ref().unwrap();
            let eval_prompt = format!(
                "Task: {}\n\nTrace:\n{}\n\nDid this trace successfully accomplish the task? Output ONLY <EVALUATOR_PASS> or <EVALUATOR_FAIL>.",
                task.prompt, outputs.raw_response
            );
            let payload = ChatRequestPayload {
                mode: RequestMode::OpenAiChat {
                    messages: vec![ApiMessage {
                        role: "user".to_string(),
                        content: Some(eval_prompt),
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                    }],
                    tools: vec![],
                },
            };
            match eval_client.complete(payload).await {
                Ok(res) => {
                    let score = res.text.trim();
                    outputs.evaluator_score = Some(score.to_string());
                }
                Err(e) => {
                    errors.push(format!("Evaluator error: {}", e));
                }
            }
        }

        let result = BatchResult {
            task_id: task.id.clone(),
            model: config.model.model.clone(),
            crp_enabled: config
                .harness
                .reasoning_protocol
                .eq_ignore_ascii_case("crp"),
            metadata: BatchMetadata {
                cinto_version: env!("CARGO_PKG_VERSION").to_string(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                temperature: config.model.temperature,
                thinking_effort: config.model.thinking_effort.clone(),
                system_prompt: config.harness.system_prompt.clone(),
            },
            outputs,
            metrics,
            errors,
        };

        let json = serde_json::to_string(&result)?;
        writeln!(output_file, "{}", json)?;
        output_file.flush()?;

        if is_temp_workspace {
            std::fs::remove_dir_all(&active_workspace).ok();
        } else {
            clean_workspace(&active_workspace).ok();
        }
    }

    if !dry_run {
        println!("Batch complete.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Kernel batch runner
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Default)]
struct KernelStageResult {
    crp_valid: bool,
    duration_ms: u128,
    retries: u32,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct KernelBatchResult {
    task_id: String,
    model: String,
    timestamp: u64,
    stages_attempted: u32,
    stages_completed: u32,
    workflow_succeeded: bool,
    interpret: Option<KernelStageResult>,
    locate: Option<KernelStageResult>,
    hypothesize: Option<KernelStageResult>,
    patch: Option<KernelStageResult>,
    report: Option<KernelStageResult>,
    total_duration_ms: u128,
    errors: Vec<String>,
}

pub async fn run_kernel(
    config: Config,
    tasks_path: PathBuf,
    output_path: PathBuf,
    dry_run: bool,
    budget: Option<usize>,
    traces_dir: Option<PathBuf>,
) -> Result<()> {
    use crate::kernel::{
        index::index_repo,
        worker::{WorkerEvent, WorkerLoop},
    };

    if dry_run {
        println!("Kernel batch DRY-RUN: validating task file only.");
    }

    let file = std::fs::File::open(&tasks_path)
        .with_context(|| format!("failed to open {}", tasks_path.display()))?;
    let reader = BufReader::new(file);

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }

    // Set up trace writer if requested
    let mut trace_file = if let Some(ref dir) = traces_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create traces dir {}", dir.display()))?;
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let trace_path = dir.join(format!("traces_{ts}.jsonl"));
        println!("Saving traces to {}", trace_path.display());
        Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&trace_path)
                .with_context(|| format!("failed to open {}", trace_path.display()))?,
        )
    } else {
        None
    };

    let mut output_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&output_path)
        .with_context(|| format!("failed to open {}", output_path.display()))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let task: BatchTask = serde_json::from_str(line)
            .with_context(|| format!("failed to parse task at line {}", line_idx + 1))?;

        println!("[kernel] task: {} — {}", task.id, &task.prompt[..task.prompt.len().min(60)]);

        if dry_run {
            println!("  [dry-run] would run WorkerLoop on task {}", task.id);
            continue;
        }

        // ── Set up workspace ─────────────────────────────────────────────────
        let (active_workspace, is_temp) = if let Some(fixture) = &task.fixture_dir {
            let src = if fixture.is_absolute() {
                fixture.clone()
            } else {
                std::env::current_dir()?.join(fixture)
            };
            let tmp = std::env::temp_dir().join(format!("cinto_kernel_{}_{}", task.id, timestamp));
            copy_dir_all(&src, &tmp)?;
            (tmp, true)
        } else {
            (config.harness.workspace.clone(), false)
        };

        // ── Pre-index the workspace ──────────────────────────────────────────
        if let Ok(idx) = index_repo(&active_workspace) {
            let _ = crate::kernel::index::save(&active_workspace, &idx);
        }

        // ── Run WorkerLoop with concurrent event handler ─────────────────────
        // Approvals must be answered during the workflow (the worker awaits
        // response_rx), so we drain events in a separate task rather than
        // collecting them post-hoc.
        let (event_tx, event_rx) = mpsc::unbounded_channel::<WorkerEvent>();
        let mut kernel_config = config.clone();
        kernel_config.harness.workspace = active_workspace.clone();

        let mut worker = WorkerLoop::new(&active_workspace, kernel_config, task.prompt.clone(), event_tx);
        if let Some(b) = budget {
            worker = worker.with_budget(b);
        }

        // Spawn event consumer — auto-approves patches, records all other events.
        let event_handle: tokio::task::JoinHandle<Vec<WorkerEvent>> =
            tokio::spawn(async move {
                let mut rx = event_rx;
                let mut recorded = Vec::new();
                while let Some(event) = rx.recv().await {
                    match event {
                        WorkerEvent::PatchApprovalRequested { path, response_tx, .. } => {
                            println!("  [auto-approve] write {path}");
                            let _ = response_tx.send(true);
                        }
                        e => recorded.push(e),
                    }
                }
                recorded
            });

        let start = Instant::now();
        let workflow_result = worker.run_bugfix().await;
        let total_duration_ms = start.elapsed().as_millis();

        // event_tx dropped when WorkerLoop is consumed — signals handler to finish.
        let events = event_handle.await.unwrap_or_default();

        // ── Build per-stage results from events ──────────────────────────────
        let mut stage_results: std::collections::HashMap<String, KernelStageResult> =
            std::collections::HashMap::new();
        let mut current_stage: Option<String> = None;
        let mut stage_start: Option<Instant> = None;
        let mut retry_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        let mut errors: Vec<String> = Vec::new();
        let mut stages_attempted: u32 = 0;
        let mut stages_completed: u32 = 0;

        // Replay events to reconstruct per-stage timing and validity
        let event_start = Instant::now(); // approximate — events already happened
        let mut pending_traces: Vec<serde_json::Value> = Vec::new();
        for event in &events {
            match event {
                WorkerEvent::StageStarted { stage } => {
                    current_stage = Some(stage.clone());
                    stage_start = Some(event_start); // approximate
                    stages_attempted += 1;
                }
                WorkerEvent::StageCompleted { stage, crp_valid } => {
                    let dur = stage_start.map(|s| s.elapsed().as_millis()).unwrap_or(0);
                    let retries = *retry_counts.get(stage).unwrap_or(&0);
                    stage_results.insert(stage.clone(), KernelStageResult {
                        crp_valid: *crp_valid,
                        duration_ms: dur,
                        retries,
                        error: None,
                    });
                    stages_completed += 1;
                    current_stage = None;
                }
                WorkerEvent::StageRetry { stage, attempt, .. } => {
                    *retry_counts.entry(stage.clone()).or_default() = *attempt;
                }
                WorkerEvent::StageFailed { stage, error } => {
                    let dur = stage_start.map(|s| s.elapsed().as_millis()).unwrap_or(0);
                    let retries = *retry_counts.get(stage).unwrap_or(&0);
                    stage_results.insert(stage.clone(), KernelStageResult {
                        crp_valid: false,
                        duration_ms: dur,
                        retries,
                        error: Some(error.clone()),
                    });
                    errors.push(format!("{stage}: {error}"));
                }
                WorkerEvent::WorkflowFailed { error } => {
                    errors.push(error.clone());
                }
                WorkerEvent::PatchApplied { files_changed } => {
                    println!("  [patch] {}", files_changed.join(", "));
                }
                WorkerEvent::StageSkipped { stage, reason } => {
                    errors.push(format!("{stage} skipped: {reason}"));
                }
                WorkerEvent::StageTrace {
                    stage,
                    attempt,
                    system_prompt,
                    user_message,
                    model_response,
                    crp_valid,
                    error,
                    finish_reason,
                } => {
                    pending_traces.push(serde_json::json!({
                        "task_id": task.id,
                        "model": config.model.model,
                        "stage": stage,
                        "attempt": attempt,
                        "system_prompt": system_prompt,
                        "user_message": user_message,
                        "model_response": model_response,
                        "crp_valid": crp_valid,
                        "error": error,
                        "finish_reason": finish_reason,
                        "workflow_succeeded": workflow_result.is_ok(),
                        "timestamp": timestamp,
                    }));
                }
                _ => {}
            }
        }

        if let Err(e) = &workflow_result {
            errors.push(format!("{e:#}"));
        }

        let result = KernelBatchResult {
            task_id: task.id.clone(),
            model: config.model.model.clone(),
            timestamp,
            stages_attempted,
            stages_completed,
            workflow_succeeded: workflow_result.is_ok(),
            interpret: stage_results.remove("interpret"),
            locate: stage_results.remove("locate"),
            hypothesize: stage_results.remove("hypothesize"),
            patch: stage_results.remove("patch"),
            report: stage_results.remove("report"),
            total_duration_ms,
            errors,
        };

        let json = serde_json::to_string(&result)?;
        writeln!(output_file, "{json}")?;
        output_file.flush()?;

        // Flush traces for this task
        if let Some(ref mut tf) = trace_file {
            for trace in &pending_traces {
                writeln!(tf, "{trace}")?;
            }
            tf.flush()?;
            if !pending_traces.is_empty() {
                println!("  traces: {} stage record(s) saved", pending_traces.len());
            }
        }

        println!(
            "  stages: {}/{} — workflow: {} — {total_duration_ms}ms",
            stages_completed,
            stages_attempted,
            if result.workflow_succeeded { "ok" } else { "failed" }
        );

        if is_temp {
            std::fs::remove_dir_all(&active_workspace).ok();
        }
    }

    if !dry_run {
        println!("Kernel batch complete.");
    }
    Ok(())
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
