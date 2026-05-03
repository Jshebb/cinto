use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

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
}

#[derive(Debug, Serialize)]
struct BatchResult {
    id: String,
    trace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_score: Option<String>,
}

pub async fn run(
    config: Config,
    tasks_path: PathBuf,
    output_path: PathBuf,
    evaluator_endpoint: Option<String>,
    evaluator_model: Option<String>,
    evaluator_api_key: Option<String>,
) -> Result<()> {
    let file = std::fs::File::open(&tasks_path)
        .with_context(|| format!("failed to open tasks file {}", tasks_path.display()))?;
    let reader = BufReader::new(file);

    let mut output_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&output_path)
        .with_context(|| format!("failed to open output file {}", output_path.display()))?;

    let evaluator_client = if let Some(endpoint) = evaluator_endpoint {
        let eval_config = ModelConfig {
            endpoint,
            model: evaluator_model.unwrap_or_else(|| "deepseek-chat".to_string()),
            format: "openai-tools".to_string(),
            api_key_env: None,
            max_tokens: 100,
            temperature: 0.0,
            thinking_effort: "none".to_string(),
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

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let task: BatchTask =
            serde_json::from_str(&line).context("failed to parse task JSON line")?;

        println!("Running task {}...", task.id);

        clean_workspace(&config.harness.workspace)?;

        let mut session = AgentSession::new(config.clone());
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let prompt = task.prompt.clone();
        let session_handle = tokio::spawn(async move {
            let res = session
                .send_user_message_streaming(prompt, event_tx.clone())
                .await;
            (session, res)
        });

        while let Some(event) = event_rx.recv().await {
            match event {
                TurnEvent::ToolApprovalRequested {
                    recipient,
                    response_tx,
                    ..
                } => {
                    println!("  Auto-approving tool: {}", recipient);
                    let _ = response_tx.send(true);
                }
                TurnEvent::CrpRetryRequested { attempt, .. } => {
                    println!("  CRP retry requested (attempt {})", attempt);
                }
                TurnEvent::CrpRetryExhausted { .. } => {
                    println!("  CRP retries exhausted");
                }
                TurnEvent::Message(msg) => {
                    if msg.role == Role::Tool {
                        println!("  Tool executed: {}", msg.recipient.unwrap_or_default());
                    }
                }
                _ => {}
            }
        }

        let (session, res) = session_handle.await?;
        if let Err(e) = res {
            eprintln!("  Session error: {}", e);
            continue;
        }

        let Some(last_msg) = session.history().last() else {
            println!("  No assistant response generated.");
            continue;
        };

        if last_msg.role != Role::Assistant || last_msg.channel != Some(Channel::Final) {
            println!("  Last message was not a final assistant response.");
            continue;
        }

        let trace_text = &last_msg.content;
        let trace = match crp::parse(trace_text) {
            Ok(t) => t,
            Err(_) => {
                println!("  Failed to parse final CRP trace.");
                continue;
            }
        };

        let template = session.config().harness.default_template.clone();
        let effort = session.config().model.thinking_effort.clone();
        let templates = crp::TemplateSet::load(Some(
            crp::workspace_template_dir(&session.config().harness.workspace).as_path(),
        ));
        let active_template = templates.resolve(&template, &effort);
        let validation_config =
            active_template.validation_config(Some(session.config().harness.workspace.as_path()));
        let report = crp::validate(&trace, &validation_config);

        if !report.is_executable() {
            println!("  Trace failed syntactic validation.");
            continue;
        }

        let mut eval_score = None;
        if let Some(eval_client) = &evaluator_client {
            println!("  Running evaluator...");
            let eval_prompt = format!(
                "Task: {}\n\nTrace:\n{}\n\nDid this trace successfully accomplish the task? Output ONLY <EVALUATOR_PASS> or <EVALUATOR_FAIL>.",
                task.prompt, trace_text
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
                    eval_score = Some(score.to_string());
                    if !score.contains("<EVALUATOR_PASS>") {
                        println!("  Evaluator failed the trace: {}", score);
                        continue;
                    }
                }
                Err(e) => {
                    eprintln!("  Evaluator error: {}", e);
                    continue;
                }
            }
        }

        println!("  Success. Saving trace.");
        let result = BatchResult {
            id: task.id,
            trace: trace_text.clone(),
            eval_score,
        };

        let json = serde_json::to_string(&result)?;
        writeln!(output_file, "{}", json)?;
        output_file.flush()?;

        clean_workspace(&config.harness.workspace)?;
    }

    println!("Batch complete.");
    Ok(())
}
