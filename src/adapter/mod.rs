use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::session::Message;

mod harmony;
mod openai;

pub use harmony::HarmonyAdapter;
pub use openai::OpenAiAdapter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantOutput {
    Final(String),
    ToolCall {
        recipient: String,
        arguments: String,
    },
    Raw(String),
}

#[derive(Debug, Clone)]
pub struct ChatRequestPayload {
    pub mode: RequestMode,
}

#[derive(Debug, Clone)]
pub enum RequestMode {
    HarmonyText {
        prompt: String,
    },
    OpenAiChat {
        messages: Vec<ApiMessage>,
        tools: Vec<ApiToolSpec>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ApiFunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiToolSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ApiToolFunction,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Default, Clone)]
pub struct CompletionResult {
    pub text: String,
    pub tool_calls: Vec<ApiToolCall>,
}

pub trait PromptAdapter: Send + Sync + std::fmt::Debug {
    fn render_request(&self, history: &[Message]) -> ChatRequestPayload;
    fn parse_response(&self, raw: &CompletionResult) -> AssistantOutput;
    fn debug_render(&self, history: &[Message]) -> String;
    fn tool_details(&self) -> String;
}

pub fn build_adapter(config: &Config) -> Result<Box<dyn PromptAdapter>> {
    let (system_prompt, developer_prompt) = prompt_pair(config);
    match config.model.format.trim().to_ascii_lowercase().as_str() {
        "harmony" => Ok(Box::new(HarmonyAdapter::new(
            system_prompt,
            developer_prompt,
        ))),
        "openai-tools" => Ok(Box::new(OpenAiAdapter::new(
            system_prompt,
            developer_prompt,
        ))),
        other => Err(anyhow!(
            "unknown model.format `{other}`; expected `harmony` or `openai-tools`"
        )),
    }
}

const AGENTS_FILE: &str = "AGENTS.md";
const MAX_AGENTS_CHARS: usize = 24_000;

fn prompt_pair(config: &Config) -> (String, String) {
    let system_prompt = config.harness.system_prompt.clone();
    let mut developer_prompt = config.harness.developer_prompt.clone();

    if let Ok(Some(agents)) = load_agents_instructions(&config.harness.workspace) {
        developer_prompt.push_str("\n\n");
        developer_prompt.push_str(&agents);
    }

    (system_prompt, developer_prompt)
}

fn load_agents_instructions(workspace: &Path) -> Result<Option<String>> {
    let path = workspace.join(AGENTS_FILE);
    if !path.exists() {
        return Ok(None);
    }
    if !path.is_file() {
        return Ok(None);
    }

    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let char_count = contents.chars().count();
    let mut trimmed = contents.trim().to_string();
    let truncated = char_count > MAX_AGENTS_CHARS;
    if truncated {
        trimmed = contents.chars().take(MAX_AGENTS_CHARS).collect::<String>();
        trimmed.push_str("\n\n[AGENTS.md truncated by Cinto]");
    }

    Ok(Some(format!(
        "Workspace instructions from AGENTS.md:\n\n{}",
        trimmed.trim()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_agents_md_into_developer_prompt() {
        let workspace =
            std::env::temp_dir().join(format!("cinto-agents-md-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("create workspace");
        fs::write(
            workspace.join(AGENTS_FILE),
            "# Project Instructions\n\nRun `cargo test`.",
        )
        .expect("write agents");

        let mut config = Config::default();
        config.harness.workspace = workspace.clone();
        config.harness.developer_prompt = "Base developer prompt.".to_string();

        let (_, developer_prompt) = prompt_pair(&config);

        assert!(developer_prompt.contains("Base developer prompt."));
        assert!(developer_prompt.contains("Workspace instructions from AGENTS.md"));
        assert!(developer_prompt.contains("Run `cargo test`."));

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn ignores_missing_agents_md() {
        let workspace = std::env::temp_dir().join(format!(
            "cinto-missing-agents-md-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("create workspace");

        let mut config = Config::default();
        config.harness.workspace = workspace.clone();
        config.harness.developer_prompt = "Only base.".to_string();

        let (_, developer_prompt) = prompt_pair(&config);

        assert_eq!(developer_prompt, "Only base.");

        let _ = fs::remove_dir_all(&workspace);
    }
}
