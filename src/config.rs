use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub model: ModelConfig,
    pub harness: HarnessConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub endpoint: String,
    pub model: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    #[serde(default = "default_thinking_effort")]
    pub thinking_effort: String,
    #[serde(default = "default_stream")]
    pub stream: bool,
    pub stop: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_context_window")]
    pub context_window: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub workspace: PathBuf,
    pub allow_shell: bool,
    #[serde(default = "default_require_edit_approval")]
    pub require_edit_approval: bool,
    #[serde(default = "default_max_tool_turns")]
    pub max_tool_turns: u32,
    #[serde(default = "default_auto_context_compression")]
    pub auto_context_compression: bool,
    #[serde(default = "default_context_compression_threshold")]
    pub context_compression_threshold: u32,
    #[serde(default = "default_context_compression_keep_recent")]
    pub context_compression_keep_recent: u32,
    #[serde(default = "default_load_workspace_instructions")]
    pub load_workspace_instructions: bool,
    pub system_prompt: String,
    pub developer_prompt: String,
}

impl Default for Config {
    fn default() -> Self {
        let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            model: ModelConfig {
                endpoint: "http://127.0.0.1:1234".to_string(),
                model: "openai/gpt-oss-20b".to_string(),
                format: default_format(),
                api_key_env: None,
                max_tokens: 4096,
                temperature: 0.2,
                thinking_effort: default_thinking_effort(),
                stream: default_stream(),
                stop: vec!["<|return|>".to_string(), "<|call|>".to_string()],
                request_timeout_secs: default_timeout_secs(),
                context_window: default_context_window(),
            },
            harness: HarnessConfig {
                workspace,
                allow_shell: false,
                require_edit_approval: default_require_edit_approval(),
                max_tool_turns: default_max_tool_turns(),
                auto_context_compression: default_auto_context_compression(),
                context_compression_threshold: default_context_compression_threshold(),
                context_compression_keep_recent: default_context_compression_keep_recent(),
                load_workspace_instructions: default_load_workspace_instructions(),
                system_prompt: "You are Cinto, a local coding agent running in a terminal UI. You help the user understand and modify the current workspace.".to_string(),
                developer_prompt: "Use concise reasoning, ask before destructive actions, and prefer small verifiable edits. When you need repository context, request tools in the commentary channel. Treat <CINTO_TOOL_OUTPUT_END> as the end of tool output; do not continue or echo tool output after that marker. If a tool result says it was compacted, use search or narrower reads for missing details. Treat <CINTO_CONTEXT_COMPACTED> as a concise note about earlier conversation history; rely on recent messages for exact details.".to_string(),
            },
        }
    }
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let Some(path) = path.or_else(Self::default_path) else {
            return Ok(Self::default());
        };

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        toml::from_str(&contents)
            .with_context(|| format!("failed to parse config at {}", path.display()))
    }

    pub fn save(&self, path: Option<PathBuf>) -> Result<PathBuf> {
        let path = path
            .or_else(Self::default_path)
            .context("could not determine a config path")?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let contents = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&path, contents)
            .with_context(|| format!("failed to write config at {}", path.display()))?;
        Ok(path)
    }

    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("cinto").join("config.toml"))
    }
}

fn default_timeout_secs() -> u64 {
    600
}

fn default_context_window() -> u32 {
    8192
}

fn default_max_tool_turns() -> u32 {
    16
}

fn default_auto_context_compression() -> bool {
    true
}

fn default_context_compression_threshold() -> u32 {
    80
}

fn default_context_compression_keep_recent() -> u32 {
    18
}

fn default_load_workspace_instructions() -> bool {
    true
}

fn default_require_edit_approval() -> bool {
    true
}

fn default_thinking_effort() -> String {
    "medium".to_string()
}

fn default_format() -> String {
    "harmony".to_string()
}

fn default_stream() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_config() {
        let path =
            std::env::temp_dir().join(format!("cinto-config-test-{}.toml", std::process::id()));
        let mut config = Config::default();
        config.model.endpoint = "http://localhost:9000/v1/completions".to_string();
        config.model.api_key_env = Some("CINTO_TEST_KEY".to_string());

        let saved = config.save(Some(path.clone())).expect("save config");
        let loaded = Config::load(Some(path.clone())).expect("load config");

        assert_eq!(saved, path);
        assert_eq!(loaded.model.endpoint, config.model.endpoint);
        assert_eq!(loaded.model.api_key_env, config.model.api_key_env);

        let _ = fs::remove_file(path);
    }
}
