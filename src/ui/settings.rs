use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};

use crate::config::Config;

#[derive(Debug, Clone, Copy)]
pub(super) enum SettingField {
    Endpoint,
    Model,
    ApiKeyEnv,
    MaxTokens,
    Temperature,
    ThinkingEffort,
    Stream,
    Timeout,
    ToolTurns,
    Stop,
    Workspace,
    EditApproval,
    AllowShell,
    SystemPrompt,
    DeveloperPrompt,
}

pub(super) const SETTINGS: [SettingField; 15] = [
    SettingField::Endpoint,
    SettingField::Model,
    SettingField::ApiKeyEnv,
    SettingField::MaxTokens,
    SettingField::Temperature,
    SettingField::ThinkingEffort,
    SettingField::Stream,
    SettingField::Timeout,
    SettingField::ToolTurns,
    SettingField::Stop,
    SettingField::Workspace,
    SettingField::EditApproval,
    SettingField::AllowShell,
    SettingField::SystemPrompt,
    SettingField::DeveloperPrompt,
];

const THINKING_EFFORTS: [&str; 4] = ["none", "low", "medium", "high"];

impl SettingField {
    pub(super) fn label(self) -> &'static str {
        match self {
            SettingField::Endpoint => "endpoint",
            SettingField::Model => "model",
            SettingField::ApiKeyEnv => "api key env",
            SettingField::MaxTokens => "max tokens",
            SettingField::Temperature => "temperature",
            SettingField::ThinkingEffort => "thinking",
            SettingField::Stream => "stream",
            SettingField::Timeout => "timeout secs",
            SettingField::ToolTurns => "tool turns",
            SettingField::Stop => "stop",
            SettingField::Workspace => "workspace",
            SettingField::EditApproval => "edit approval",
            SettingField::AllowShell => "allow shell",
            SettingField::SystemPrompt => "system prompt",
            SettingField::DeveloperPrompt => "developer prompt",
        }
    }

    pub(super) fn value(self, config: &Config) -> String {
        match self {
            SettingField::Endpoint => config.model.endpoint.clone(),
            SettingField::Model => config.model.model.clone(),
            SettingField::ApiKeyEnv => config.model.api_key_env.clone().unwrap_or_default(),
            SettingField::MaxTokens => config.model.max_tokens.to_string(),
            SettingField::Temperature => format!("{:.2}", config.model.temperature),
            SettingField::ThinkingEffort => config.model.thinking_effort.clone(),
            SettingField::Stream => config.model.stream.to_string(),
            SettingField::Timeout => config.model.request_timeout_secs.to_string(),
            SettingField::ToolTurns => config.harness.max_tool_turns.to_string(),
            SettingField::Stop => config.model.stop.join(","),
            SettingField::Workspace => config.harness.workspace.display().to_string(),
            SettingField::EditApproval => config.harness.require_edit_approval.to_string(),
            SettingField::AllowShell => config.harness.allow_shell.to_string(),
            SettingField::SystemPrompt => escape_newlines(&config.harness.system_prompt),
            SettingField::DeveloperPrompt => escape_newlines(&config.harness.developer_prompt),
        }
    }

    pub(super) fn apply(self, config: &mut Config, value: String) -> Result<()> {
        match self {
            SettingField::Endpoint => config.model.endpoint = non_empty(value, "endpoint")?,
            SettingField::Model => config.model.model = non_empty(value, "model")?,
            SettingField::ApiKeyEnv => {
                let value = value.trim().to_string();
                config.model.api_key_env = if value.is_empty() { None } else { Some(value) };
            }
            SettingField::MaxTokens => {
                config.model.max_tokens = parse_number(&value, "max tokens")?;
            }
            SettingField::Temperature => {
                config.model.temperature = value
                    .trim()
                    .parse()
                    .context("temperature must be a number")?;
            }
            SettingField::ThinkingEffort => {
                let value = value.trim().to_ascii_lowercase();
                if !matches!(value.as_str(), "none" | "low" | "medium" | "high") {
                    return Err(anyhow!("thinking must be one of none, low, medium, high"));
                }
                config.model.thinking_effort = value;
            }
            SettingField::Stream => {
                config.model.stream = value
                    .trim()
                    .parse()
                    .context("stream must be true or false")?;
            }
            SettingField::Timeout => {
                config.model.request_timeout_secs = parse_number(&value, "timeout secs")?;
            }
            SettingField::ToolTurns => {
                config.harness.max_tool_turns = parse_number(&value, "tool turns")?;
            }
            SettingField::Stop => {
                config.model.stop = value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(ToString::to_string)
                    .collect();
            }
            SettingField::Workspace => {
                config.harness.workspace = PathBuf::from(non_empty(value, "workspace")?);
            }
            SettingField::EditApproval => {
                config.harness.require_edit_approval = value
                    .trim()
                    .parse()
                    .context("edit approval must be true or false")?;
            }
            SettingField::AllowShell => {
                config.harness.allow_shell = value
                    .trim()
                    .parse()
                    .context("allow shell must be true or false")?;
            }
            SettingField::SystemPrompt => {
                config.harness.system_prompt = unescape_newlines(&non_empty(value, "system prompt")?);
            }
            SettingField::DeveloperPrompt => {
                config.harness.developer_prompt =
                    unescape_newlines(&non_empty(value, "developer prompt")?);
            }
        }

        Ok(())
    }
}

fn escape_newlines(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n")
}

fn unescape_newlines(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

pub(super) fn next_thinking_effort(current: &str) -> String {
    let current = current.trim().to_ascii_lowercase();
    let index = THINKING_EFFORTS
        .iter()
        .position(|effort| *effort == current)
        .unwrap_or(1);
    THINKING_EFFORTS[(index + 1) % THINKING_EFFORTS.len()].to_string()
}

fn non_empty(value: String, label: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(anyhow!("{label} cannot be empty"));
    }
    Ok(value)
}

fn parse_number<T>(value: &str, label: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value
        .trim()
        .parse()
        .with_context(|| format!("{label} must be a number"))
}
