use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};

use crate::config::Config;

#[derive(Debug, Clone, Copy)]
pub(super) enum SettingField {
    Endpoint,
    Model,
    Format,
    ReasoningProtocol,
    ApiKeyEnv,
    MaxTokens,
    Temperature,
    ThinkingEffort,
    Stream,
    Timeout,
    ContextWindow,
    AutoContextCompression,
    ContextCompressionThreshold,
    ContextCompressionKeepRecent,
    WorkspaceInstructions,
    ToolTurns,
    ModelRounds,
    CrpRetries,
    TransportRetries,
    EmptyResponseRetries,
    Stop,
    Workspace,
    EditApproval,
    AllowShell,
    SystemPrompt,
    DeveloperPrompt,
}

pub(super) const SETTINGS: [SettingField; 26] = [
    SettingField::Endpoint,
    SettingField::Model,
    SettingField::Format,
    SettingField::ReasoningProtocol,
    SettingField::ApiKeyEnv,
    SettingField::MaxTokens,
    SettingField::Temperature,
    SettingField::ThinkingEffort,
    SettingField::Stream,
    SettingField::Timeout,
    SettingField::ContextWindow,
    SettingField::AutoContextCompression,
    SettingField::ContextCompressionThreshold,
    SettingField::ContextCompressionKeepRecent,
    SettingField::WorkspaceInstructions,
    SettingField::ToolTurns,
    SettingField::ModelRounds,
    SettingField::CrpRetries,
    SettingField::TransportRetries,
    SettingField::EmptyResponseRetries,
    SettingField::Stop,
    SettingField::Workspace,
    SettingField::EditApproval,
    SettingField::AllowShell,
    SettingField::SystemPrompt,
    SettingField::DeveloperPrompt,
];

const THINKING_EFFORTS: [&str; 4] = ["none", "low", "medium", "high"];
const FORMATS: [&str; 2] = ["harmony", "openai-tools"];
const REASONING_PROTOCOLS: [&str; 2] = ["crp", "plain"];

impl SettingField {
    pub(super) fn label(self) -> &'static str {
        match self {
            SettingField::Endpoint => "endpoint",
            SettingField::Model => "model",
            SettingField::Format => "format",
            SettingField::ReasoningProtocol => "reasoning",
            SettingField::ApiKeyEnv => "api key env",
            SettingField::MaxTokens => "max tokens",
            SettingField::Temperature => "temperature",
            SettingField::ThinkingEffort => "thinking",
            SettingField::Stream => "stream",
            SettingField::Timeout => "timeout secs",
            SettingField::ContextWindow => "context window",
            SettingField::AutoContextCompression => "auto context compression",
            SettingField::ContextCompressionThreshold => "context compression %",
            SettingField::ContextCompressionKeepRecent => "context keep recent",
            SettingField::WorkspaceInstructions => "workspace instructions",
            SettingField::ToolTurns => "tool turns",
            SettingField::ModelRounds => "model rounds",
            SettingField::CrpRetries => "crp retries",
            SettingField::TransportRetries => "transport retries",
            SettingField::EmptyResponseRetries => "empty response retries",
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
            SettingField::Format => config.model.format.clone(),
            SettingField::ReasoningProtocol => config.harness.reasoning_protocol.clone(),
            SettingField::ApiKeyEnv => config.model.api_key_env.clone().unwrap_or_default(),
            SettingField::MaxTokens => config.model.max_tokens.to_string(),
            SettingField::Temperature => format!("{:.2}", config.model.temperature),
            SettingField::ThinkingEffort => config.model.thinking_effort.clone(),
            SettingField::Stream => config.model.stream.to_string(),
            SettingField::Timeout => config.model.request_timeout_secs.to_string(),
            SettingField::ContextWindow => config.model.context_window.to_string(),
            SettingField::AutoContextCompression => {
                config.harness.auto_context_compression.to_string()
            }
            SettingField::ContextCompressionThreshold => {
                config.harness.context_compression_threshold.to_string()
            }
            SettingField::ContextCompressionKeepRecent => {
                config.harness.context_compression_keep_recent.to_string()
            }
            SettingField::WorkspaceInstructions => {
                config.harness.load_workspace_instructions.to_string()
            }
            SettingField::ToolTurns => config.harness.max_tool_turns.to_string(),
            SettingField::ModelRounds => config.harness.max_model_rounds.to_string(),
            SettingField::CrpRetries => config.harness.crp_retry_budget.to_string(),
            SettingField::TransportRetries => config.harness.transport_retry_budget.to_string(),
            SettingField::EmptyResponseRetries => {
                config.harness.empty_response_retry_budget.to_string()
            }
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
            SettingField::Format => {
                let value = value.trim().to_ascii_lowercase();
                if !FORMATS.contains(&value.as_str()) {
                    return Err(anyhow!("format must be one of harmony, openai-tools"));
                }
                config.model.format = value;
            }
            SettingField::ReasoningProtocol => {
                let value = value.trim().to_ascii_lowercase();
                if !REASONING_PROTOCOLS.contains(&value.as_str()) {
                    return Err(anyhow!("reasoning must be one of crp, plain"));
                }
                config.harness.reasoning_protocol = value;
            }
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
            SettingField::ContextWindow => {
                config.model.context_window = parse_number(&value, "context window")?;
            }
            SettingField::AutoContextCompression => {
                config.harness.auto_context_compression = value
                    .trim()
                    .parse()
                    .context("auto context compression must be true or false")?;
            }
            SettingField::ContextCompressionThreshold => {
                let threshold = parse_number(&value, "context compression %")?;
                if !(10..=100).contains(&threshold) {
                    return Err(anyhow!("context compression % must be between 10 and 100"));
                }
                config.harness.context_compression_threshold = threshold;
            }
            SettingField::ContextCompressionKeepRecent => {
                config.harness.context_compression_keep_recent =
                    parse_number(&value, "context keep recent")?;
            }
            SettingField::WorkspaceInstructions => {
                config.harness.load_workspace_instructions = value
                    .trim()
                    .parse()
                    .context("workspace instructions must be true or false")?;
            }
            SettingField::ToolTurns => {
                config.harness.max_tool_turns = parse_number(&value, "tool turns")?;
            }
            SettingField::ModelRounds => {
                config.harness.max_model_rounds = parse_number(&value, "model rounds")?;
            }
            SettingField::CrpRetries => {
                config.harness.crp_retry_budget = parse_number(&value, "crp retries")?;
            }
            SettingField::TransportRetries => {
                config.harness.transport_retry_budget = parse_number(&value, "transport retries")?;
            }
            SettingField::EmptyResponseRetries => {
                config.harness.empty_response_retry_budget =
                    parse_number(&value, "empty response retries")?;
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
                config.harness.system_prompt =
                    unescape_newlines(&non_empty(value, "system prompt")?);
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

pub(super) fn next_format(current: &str) -> String {
    let current = current.trim().to_ascii_lowercase();
    let index = FORMATS
        .iter()
        .position(|format| *format == current)
        .unwrap_or(0);
    FORMATS[(index + 1) % FORMATS.len()].to_string()
}

pub(super) fn next_reasoning_protocol(current: &str) -> String {
    let current = current.trim().to_ascii_lowercase();
    let index = REASONING_PROTOCOLS
        .iter()
        .position(|protocol| *protocol == current)
        .unwrap_or(0);
    REASONING_PROTOCOLS[(index + 1) % REASONING_PROTOCOLS.len()].to_string()
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
