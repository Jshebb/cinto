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
    pub max_tokens: u32,
    pub temperature: f32,
    pub stop: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub workspace: PathBuf,
    pub allow_shell: bool,
    pub system_prompt: String,
    pub developer_prompt: String,
}

impl Default for Config {
    fn default() -> Self {
        let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            model: ModelConfig {
                endpoint: "http://127.0.0.1:8000/v1/completions".to_string(),
                model: "openai/gpt-oss-20b".to_string(),
                max_tokens: 4096,
                temperature: 0.2,
                stop: vec!["<|return|>".to_string(), "<|call|>".to_string()],
            },
            harness: HarnessConfig {
                workspace,
                allow_shell: false,
                system_prompt: "You are OpenHarness, a local coding agent running in a terminal UI. You help the user understand and modify the current workspace.".to_string(),
                developer_prompt: "Use concise reasoning, ask before destructive actions, and prefer small verifiable edits. When you need repository context, request tools in the commentary channel.".to_string(),
            },
        }
    }
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let Some(path) = path.or_else(default_config_path) else {
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
}

fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("openharness").join("config.toml"))
}
