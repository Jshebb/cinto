use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    config::Config,
    harmony::{AssistantOutput, HarmonyPrompt, available_tool_details, parse_assistant_output},
    model::ModelClient,
};

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

#[derive(Debug, Clone)]
pub enum TurnEvent {
    AssistantDelta(String),
    DiscardAssistantDraft,
    Message(Message),
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
    prompt: HarmonyPrompt,
    client: ModelClient,
    history: Vec<Message>,
    todos: Vec<TodoItem>,
}

impl AgentSession {
    pub fn new(config: Config) -> Self {
        let prompt = HarmonyPrompt::new(
            config.harness.system_prompt.clone(),
            config.harness.developer_prompt.clone(),
        );
        let client = ModelClient::new(config.model.clone());

        Self {
            config,
            prompt,
            client,
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
        self.prompt = HarmonyPrompt::new(
            config.harness.system_prompt.clone(),
            config.harness.developer_prompt.clone(),
        );
        self.client = ModelClient::new(config.model.clone());
        self.config = config;
    }

    pub fn render_prompt(&self) -> String {
        self.prompt.render(&self.history)
    }

    pub fn estimated_prompt_tokens(&self) -> usize {
        self.render_prompt().len() / 4
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn tool_details(&self) -> String {
        available_tool_details()
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

        for _ in 0..max_tool_turns {
            let prompt = self.render_prompt();
            let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel();
            let client = self.client.clone();
            let completion = client.complete_streaming(prompt, delta_tx);

            tokio::pin!(completion);
            loop {
                tokio::select! {
                    result = &mut completion => {
                        let completion = result?;
                        match parse_assistant_output(&completion) {
                            AssistantOutput::Final(text) => {
                                let message = Message::assistant_final(text);
                                self.history.push(message.clone());
                                let _ = event_tx.send(TurnEvent::Message(message));
                                return Ok(());
                            }
                            AssistantOutput::ToolCall { recipient, arguments } => {
                                let _ = event_tx.send(TurnEvent::DiscardAssistantDraft);
                                let call = Message::assistant_tool_call(recipient.clone(), arguments.clone());
                                self.history.push(call.clone());
                                let _ = event_tx.send(TurnEvent::Message(call));

                                let output = self
                                    .execute_tool(&recipient, &arguments)
                                    .unwrap_or_else(|error| format!("tool error: {error:#}"));
                                let tool = Message::tool(recipient, output);
                                self.history.push(tool.clone());
                                let _ = event_tx.send(TurnEvent::Message(tool));
                                break;
                            }
                            AssistantOutput::Raw(text) => {
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

        let message = Message::assistant_final(format!(
            "I hit the configured tool-call budget ({max_tool_turns}) before producing a final answer. Try asking for a narrower step, or raise `harness.max_tool_turns` in settings/config."
        ));
        self.history.push(message.clone());
        let _ = event_tx.send(TurnEvent::Message(message));
        Ok(())
    }

    fn execute_tool(&mut self, recipient: &str, arguments: &str) -> Result<String> {
        let recipient = clean_recipient(recipient);
        match recipient.as_str() {
            "functions.list_files" => self.list_files(arguments),
            "functions.read_file" => self.read_file(arguments),
            "functions.search" => self.search(arguments),
            "functions.todo_read" => Ok(self.todo_details()),
            "functions.todo_write" => self.todo_write(arguments),
            _ => Ok(format!("unsupported tool: {recipient}")),
        }
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
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
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

    fn workspace_path(&self, requested: &str) -> Result<PathBuf> {
        let requested_path = Path::new(requested);
        if requested_path.is_absolute() {
            return Err(anyhow!("absolute paths are not allowed: {requested}"));
        }

        if requested_path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(anyhow!(
                "parent directory traversal is not allowed: {requested}"
            ));
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

fn parse_json(arguments: &str) -> Result<Value> {
    serde_json::from_str(arguments.trim()).context("tool arguments must be JSON")
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
