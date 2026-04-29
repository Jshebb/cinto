use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use serde_json::Value;

use crate::{
    config::Config,
    harmony::{AssistantOutput, HarmonyPrompt, parse_assistant_output},
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
        }
    }

    pub fn clear(&mut self) {
        self.history.clear();
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

    pub async fn send_user_message(&mut self, content: String) -> Result<Vec<Message>> {
        self.history.push(Message::user(content));
        self.complete_turn().await
    }

    async fn complete_turn(&mut self) -> Result<Vec<Message>> {
        let mut produced = Vec::new();

        for _ in 0..4 {
            let prompt = self.render_prompt();
            let completion = self.client.complete(prompt).await?;

            match parse_assistant_output(&completion) {
                AssistantOutput::Final(text) => {
                    let message = Message::assistant_final(text);
                    self.history.push(message.clone());
                    produced.push(message);
                    return Ok(produced);
                }
                AssistantOutput::ToolCall {
                    recipient,
                    arguments,
                } => {
                    let call = Message::assistant_tool_call(recipient.clone(), arguments.clone());
                    self.history.push(call.clone());
                    produced.push(call);

                    let output = self.execute_tool(&recipient, &arguments)?;
                    let tool = Message::tool(recipient, output);
                    self.history.push(tool.clone());
                    produced.push(tool);
                }
                AssistantOutput::Raw(text) => {
                    let message = Message::assistant_final(text);
                    self.history.push(message.clone());
                    produced.push(message);
                    return Ok(produced);
                }
            }
        }

        Err(anyhow!("tool loop exceeded maximum turn depth"))
    }

    fn execute_tool(&self, recipient: &str, arguments: &str) -> Result<String> {
        match recipient {
            "functions.list_files" => self.list_files(arguments),
            "functions.read_file" => self.read_file(arguments),
            "functions.search" => self.search(arguments),
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
