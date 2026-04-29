use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::ModelConfig;

#[derive(Debug, Clone)]
pub struct ModelClient {
    http: Client,
    config: ModelConfig,
}

impl ModelClient {
    pub fn new(config: ModelConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    pub async fn complete(&self, prompt: String) -> Result<String> {
        let request = CompletionRequest {
            model: self.config.model.clone(),
            prompt,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            stop: self.config.stop.clone(),
            stream: false,
            skip_special_tokens: false,
            spaces_between_special_tokens: false,
        };

        let response = self
            .http
            .post(&self.config.endpoint)
            .json(&request)
            .send()
            .await
            .with_context(|| format!("failed to call {}", self.config.endpoint))?
            .error_for_status()
            .with_context(|| {
                format!(
                    "model server returned an error from {}",
                    self.config.endpoint
                )
            })?;

        let body: CompletionResponse = response
            .json()
            .await
            .context("failed to decode completion response")?;

        body.choices
            .into_iter()
            .next()
            .map(|choice| choice.text)
            .context("completion response had no choices")
    }
}

#[derive(Debug, Serialize)]
struct CompletionRequest {
    model: String,
    prompt: String,
    max_tokens: u32,
    temperature: f32,
    stop: Vec<String>,
    stream: bool,
    skip_special_tokens: bool,
    spaces_between_special_tokens: bool,
}

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct CompletionChoice {
    text: String,
}
