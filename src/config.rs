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
    #[serde(default)]
    pub api_key_env: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub stop: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    pub request_timeout_secs: u64,
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
                api_key_env: None,
                max_tokens: 4096,
                temperature: 0.2,
                stop: vec!["<|return|>".to_string(), "<|call|>".to_string()],
                request_timeout_secs: default_timeout_secs(),
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
        dirs::config_dir().map(|dir| dir.join("openharness").join("config.toml"))
    }
}

fn default_timeout_secs() -> u64 {
    600
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_config() {
        let path = std::env::temp_dir().join(format!(
            "openharness-config-test-{}.toml",
            std::process::id()
        ));
        let mut config = Config::default();
        config.model.endpoint = "http://localhost:9000/v1/completions".to_string();
        config.model.api_key_env = Some("OPENHARNESS_TEST_KEY".to_string());

        let saved = config.save(Some(path.clone())).expect("save config");
        let loaded = Config::load(Some(path.clone())).expect("load config");

        assert_eq!(saved, path);
        assert_eq!(loaded.model.endpoint, config.model.endpoint);
        assert_eq!(loaded.model.api_key_env, config.model.api_key_env);

        let _ = fs::remove_file(path);
    }
}
