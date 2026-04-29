use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};

use crate::config::ModelConfig;

#[derive(Debug, Clone)]
pub struct ModelClient {
    http: Client,
    config: ModelConfig,
}

impl ModelClient {
    pub fn new(config: ModelConfig) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { http, config }
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
            .authorize(self.http.post(&self.config.endpoint))
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

    fn authorize(&self, request: RequestBuilder) -> RequestBuilder {
        let Some(env_name) = self.config.api_key_env.as_deref() else {
            return request;
        };

        match std::env::var(env_name) {
            Ok(api_key) if !api_key.trim().is_empty() => request.bearer_auth(api_key),
            _ => request,
        }
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
