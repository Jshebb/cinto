use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{
    adapter::{AssistantOutput, PromptAdapter, build_adapter},
    config::Config,
    crp,
    model::ModelClient,
};

const MAX_TOOL_CONTEXT_CHARS: usize = 8_000;
const MAX_TOOL_CONTEXT_LINES: usize = 220;
const TOOL_OUTPUT_END: &str = "<CINTO_TOOL_OUTPUT_END>";
const CONTEXT_COMPACTED_MARKER: &str = "<CINTO_CONTEXT_COMPACTED>";
const MAX_CONTEXT_SUMMARY_CHARS: usize = 4_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    #[allow(dead_code)]
    Analysis,
    Commentary,
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub channel: Option<Channel>,
    pub recipient: Option<String>,
    pub content: String,
}

#[derive(Debug)]
pub enum TurnEvent {
    AssistantDelta(String),
    DiscardAssistantDraft,
    ToolApprovalRequested {
        recipient: String,
        summary: String,
        response_tx: oneshot::Sender<bool>,
    },
    ContextCompacted {
        original_messages: usize,
        kept_messages: usize,
        estimated_tokens: usize,
        threshold_tokens: usize,
    },
    EmptyModelResponse {
        format: String,
        model: String,
    },
    ModelRetryRequested {
        attempt: u32,
        budget: u32,
        reason: String,
    },
    CrpRetryRequested {
        attempt: u32,
        budget: u32,
        invalid_slots: Vec<String>,
        parse_error: Option<String>,
    },
    CrpRetryExhausted {
        budget: u32,
        invalid_slots: Vec<String>,
    },
    Message(Message),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrpValidationDecision {
    Accept,
    RetryQueued,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelRetryDecision {
    RetryQueued,
    Exhausted,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            channel: None,
            recipient: None,
            content: content.into(),
        }
    }

    pub fn assistant_final(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            channel: Some(Channel::Final),
            recipient: None,
            content: content.into(),
        }
    }

    pub fn assistant_tool_call(recipient: impl Into<String>, arguments: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            channel: Some(Channel::Commentary),
            recipient: Some(recipient.into()),
            content: arguments.into(),
        }
    }

    pub fn tool(recipient: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            channel: Some(Channel::Commentary),
            recipient: Some(recipient.into()),
            content: content.into(),
        }
    }
}

#[derive(Debug)]
pub struct AgentSession {
    config: Config,
    adapter: Box<dyn PromptAdapter>,
    client: ModelClient,
    templates: crp::TemplateSet,
    history: Vec<Message>,
    todos: Vec<TodoItem>,
}

impl AgentSession {
    pub fn new(config: Config) -> Self {
        let adapter = build_adapter(&config).unwrap_or_else(|error| {
            eprintln!(
                "warning: {error}; falling back to harmony transport with configured reasoning protocol"
            );
            fallback_adapter(&config)
        });
        let client = ModelClient::new(config.model.clone());
        let templates = crp::TemplateSet::load(Some(
            crp::workspace_template_dir(&config.harness.workspace).as_path(),
        ));

        Self {
            config,
            adapter,
            client,
            templates,
            history: Vec::new(),
            todos: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.todos.clear();
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn update_config(&mut self, config: Config) {
        self.adapter = build_adapter(&config).unwrap_or_else(|error| {
            eprintln!(
                "warning: {error}; falling back to harmony transport with configured reasoning protocol"
            );
            fallback_adapter(&config)
        });
        self.client = ModelClient::new(config.model.clone());
        self.templates = crp::TemplateSet::load(Some(
            crp::workspace_template_dir(&config.harness.workspace).as_path(),
        ));
        self.config = config;
    }

    pub fn render_prompt(&self) -> String {
        self.adapter.debug_render(&self.history)
    }

    pub fn estimated_prompt_tokens(&self) -> usize {
        self.render_prompt().len() / 4
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn discard_trailing_user_message(&mut self) -> bool {
        if self
            .history
            .last()
            .is_some_and(|message| message.role == Role::User)
        {
            self.history.pop();
            return true;
        }
        false
    }

    pub fn tool_details(&self) -> String {
        self.adapter.tool_details()
    }

    pub fn todo_details(&self) -> String {
        format_todos(&self.todos)
    }

    pub fn todo_status_line(&self) -> String {
        if self.todos.is_empty() {
            return "none".to_string();
        }

        let total = self.todos.len();
        let done = self
            .todos
            .iter()
            .filter(|item| item.status == TodoStatus::Done)
            .count();
        let blocked = self
            .todos
            .iter()
            .filter(|item| item.status == TodoStatus::Blocked)
            .count();
        let active = total.saturating_sub(done);

        if blocked > 0 {
            format!("{done}/{total} done, {blocked} blocked")
        } else {
            format!("{done}/{total} done, {active} active")
        }
    }

    pub async fn send_user_message_streaming(
        &mut self,
        content: String,
        event_tx: UnboundedSender<TurnEvent>,
    ) -> Result<()> {
        self.history.push(Message::user(content));
        self.complete_turn_streaming(event_tx).await
    }

    async fn complete_turn_streaming(
        &mut self,
        event_tx: UnboundedSender<TurnEvent>,
    ) -> Result<()> {
        let max_tool_turns = self.config.harness.max_tool_turns.max(1);
        let max_model_rounds = self.config.harness.max_model_rounds.max(1);
        let crp_enabled = self
            .config
            .harness
            .reasoning_protocol
            .eq_ignore_ascii_case("crp");
        let crp_budget = self.config.harness.crp_retry_budget;
        let transport_budget = self.config.harness.transport_retry_budget;
        let empty_response_budget = self.config.harness.empty_response_retry_budget;
        let mut crp_retries_used: u32 = 0;
        let mut empty_retries_used: u32 = 0;
        let mut transport_retries_used: u32 = 0;
        let mut tool_turns_used: u32 = 0;
        let mut model_rounds_used: u32 = 0;

        loop {
            if model_rounds_used >= max_model_rounds {
                let message = Message::assistant_final(format!(
                    "I hit the configured model-round budget ({max_model_rounds}) before producing a final answer. Try asking for a narrower step, or raise `harness.max_model_rounds` in settings/config."
                ));
                self.history.push(message.clone());
                let _ = event_tx.send(TurnEvent::Message(message));
                return Ok(());
            }
            model_rounds_used += 1;

            if let Some(event) = self.compact_context_if_needed().await {
                let _ = event_tx.send(event);
            }
            let payload = self.adapter.render_request(&self.history);
            let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel();
            let client = self.client.clone();
            let completion = client.complete_streaming(payload, delta_tx);

            tokio::pin!(completion);
            loop {
                tokio::select! {
                    result = &mut completion => {
                        let completion = match result {
                            Ok(completion) => completion,
                            Err(error) => {
                                match self.handle_transport_retry(
                                    &error,
                                    &mut transport_retries_used,
                                    transport_budget,
                                    &event_tx,
                                ) {
                                    ModelRetryDecision::RetryQueued => break,
                                    ModelRetryDecision::Exhausted => return Err(error),
                                }
                            }
                        };
                        let finish_reason = completion.finish_reason.as_deref();
                        match self.adapter.parse_response(&completion) {
                            AssistantOutput::Final(text) => {
                                if text.trim().is_empty() {
                                    match self.handle_empty_model_response(
                                        &mut empty_retries_used,
                                        empty_response_budget,
                                        &event_tx,
                                    ) {
                                        ModelRetryDecision::RetryQueued => break,
                                        ModelRetryDecision::Exhausted => {
                                            self.emit_empty_model_response(&event_tx);
                                            return Ok(());
                                        }
                                    }
                                }
                                if crp_enabled {
                                    match self.handle_crp_validation(
                                        &text,
                                        finish_reason,
                                        &mut crp_retries_used,
                                        crp_budget,
                                        &event_tx,
                                    ) {
                                        CrpValidationDecision::Accept => {}
                                        CrpValidationDecision::RetryQueued => break,
                                        CrpValidationDecision::Exhausted => {}
                                    }
                                }
                                let message = Message::assistant_final(text);
                                self.history.push(message.clone());
                                let _ = event_tx.send(TurnEvent::Message(message));
                                return Ok(());
                            }
                            AssistantOutput::ToolCall { recipient, arguments } => {
                                if tool_turns_used >= max_tool_turns {
                                    let message = Message::assistant_final(format!(
                                        "I hit the configured tool-call budget ({max_tool_turns}) before producing a final answer. Try asking for a narrower step, or raise `harness.max_tool_turns` in settings/config."
                                    ));
                                    self.history.push(message.clone());
                                    let _ = event_tx.send(TurnEvent::Message(message));
                                    return Ok(());
                                }
                                tool_turns_used += 1;

                                let _ = event_tx.send(TurnEvent::DiscardAssistantDraft);
                                let call = Message::assistant_tool_call(recipient.clone(), arguments.clone());
                                self.history.push(call.clone());
                                let _ = event_tx.send(TurnEvent::Message(call));

                                let approved = self.approve_tool_if_needed(
                                    &recipient,
                                    &arguments,
                                    &event_tx,
                                ).await;
                                let output = if approved {
                                    self.execute_tool(&recipient, &arguments)
                                        .unwrap_or_else(|error| format!("tool error: {error:#}"))
                                } else {
                                    format!("tool blocked: user rejected {}", clean_recipient(&recipient))
                                };
                                let output = compact_tool_context(&recipient, &output);
                                let tool = Message::tool(recipient, output);
                                self.history.push(tool.clone());
                                let _ = event_tx.send(TurnEvent::Message(tool));
                                break;
                            }
                            AssistantOutput::Raw(text) => {
                                if text.trim().is_empty() {
                                    match self.handle_empty_model_response(
                                        &mut empty_retries_used,
                                        empty_response_budget,
                                        &event_tx,
                                    ) {
                                        ModelRetryDecision::RetryQueued => break,
                                        ModelRetryDecision::Exhausted => {
                                            self.emit_empty_model_response(&event_tx);
                                            return Ok(());
                                        }
                                    }
                                }
                                if crp_enabled {
                                    match self.handle_crp_validation(
                                        &text,
                                        finish_reason,
                                        &mut crp_retries_used,
                                        crp_budget,
                                        &event_tx,
                                    ) {
                                        CrpValidationDecision::Accept => {}
                                        CrpValidationDecision::RetryQueued => break,
                                        CrpValidationDecision::Exhausted => {}
                                    }
                                }
                                let message = Message::assistant_final(text);
                                self.history.push(message.clone());
                                let _ = event_tx.send(TurnEvent::Message(message));
                                return Ok(());
                            }
                        }
                    }
                    Some(delta) = delta_rx.recv() => {
                        let _ = event_tx.send(TurnEvent::AssistantDelta(delta));
                    }
                }
            }
        }
    }

    fn handle_empty_model_response(
        &mut self,
        retries_used: &mut u32,
        budget: u32,
        event_tx: &UnboundedSender<TurnEvent>,
    ) -> ModelRetryDecision {
        if *retries_used >= budget {
            return ModelRetryDecision::Exhausted;
        }

        *retries_used += 1;
        let attempt = *retries_used;
        let crp_enabled = self
            .config
            .harness
            .reasoning_protocol
            .eq_ignore_ascii_case("crp");
        self.history
            .push(Message::user(empty_response_retry_message(crp_enabled)));

        let _ = event_tx.send(TurnEvent::ModelRetryRequested {
            attempt,
            budget,
            reason: "The model returned an empty response.".to_string(),
        });
        ModelRetryDecision::RetryQueued
    }

    fn handle_transport_retry(
        &self,
        error: &anyhow::Error,
        retries_used: &mut u32,
        budget: u32,
        event_tx: &UnboundedSender<TurnEvent>,
    ) -> ModelRetryDecision {
        if *retries_used >= budget {
            return ModelRetryDecision::Exhausted;
        }

        *retries_used += 1;
        let attempt = *retries_used;
        let detail = compact_inline(&format!("{error:#}"), 300);
        let _ = event_tx.send(TurnEvent::ModelRetryRequested {
            attempt,
            budget,
            reason: format!("Model request failed: {detail}. Retrying."),
        });
        ModelRetryDecision::RetryQueued
    }

    fn emit_empty_model_response(&self, event_tx: &UnboundedSender<TurnEvent>) {
        let _ = event_tx.send(TurnEvent::EmptyModelResponse {
            format: self.config.model.format.clone(),
            model: self.config.model.model.clone(),
        });
    }

    fn handle_crp_validation(
        &mut self,
        text: &str,
        finish_reason: Option<&str>,
        retries_used: &mut u32,
        budget: u32,
        event_tx: &UnboundedSender<TurnEvent>,
    ) -> CrpValidationDecision {
        let workspace = self.config.harness.workspace.clone();
        let template = self.templates.resolve(
            &self.config.harness.default_template,
            &self.config.model.thinking_effort,
        );
        let config = template.validation_config(Some(workspace.as_path()));

        let (mut retry_message, invalid_slots, parse_error) = match crp::parse(text) {
            Ok(trace) => {
                let report = crp::validate(&trace, &config);
                if report.is_executable() {
                    return CrpValidationDecision::Accept;
                }
                let invalid: Vec<String> = report
                    .invalid()
                    .map(|outcome| outcome.slot.clone())
                    .collect();
                (crp::build_retry_message(&report), invalid, None)
            }
            Err(error) => (
                crp::build_parse_retry_message(&error),
                Vec::new(),
                Some(error.to_string()),
            ),
        };
        retry_message = add_finish_reason_retry_guidance(retry_message, finish_reason);

        if *retries_used >= budget {
            let _ = event_tx.send(TurnEvent::CrpRetryExhausted {
                budget,
                invalid_slots,
            });
            return CrpValidationDecision::Exhausted;
        }

        *retries_used += 1;
        let attempt = *retries_used;

        let assistant = Message::assistant_final(text.to_string());
        self.history.push(assistant);
        self.history.push(Message::user(retry_message));

        let _ = event_tx.send(TurnEvent::CrpRetryRequested {
            attempt,
            budget,
            invalid_slots,
            parse_error,
        });
        CrpValidationDecision::RetryQueued
    }

    async fn compact_context_if_needed(&mut self) -> Option<TurnEvent> {
        if !self.config.harness.auto_context_compression {
            return None;
        }

        // Use the model client to dynamically determine the available context window size, 
        // falling back to a configured value if the client cannot report it.
        let context_window = self.client.get_available_context_window().await.max(1) as usize;
        let threshold_percent = self
            .config
            .harness
            .context_compression_threshold
            .clamp(10, 100) as usize;
        let threshold_tokens = (context_window * threshold_percent / 100).max(1);
        let estimated_tokens = self.estimated_prompt_tokens();
        if estimated_tokens < threshold_tokens {
            return None;
        }

        let keep_recent = self.config.harness.context_compression_keep_recent.max(4) as usize;
        if self.history.len() <= keep_recent + 2 {
            return None;
        }

        let original_messages = self.history.len();
        let mut split_at = self.history.len().saturating_sub(keep_recent);
        while split_at < self.history.len() && self.history[split_at].role == Role::Tool {
            split_at += 1;
        }
        if split_at == 0 || split_at >= self.history.len() {
            return None;
        }

        let compacted = build_context_summary(&self.history[..split_at]);
        let kept = self.history[split_at..].to_vec();
        let kept_messages = kept.len();

        self.history.clear();
        self.history.push(Message::user(compacted));
        self.history.extend(kept);

        Some(TurnEvent::ContextCompacted {
            original_messages,
            kept_messages,
            estimated_tokens,
            threshold_tokens,
        })
    }

    fn execute_tool(&mut self, recipient: &str, arguments: &str) -> Result<String> {
        let recipient = clean_recipient(recipient);
        let tool_name = recipient
            .strip_prefix("functions.")
            .unwrap_or(recipient.as_str());
        match tool_name {
            "list_files" => self.list_files(arguments),
            "read_file" => self.read_file(arguments),
            "write_file" => self.write_file(arguments),
            "delete_file" => self.delete_file(arguments),
            "search" => self.search(arguments),
            "todo_read" => Ok(self.todo_details()),
            "todo_write" => self.todo_write(arguments),
            _ => Ok(format!("unsupported tool: {recipient}")),
        }
    }

    async fn approve_tool_if_needed(
        &self,
        recipient: &str,
        arguments: &str,
        event_tx: &UnboundedSender<TurnEvent>,
    ) -> bool {
        if !self.config.harness.require_edit_approval || !is_mutating_tool(recipient) {
            return true;
        }

        let (response_tx, response_rx) = oneshot::channel();
        let request = TurnEvent::ToolApprovalRequested {
            recipient: clean_recipient(recipient),
            summary: tool_approval_summary(recipient, arguments),
            response_tx,
        };

        if event_tx.send(request).is_err() {
            return false;
        }

        response_rx.await.unwrap_or(false)
    }

    fn list_files(&self, arguments: &str) -> Result<String> {
        let value = parse_json(arguments)?;
        let path = value.get("path").and_then(Value::as_str).unwrap_or(".");
        let path = self.workspace_path(path)?;
        let mut entries = fs::read_dir(&path)
            .with_context(|| format!("failed to list {}", path.display()))?
            .filter_map(Result::ok)
            .map(|entry| {
                let file_type = entry.file_type().ok();
                let suffix = if file_type.is_some_and(|kind| kind.is_dir()) {
                    "/"
                } else {
                    ""
                };
                format!("{}{}", entry.file_name().to_string_lossy(), suffix)
            })
            .collect::<Vec<_>>();

        entries.sort();
        Ok(entries.join("\n"))
    }

    fn read_file(&self, arguments: &str) -> Result<String> {
        let value = parse_json(arguments)?;
        let path = value
            .get("path")
            .and_then(Value::as_str)
            .context("read_file requires path")?;
        let path = self.workspace_path(path)?;
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(format!(
            "path: {}\nbytes: {}\nlines: {}\n\n{}",
            path.display(),
            contents.len(),
            contents.lines().count(),
            contents
        ))
    }

    fn write_file(&self, arguments: &str) -> Result<String> {
        let value = parse_json(arguments)?;
        let path = value
            .get("path")
            .and_then(Value::as_str)
            .context("write_file requires path")?;
        let contents = value
            .get("content")
            .or_else(|| value.get("contents"))
            .and_then(Value::as_str)
            .context("write_file requires content")?;
        let path = self.workspace_write_path(path)?;
        let existed = path.exists();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;

        let action = if existed { "overwrote" } else { "created" };
        Ok(format!(
            "{action}: {}\nbytes: {}\nlines: {}",
            path.display(),
            contents.len(),
            contents.lines().count()
        ))
    }

    fn delete_file(&self, arguments: &str) -> Result<String> {
        let value = parse_json(arguments)?;
        let path = value
            .get("path")
            .and_then(Value::as_str)
            .context("delete_file requires path")?;
        let path = self.workspace_path(path)?;
        let metadata =
            fs::metadata(&path).with_context(|| format!("failed to inspect {}", path.display()))?;
        if !metadata.is_file() {
            return Err(anyhow!(
                "delete_file only deletes regular files: {}",
                path.display()
            ));
        }

        fs::remove_file(&path).with_context(|| format!("failed to delete {}", path.display()))?;
        Ok(format!("deleted: {}", path.display()))
    }

    fn search(&self, arguments: &str) -> Result<String> {
        let value = parse_json(arguments)?;
        let query = value
            .get("query")
            .and_then(Value::as_str)
            .context("search requires query")?;
        let path = value.get("path").and_then(Value::as_str).unwrap_or(".");
        let path = self.workspace_path(path)?;

        let output = Command::new("rg")
            .arg("--line-number")
            .arg("--hidden")
            .arg("--glob")
            .arg("!.git")
            .arg("--glob")
            .arg("!.cinto")
            .arg("--")
            .arg(query)
            .arg(path)
            .output()
            .context("failed to run rg")?;

        if output.status.success() || output.status.code() == Some(1) {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    fn todo_write(&mut self, arguments: &str) -> Result<String> {
        let update: TodoUpdate =
            serde_json::from_str(arguments.trim()).context("todo_write arguments must be JSON")?;

        self.todos = update
            .items
            .into_iter()
            .map(TodoItem::try_from)
            .collect::<Result<Vec<_>>>()?;

        Ok(format_todos(&self.todos))
    }

    fn workspace_write_path(&self, requested: &str) -> Result<PathBuf> {
        let path = self.workspace_path(requested)?;
        let workspace = self
            .config
            .harness
            .workspace
            .canonicalize()
            .with_context(|| {
                format!(
                    "failed to resolve workspace {}",
                    self.config.harness.workspace.display()
                )
            })?;

        let parent = path
            .parent()
            .context("write_file path must include a file name")?;
        let parent = nearest_existing_parent(parent)?;
        let parent = parent
            .canonicalize()
            .with_context(|| format!("failed to resolve parent {}", parent.display()))?;
        if !parent.starts_with(&workspace) {
            return Err(anyhow!("path escapes workspace: {requested}"));
        }

        Ok(path)
    }

    fn workspace_path(&self, requested: &str) -> Result<PathBuf> {
        let requested_path = Path::new(requested);
        if requested_path.is_absolute() {
            return Err(anyhow!("absolute paths are not allowed: {requested}"));
        }

        for part in requested_path.components() {
            match part {
                Component::ParentDir => {
                    return Err(anyhow!(
                        "parent directory traversal is not allowed: {requested}"
                    ));
                }
                Component::Normal(name) if is_protected_workspace_component(name) => {
                    return Err(anyhow!(
                        "protected workspace path is not allowed: {requested}"
                    ));
                }
                _ => {}
            }
        }

        let workspace = self
            .config
            .harness
            .workspace
            .canonicalize()
            .with_context(|| {
                format!(
                    "failed to resolve workspace {}",
                    self.config.harness.workspace.display()
                )
            })?;
        let joined = workspace.join(requested_path);

        if joined.exists() {
            let canonical = joined
                .canonicalize()
                .with_context(|| format!("failed to resolve {}", joined.display()))?;
            if !canonical.starts_with(&workspace) {
                return Err(anyhow!("path escapes workspace: {requested}"));
            }
            Ok(canonical)
        } else {
            Ok(joined)
        }
    }
}

fn fallback_adapter(config: &Config) -> Box<dyn PromptAdapter> {
    let mut fallback = config.clone();
    fallback.model.format = "harmony".to_string();
    build_adapter(&fallback).unwrap_or_else(|_| {
        Box::new(crate::adapter::HarmonyAdapter::new(
            config.harness.system_prompt.clone(),
            config.harness.developer_prompt.clone(),
        ))
    })
}

fn is_protected_workspace_component(name: &std::ffi::OsStr) -> bool {
    matches!(name.to_str(), Some(".git" | ".cinto"))
}

fn nearest_existing_parent(path: &Path) -> Result<PathBuf> {
    for parent in path.ancestors() {
        if parent.exists() {
            return Ok(parent.to_path_buf());
        }
    }
    Err(anyhow!(
        "failed to find an existing parent for {}",
        path.display()
    ))
}

fn parse_json(arguments: &str) -> Result<Value> {
    serde_json::from_str(arguments.trim()).context("tool arguments must be JSON")
}

fn empty_response_retry_message(crp_enabled: bool) -> String {
    if crp_enabled {
        return "<RETRY_REASON>\nThe previous model response was empty. Re-emit either one valid tool call or a complete CRP trace with a non-empty <FINAL_RESPONSE> slot. Do not return an empty response.\n</RETRY_REASON>\n".to_string();
    }

    "The previous model response was empty. Please respond with either one valid tool call or a concise final answer.".to_string()
}

fn add_finish_reason_retry_guidance(message: String, finish_reason: Option<&str>) -> String {
    let Some(reason) = finish_reason.filter(|reason| is_truncated_finish_reason(reason)) else {
        return message;
    };

    format!(
        "<RETRY_REASON>\nThe previous model response stopped with finish_reason `{reason}`, so it was likely truncated by the output token limit. Re-emit a shorter complete CRP trace with only essential slots and a non-empty <FINAL_RESPONSE>.\n</RETRY_REASON>\n\n{message}"
    )
}

fn is_truncated_finish_reason(reason: &str) -> bool {
    matches!(
        reason.trim().to_ascii_lowercase().as_str(),
        "length" | "max_tokens" | "model_length"
    )
}

fn is_mutating_tool(recipient: &str) -> bool {
    let recipient = clean_recipient(recipient);
    let tool_name = recipient
        .strip_prefix("functions.")
        .unwrap_or(recipient.as_str());
    matches!(tool_name, "write_file" | "delete_file")
}

fn tool_approval_summary(recipient: &str, arguments: &str) -> String {
    let recipient = clean_recipient(recipient);
    let tool_name = recipient
        .strip_prefix("functions.")
        .unwrap_or(recipient.as_str());
    let path = serde_json::from_str::<Value>(arguments.trim())
        .ok()
        .and_then(|value| {
            value
                .get("path")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "?".to_string());

    match tool_name {
        "write_file" => format!("write file {path}"),
        "delete_file" => format!("delete file {path}"),
        _ => format!("run {recipient}"),
    }
}

fn compact_tool_context(recipient: &str, output: &str) -> String {
    let line_count = output.lines().count();
    let char_count = output.chars().count();
    if line_count <= MAX_TOOL_CONTEXT_LINES && char_count <= MAX_TOOL_CONTEXT_CHARS {
        return format!("{output}\n\n{TOOL_OUTPUT_END}");
    }

    let lines = output.lines().collect::<Vec<_>>();
    let head_len = 80.min(lines.len());
    let tail_len = 30.min(lines.len().saturating_sub(head_len));
    let omitted = lines.len().saturating_sub(head_len + tail_len);

    let mut compact = format!(
        "Cinto compacted a large tool result before adding it to model context.\nrecipient: {}\noriginal: {} lines / {} chars\nincluded: first {} lines + last {} lines\nIf you need details from the omitted middle, use search or request a narrower file/range.\n\n--- first lines ---\n",
        clean_recipient(recipient),
        line_count,
        char_count,
        head_len,
        tail_len
    );

    for line in lines.iter().take(head_len) {
        compact.push_str(line);
        compact.push('\n');
    }

    compact.push_str(&format!("--- omitted {omitted} lines ---\n"));

    if tail_len > 0 {
        compact.push_str("--- last lines ---\n");
        for line in lines.iter().skip(lines.len() - tail_len) {
            compact.push_str(line);
            compact.push('\n');
        }
    }

    compact.push('\n');
    compact.push_str(TOOL_OUTPUT_END);
    compact
}

fn build_context_summary(messages: &[Message]) -> String {
    let mut summary = format!(
        "{CONTEXT_COMPACTED_MARKER}\nCinto automatically compacted earlier model context.\nCompacted messages: {}\nRecent messages after this note remain exact.\n\nEarlier transcript outline:\n",
        messages.len()
    );
    let mut omitted = 0usize;

    for message in messages {
        let line = summarize_message(message);
        let next_len = summary.chars().count() + line.chars().count() + 3;
        if next_len > MAX_CONTEXT_SUMMARY_CHARS {
            omitted += 1;
            continue;
        }
        summary.push_str("- ");
        summary.push_str(&line);
        summary.push('\n');
    }

    if omitted > 0 {
        summary.push_str(&format!("- ... {omitted} older outline entries omitted\n"));
    }

    summary.trim_end().to_string()
}

fn summarize_message(message: &Message) -> String {
    let role = match message.role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    };
    let channel = match message.channel {
        Some(Channel::Analysis) => " analysis",
        Some(Channel::Commentary) => " commentary",
        Some(Channel::Final) => " final",
        None => "",
    };
    let recipient = message
        .recipient
        .as_deref()
        .map(clean_recipient)
        .map(|value| format!(" to {value}"))
        .unwrap_or_default();

    // For assistant final messages, try to extract structured CRP slot summaries
    // instead of blindly truncating the entire trace.
    if message.role == Role::Assistant && message.channel == Some(Channel::Final) {
        if let Ok(trace) = crp::parse(&message.content) {
            return summarize_crp_trace(&trace, role, channel, &recipient);
        }
    }

    let content = compact_inline(&message.content, 1000);
    format!("{role}{channel}{recipient}: {content}")
}

fn summarize_crp_trace(trace: &crp::Trace, role: &str, channel: &str, recipient: &str) -> String {
    let mut parts = Vec::new();

    if let Some(task) = trace.get("TASK_INTERPRETATION") {
        parts.push(format!("task: {}", compact_inline(&task.content, 300)));
    }
    if let Some(work) = trace.get("WORK_DONE") {
        parts.push(format!("work: {}", compact_inline(&work.content, 500)));
    }
    if let Some(response) = trace.get("FINAL_RESPONSE") {
        parts.push(format!(
            "response: {}",
            compact_inline(&response.content, 800)
        ));
    }

    let other_slots: Vec<&str> = trace
        .slots
        .iter()
        .filter(|s| s.name != "TASK_INTERPRETATION" && s.name != "FINAL_RESPONSE")
        .map(|s| s.name.as_str())
        .collect();
    if !other_slots.is_empty() {
        parts.push(format!("also emitted: {}", other_slots.join(", ")));
    }

    if parts.is_empty() {
        return format!(
            "{role}{channel}{recipient}: {}",
            compact_inline(
                &trace
                    .slots
                    .first()
                    .map(|s| s.content.as_str())
                    .unwrap_or("(empty CRP trace)"),
                240
            )
        );
    }

    format!("{role}{channel}{recipient}: {}", parts.join(" | "))
}

fn compact_inline(value: &str, max_chars: usize) -> String {
    let one_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max_chars {
        return one_line;
    }

    let mut compact = one_line.chars().take(max_chars).collect::<String>();
    compact.push_str("...");
    compact
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TodoItem {
    title: String,
    status: TodoStatus,
    detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TodoStatus {
    Pending,
    InProgress,
    Done,
    Blocked,
}

#[derive(Debug, Deserialize)]
struct TodoUpdate {
    items: Vec<TodoInput>,
}

#[derive(Debug, Deserialize)]
struct TodoInput {
    title: String,
    status: String,
    #[serde(default)]
    detail: Option<String>,
}

impl TryFrom<TodoInput> for TodoItem {
    type Error = anyhow::Error;

    fn try_from(input: TodoInput) -> Result<Self> {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err(anyhow!("todo title cannot be empty"));
        }

        let detail = input
            .detail
            .map(|detail| detail.trim().to_string())
            .filter(|detail| !detail.is_empty());

        Ok(Self {
            title,
            status: TodoStatus::try_from(input.status.as_str())?,
            detail,
        })
    }
}

impl TryFrom<&str> for TodoStatus {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value.trim() {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "done" => Ok(Self::Done),
            "blocked" => Ok(Self::Blocked),
            other => Err(anyhow!(
                "todo status must be pending, in_progress, done, or blocked, got {other}"
            )),
        }
    }
}

impl TodoStatus {
    fn marker(self) -> &'static str {
        match self {
            Self::Pending => "[ ]",
            Self::InProgress => "[*]",
            Self::Done => "[x]",
            Self::Blocked => "[!]",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }
}

fn format_todos(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "No todo list is set for the current task.".to_string();
    }

    let mut output = String::from("Current task todo list\n\n");
    for (index, item) in todos.iter().enumerate() {
        output.push_str(&format!(
            "{} {}. {} ({})\n",
            item.status.marker(),
            index + 1,
            item.title,
            item.status.label()
        ));

        if let Some(detail) = &item.detail {
            output.push_str("    ");
            output.push_str(detail);
            output.push('\n');
        }
    }

    output.trim_end().to_string()
}

fn clean_recipient(recipient: &str) -> String {
    recipient
        .split("<|")
        .next()
        .unwrap_or(recipient)
        .split_whitespace()
        .next()
        .unwrap_or(recipient)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discard_trailing_user_message_only_removes_unanswered_user() {
        let mut session = AgentSession::new(Config::default());
        session.history.push(Message::user("cancel me"));

        assert!(session.discard_trailing_user_message());
        assert!(session.history().is_empty());

        session
            .history
            .push(Message::assistant_final("keep assistant"));
        assert!(!session.discard_trailing_user_message());
        assert_eq!(session.history().len(), 1);
    }

    #[test]
    fn write_file_creates_nested_workspace_file() {
        let workspace =
            std::env::temp_dir().join(format!("cinto-write-file-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("create test workspace");

        let mut config = Config::default();
        config.harness.workspace = workspace.clone();
        let mut session = AgentSession::new(config);
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let args = serde_json::json!({
            "path": "src/bin/hello_world.rs",
            "content": content,
        })
        .to_string();

        let output = session
            .execute_tool("functions.write_file", &args)
            .expect("write file");
        let written = workspace.join("src/bin/hello_world.rs");

        assert!(output.contains("created:"));
        assert_eq!(
            fs::read_to_string(&written).expect("read written file"),
            content
        );

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn read_file_rejects_git_internals() {
        let workspace =
            std::env::temp_dir().join(format!("cinto-read-git-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(workspace.join(".git")).expect("create fake git dir");
        fs::write(workspace.join(".git/config"), "secret").expect("write fake git config");

        let mut config = Config::default();
        config.harness.workspace = workspace.clone();
        let session = AgentSession::new(config);
        let args = serde_json::json!({
            "path": ".git/config",
        })
        .to_string();

        let error = session
            .read_file(&args)
            .expect_err("protected path should fail");

        assert!(error.to_string().contains("protected workspace path"));

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn write_file_rejects_cinto_internals() {
        let workspace =
            std::env::temp_dir().join(format!("cinto-write-internal-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("create workspace");

        let mut config = Config::default();
        config.harness.workspace = workspace.clone();
        let session = AgentSession::new(config);
        let args = serde_json::json!({
            "path": ".cinto/checkpoints/owned.patch",
            "content": "nope",
        })
        .to_string();

        let error = session
            .write_file(&args)
            .expect_err("protected path should fail");

        assert!(error.to_string().contains("protected workspace path"));

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn execute_tool_accepts_short_write_file_name() {
        let workspace = std::env::temp_dir().join(format!(
            "cinto-short-write-file-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("create test workspace");

        let mut config = Config::default();
        config.harness.workspace = workspace.clone();
        let mut session = AgentSession::new(config);
        let args = serde_json::json!({
            "path": "short.rs",
            "content": "fn main() {}\n",
        })
        .to_string();

        let output = session
            .execute_tool("write_file", &args)
            .expect("write file with short name");

        assert!(output.contains("created:"));
        assert!(workspace.join("short.rs").exists());

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn delete_file_removes_workspace_file() {
        let workspace =
            std::env::temp_dir().join(format!("cinto-delete-file-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("create test workspace");
        let target = workspace.join("hello.rs");
        fs::write(&target, "fn main() {}\n").expect("write target");

        let mut config = Config::default();
        config.harness.workspace = workspace.clone();
        let mut session = AgentSession::new(config);
        let args = serde_json::json!({
            "path": "hello.rs",
        })
        .to_string();

        let output = session
            .execute_tool("functions.delete_file", &args)
            .expect("delete file");

        assert!(output.contains("deleted:"));
        assert!(!target.exists());

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn write_file_rejects_parent_traversal() {
        let workspace = std::env::temp_dir().join(format!(
            "cinto-write-file-traversal-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("create test workspace");

        let mut config = Config::default();
        config.harness.workspace = workspace.clone();
        let mut session = AgentSession::new(config);
        let args = serde_json::json!({
            "path": "../escape.rs",
            "content": "nope",
        })
        .to_string();

        let error = session
            .execute_tool("functions.write_file", &args)
            .expect_err("parent traversal should fail");

        assert!(error.to_string().contains("parent directory traversal"));

        let _ = fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn auto_context_compression_keeps_recent_history() {
        let mut config = Config::default();
        config.model.context_window = 80;
        config.harness.context_compression_threshold = 10;
        config.harness.context_compression_keep_recent = 4;
        let mut session = AgentSession::new(config);

        for index in 0..12 {
            session.history.push(Message::user(format!(
                "old user message {index} {}",
                "x".repeat(80)
            )));
            session.history.push(Message::assistant_final(format!(
                "old assistant message {index} {}",
                "y".repeat(80)
            )));
        }

        let event = session
            .compact_context_if_needed()
            .await
            .expect("context should compact");

        assert!(matches!(event, TurnEvent::ContextCompacted { .. }));
        assert!(
            session.history[0]
                .content
                .contains(CONTEXT_COMPACTED_MARKER)
        );
        assert!(
            session.history[0]
                .content
                .contains("Earlier transcript outline")
        );
        assert_eq!(session.history.len(), 5);
        assert!(session.history[1].content.contains("old user message 10"));
        assert!(
            session.history[4]
                .content
                .contains("old assistant message 11")
        );
    }

    #[tokio::test]
    async fn auto_context_compression_does_not_start_tail_with_tool_result() {
        let mut config = Config::default();
        config.model.context_window = 80;
        config.harness.context_compression_threshold = 10;
        config.harness.context_compression_keep_recent = 4;
        let mut session = AgentSession::new(config);

        for index in 0..8 {
            session
                .history
                .push(Message::user(format!("message {index} {}", "x".repeat(80))));
        }
        session.history.push(Message::assistant_tool_call(
            "functions.read_file",
            "{\"path\":\"x\"}",
        ));
        session
            .history
            .push(Message::tool("functions.read_file", "contents"));
        session.history.push(Message::assistant_final("done"));

        session
            .compact_context_if_needed()
            .await
            .expect("context should compact");

        assert_ne!(session.history[1].role, Role::Tool);
    }

    #[test]
    fn handle_crp_validation_no_op_for_valid_trace() {
        let mut config = Config::default();
        config.harness.workspace = std::env::current_dir().unwrap();
        let mut session = AgentSession::new(config);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 0u32;

        let trace = "\
<TASK_INTERPRETATION>test</TASK_INTERPRETATION>
<RELEVANT_FILES>
- Cargo.toml
</RELEVANT_FILES>
<PROPOSED_APPROACH>
- do it
</PROPOSED_APPROACH>
<FILE_EDITS>
@@ Cargo.toml prepend
fn hello() {}
</FILE_EDITS>
<FINAL_RESPONSE>All good.</FINAL_RESPONSE>";

        let decision = session.handle_crp_validation(trace, None, &mut retries, 3, &tx);

        assert_eq!(decision, CrpValidationDecision::Accept);
        assert_eq!(retries, 0);
        assert!(rx.try_recv().is_err());
        assert!(session.history.is_empty());
    }

    #[test]
    fn handle_crp_validation_accepts_final_response_only_trace() {
        let mut config = Config::default();
        config.harness.workspace = std::env::current_dir().unwrap();
        let mut session = AgentSession::new(config);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 0u32;

        let decision = session.handle_crp_validation(
            "<FINAL_RESPONSE>Done without extra structure.</FINAL_RESPONSE>",
            None,
            &mut retries,
            3,
            &tx,
        );

        assert_eq!(decision, CrpValidationDecision::Accept);
        assert_eq!(retries, 0);
        assert!(rx.try_recv().is_err());
        assert!(session.history.is_empty());
    }

    #[test]
    fn handle_crp_validation_pushes_retry_when_final_response_missing() {
        let mut config = Config::default();
        config.harness.workspace = std::env::current_dir().unwrap();
        let mut session = AgentSession::new(config);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 0u32;

        let decision = session.handle_crp_validation(
            "<TASK_INTERPRETATION>x</TASK_INTERPRETATION>",
            None,
            &mut retries,
            3,
            &tx,
        );

        assert_eq!(decision, CrpValidationDecision::RetryQueued);
        assert_eq!(retries, 1);
        assert_eq!(session.history.len(), 2);
        assert_eq!(session.history[0].role, Role::Assistant);
        assert_eq!(session.history[1].role, Role::User);
        assert!(session.history[1].content.contains("<RETRY_REASON>"));
        assert!(session.history[1].content.contains("FINAL_RESPONSE"));

        match rx.try_recv().expect("event") {
            TurnEvent::CrpRetryRequested {
                attempt,
                budget,
                invalid_slots,
                parse_error,
            } => {
                assert_eq!(attempt, 1);
                assert_eq!(budget, 3);
                assert!(invalid_slots.contains(&"FINAL_RESPONSE".to_string()));
                assert!(parse_error.is_none());
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn handle_crp_validation_emits_exhausted_when_budget_consumed() {
        let mut config = Config::default();
        config.harness.workspace = std::env::current_dir().unwrap();
        let mut session = AgentSession::new(config);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 3u32;

        let decision = session.handle_crp_validation(
            "<TASK_INTERPRETATION>x</TASK_INTERPRETATION>",
            None,
            &mut retries,
            3,
            &tx,
        );

        assert_eq!(decision, CrpValidationDecision::Exhausted);
        assert_eq!(retries, 3);
        assert!(session.history.is_empty());
        match rx.try_recv().expect("event") {
            TurnEvent::CrpRetryExhausted {
                budget,
                invalid_slots,
            } => {
                assert_eq!(budget, 3);
                assert!(invalid_slots.contains(&"FINAL_RESPONSE".to_string()));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn handle_crp_validation_retries_on_parse_failure() {
        let mut config = Config::default();
        config.harness.workspace = std::env::current_dir().unwrap();
        let mut session = AgentSession::new(config);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 0u32;

        let decision = session.handle_crp_validation("hello", None, &mut retries, 3, &tx);

        assert_eq!(decision, CrpValidationDecision::RetryQueued);
        assert_eq!(retries, 1);
        match rx.try_recv().expect("event") {
            TurnEvent::CrpRetryRequested { parse_error, .. } => {
                assert!(parse_error.is_some());
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn handle_crp_validation_adds_truncation_guidance() {
        let mut config = Config::default();
        config.harness.workspace = std::env::current_dir().unwrap();
        let mut session = AgentSession::new(config);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 0u32;

        let decision = session.handle_crp_validation("hello", Some("length"), &mut retries, 3, &tx);

        assert_eq!(decision, CrpValidationDecision::RetryQueued);
        assert_eq!(session.history.len(), 2);
        let retry = &session.history[1].content;
        assert!(retry.contains("finish_reason `length`"));
        assert!(retry.contains("shorter complete CRP trace"));
        assert!(retry.contains("FINAL_RESPONSE"));
    }

    #[test]
    fn handle_empty_model_response_queues_one_retry() {
        let mut session = AgentSession::new(Config::default());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 0u32;

        let decision = session.handle_empty_model_response(&mut retries, 1, &tx);

        assert_eq!(decision, ModelRetryDecision::RetryQueued);
        assert_eq!(retries, 1);
        assert_eq!(session.history.len(), 1);
        assert_eq!(session.history[0].role, Role::User);
        assert!(session.history[0].content.contains("<RETRY_REASON>"));
        match rx.try_recv().expect("event") {
            TurnEvent::ModelRetryRequested {
                attempt,
                budget,
                reason,
            } => {
                assert_eq!(attempt, 1);
                assert_eq!(budget, 1);
                assert!(reason.contains("empty response"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn handle_empty_model_response_exhausts_after_budget() {
        let mut session = AgentSession::new(Config::default());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 1u32;

        let decision = session.handle_empty_model_response(&mut retries, 1, &tx);

        assert_eq!(decision, ModelRetryDecision::Exhausted);
        assert_eq!(retries, 1);
        assert!(session.history.is_empty());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn handle_transport_retry_uses_separate_budget_without_history_mutation() {
        let session = AgentSession::new(Config::default());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut retries = 0u32;
        let error = anyhow!("connection closed");

        let decision = session.handle_transport_retry(&error, &mut retries, 1, &tx);

        assert_eq!(decision, ModelRetryDecision::RetryQueued);
        assert_eq!(retries, 1);
        assert!(session.history.is_empty());
        match rx.try_recv().expect("event") {
            TurnEvent::ModelRetryRequested {
                attempt,
                budget,
                reason,
            } => {
                assert_eq!(attempt, 1);
                assert_eq!(budget, 1);
                assert!(reason.contains("connection closed"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn summarize_message_extracts_crp_slots() {
        let msg = Message {
            role: Role::Assistant,
            channel: Some(Channel::Final),
            content: "<TASK_INTERPRETATION>Fix the bug in parser</TASK_INTERPRETATION>\n<PROPOSED_APPROACH>\n- Step one\n</PROPOSED_APPROACH>\n<FINAL_RESPONSE>Done, the parser now handles edge cases.</FINAL_RESPONSE>".to_string(),
            recipient: None,
        };

        let summary = summarize_message(&msg);

        assert!(summary.starts_with("assistant final:"));
        assert!(summary.contains("task: Fix the bug in parser"));
        assert!(summary.contains("response: Done, the parser now handles edge cases."));
        assert!(summary.contains("also emitted: PROPOSED_APPROACH"));
        // Should NOT contain the full PROPOSED_APPROACH content
        assert!(!summary.contains("Step one"));
    }

    #[test]
    fn summarize_message_falls_back_for_non_crp() {
        let msg = Message {
            role: Role::Assistant,
            channel: Some(Channel::Final),
            content: "Just a plain text answer without CRP slots.".to_string(),
            recipient: None,
        };

        let summary = summarize_message(&msg);

        assert!(summary.starts_with("assistant final:"));
        assert!(summary.contains("Just a plain text answer"));
    }

    #[test]
    fn summarize_message_user_messages_unchanged() {
        let msg = Message {
            role: Role::User,
            channel: None,
            content: "Please fix the bug.".to_string(),
            recipient: None,
        };

        let summary = summarize_message(&msg);

        assert_eq!(summary, "user: Please fix the bug.");
    }
}
