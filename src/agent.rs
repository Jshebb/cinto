/// `cinto agent` — JSON IPC subprocess mode for editor extensions.
///
/// Spawned by an editor extension. Emits newline-delimited JSON events to
/// stdout (kernel → extension). Reads newline-delimited JSON messages from
/// stdin (extension → kernel) for patch approvals. Every message has a `type`
/// field. See docs/vscode-extension-plan.md for the full protocol spec.
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;

use anyhow::Result;

use crate::config::Config;
use crate::kernel::worker::{WorkerEvent, WorkerLoop};
use crate::model::ModelClient;

static APPROVAL_COUNTER: AtomicU32 = AtomicU32::new(0);

fn next_approval_id() -> String {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u32;
    let seq = APPROVAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{ts:08x}{seq:04x}")
}

fn emit(msg: serde_json::Value) {
    let line = serde_json::to_string(&msg).unwrap_or_else(|_| r#"{"type":"error","message":"serialization failed"}"#.into());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

pub async fn run(mut config: Config, task: String, workspace_override: Option<PathBuf>, traces_dir: Option<PathBuf>) -> Result<()> {
    if let Some(ws) = workspace_override {
        config.harness.workspace = ws;
    }
    let workspace = config.harness.workspace.clone();

    // ── Pre-index workspace ─────────────────────────────────────────────────
    if let Ok(idx) = crate::kernel::index::index_repo(&workspace) {
        let _ = crate::kernel::index::save(&workspace, &idx);
    }

    // ── Context window detection ────────────────────────────────────────────
    let client = ModelClient::new(config.model.clone());
    let effective_budget = match client.get_available_context_window().await {
        actual if actual > 0 && actual < config.model.context_window => {
            let chars = actual.saturating_sub(actual * 30 / 100) * 4;
            chars as usize
        }
        _ => crate::kernel::context_pack::DEFAULT_CODE_BUDGET,
    };

    emit(serde_json::json!({
        "type": "kernel_ready",
        "version": env!("CARGO_PKG_VERSION"),
        "workspace": workspace.display().to_string(),
        "model": config.model.model,
        "context_budget": effective_budget,
    }));

    // ── Spawn worker ────────────────────────────────────────────────────────
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<WorkerEvent>();
    let mut worker = WorkerLoop::new(&workspace, config.clone(), task.clone(), event_tx);
    worker = worker.with_budget(effective_budget);

    // Optionally set up trace writer
    let mut trace_file: Option<std::fs::File> = if let Some(ref dir) = traces_dir {
        std::fs::create_dir_all(dir).ok();
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = dir.join(format!("traces_{ts}.jsonl"));
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
    } else {
        None
    };

    let worker_handle = tokio::spawn(async move { worker.run_bugfix().await });

    // ── Stdin reader channel ────────────────────────────────────────────────
    // We read stdin in a blocking thread and forward lines through a channel
    // so the async event loop can select on both events and stdin.
    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => { let _ = stdin_tx.send(l); }
                Err(_) => break,
            }
        }
    });

    // ── Event loop ──────────────────────────────────────────────────────────
    // Tracks whether the workflow succeeded (used for trace filtering).
    let mut workflow_ok = false;

    while let Some(event) = event_rx.recv().await {
        match event {
            WorkerEvent::StageStarted { stage } => {
                emit(serde_json::json!({"type": "stage_started", "stage": stage}));
            }

            WorkerEvent::ContextPackReady { stage, chars_used, budget } => {
                emit(serde_json::json!({
                    "type": "context_pack_ready",
                    "stage": stage,
                    "chars_used": chars_used,
                    "budget": budget,
                }));
            }

            WorkerEvent::StageCompleted { stage, crp_valid } => {
                emit(serde_json::json!({
                    "type": "stage_completed",
                    "stage": stage,
                    "crp_valid": crp_valid,
                }));
            }

            WorkerEvent::StageRetry { stage, attempt, reason } => {
                emit(serde_json::json!({
                    "type": "stage_retry",
                    "stage": stage,
                    "attempt": attempt,
                    "reason": reason,
                }));
            }

            WorkerEvent::StageFailed { stage, error } => {
                emit(serde_json::json!({
                    "type": "stage_failed",
                    "stage": stage,
                    "error": error,
                }));
            }

            WorkerEvent::StageSkipped { stage, reason } => {
                emit(serde_json::json!({
                    "type": "stage_skipped",
                    "stage": stage,
                    "reason": reason,
                }));
            }

            WorkerEvent::PatchApplied { files_changed } => {
                emit(serde_json::json!({
                    "type": "patch_applied",
                    "files_changed": files_changed,
                }));
            }

            WorkerEvent::WorkflowComplete { final_response } => {
                workflow_ok = true;
                emit(serde_json::json!({
                    "type": "workflow_complete",
                    "final_response": final_response,
                }));
            }

            WorkerEvent::WorkflowFailed { error } => {
                emit(serde_json::json!({
                    "type": "workflow_failed",
                    "error": error,
                }));
            }

            // ── Patch approval: bidirectional exchange ──────────────────────
            // The kernel blocks on response_rx until we send. No further events
            // are emitted until the approval is resolved, so we can safely
            // block on stdin here without missing events.
            WorkerEvent::PatchApprovalRequested { path, preview, response_tx } => {
                let id = next_approval_id();
                emit(serde_json::json!({
                    "type": "patch_approval_requested",
                    "id": id,
                    "path": path,
                    "preview": preview,
                }));

                // Read from the stdin channel until we get a matching response.
                // The kernel is blocked waiting for this, so no further events
                // will arrive until we resolve the approval.
                let approved = loop {
                    match stdin_rx.recv().await {
                        None => break false, // channel closed = EOF
                        Some(line) => {
                            let line = line.trim().to_string();
                            if line.is_empty() { continue; }
                            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
                                let is_response = msg
                                    .get("type")
                                    .and_then(|t| t.as_str())
                                    == Some("patch_approval_response");
                                let id_matches = msg
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    == Some(id.as_str());
                                if is_response && id_matches {
                                    break msg
                                        .get("approved")
                                        .and_then(|a| a.as_bool())
                                        .unwrap_or(false);
                                }
                            }
                        }
                    }
                };

                let _ = response_tx.send(approved);
            }

            // ── Trace events (write to trace file if enabled) ───────────────
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
                if let Some(ref mut tf) = trace_file {
                    let record = serde_json::json!({
                        "stage": stage,
                        "attempt": attempt,
                        "system_prompt": system_prompt,
                        "user_message": user_message,
                        "model_response": model_response,
                        "crp_valid": crp_valid,
                        "workflow_succeeded": workflow_ok,
                        "error": error,
                        "finish_reason": finish_reason,
                        "model": config.model.model,
                    });
                    let _ = writeln!(tf, "{}", serde_json::to_string(&record).unwrap_or_default());
                    let _ = tf.flush();
                }
            }
        }
    }

    worker_handle.await??;

    Ok(())
}
