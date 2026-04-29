use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

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
        let endpoint = normalize_endpoint(&self.config.endpoint);
        if endpoint.ends_with("/chat/completions") {
            return self.chat_complete(endpoint, prompt).await;
        }

        self.text_complete(endpoint, prompt).await
    }

    pub async fn complete_streaming(
        &self,
        prompt: String,
        delta_tx: UnboundedSender<String>,
    ) -> Result<String> {
        if !self.config.stream {
            return self.complete(prompt).await;
        }

        let endpoint = normalize_endpoint(&self.config.endpoint);
        if endpoint.ends_with("/chat/completions") {
            return self
                .chat_complete_streaming(endpoint, prompt, delta_tx)
                .await;
        }

        self.text_complete_streaming(endpoint, prompt, delta_tx)
            .await
    }

    async fn text_complete(&self, endpoint: String, prompt: String) -> Result<String> {
        let request = CompletionRequest {
            model: self.config.model.clone(),
            prompt,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            reasoning_effort: self.reasoning_effort(),
            stop: self.config.stop.clone(),
            stream: false,
            skip_special_tokens: false,
            spaces_between_special_tokens: false,
        };

        let response = self
            .authorize(self.http.post(&endpoint))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("failed to call {endpoint}"))?;

        let response = require_success(response, &endpoint).await?;

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

    async fn text_complete_streaming(
        &self,
        endpoint: String,
        prompt: String,
        delta_tx: UnboundedSender<String>,
    ) -> Result<String> {
        let request = CompletionRequest {
            model: self.config.model.clone(),
            prompt,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            reasoning_effort: self.reasoning_effort(),
            stop: self.config.stop.clone(),
            stream: true,
            skip_special_tokens: false,
            spaces_between_special_tokens: false,
        };

        let response = self
            .authorize(self.http.post(&endpoint))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("failed to call {endpoint}"))?;

        let response = require_success(response, &endpoint).await?;
        collect_stream(response, &endpoint, delta_tx).await
    }

    async fn chat_complete(&self, endpoint: String, prompt: String) -> Result<String> {
        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            reasoning_effort: self.reasoning_effort(),
            stop: self.config.stop.clone(),
            stream: false,
        };

        let response = self
            .authorize(self.http.post(&endpoint))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("failed to call {endpoint}"))?;

        let response = require_success(response, &endpoint).await?;
        let body: ChatCompletionResponse = response
            .json()
            .await
            .context("failed to decode chat completion response")?;

        body.choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .context("chat completion response had no choices")
    }

    async fn chat_complete_streaming(
        &self,
        endpoint: String,
        prompt: String,
        delta_tx: UnboundedSender<String>,
    ) -> Result<String> {
        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            reasoning_effort: self.reasoning_effort(),
            stop: self.config.stop.clone(),
            stream: true,
        };

        let response = self
            .authorize(self.http.post(&endpoint))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("failed to call {endpoint}"))?;

        let response = require_success(response, &endpoint).await?;
        collect_stream(response, &endpoint, delta_tx).await
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

    fn reasoning_effort(&self) -> Option<String> {
        let value = self.config.thinking_effort.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(value.to_string())
        }
    }
}

async fn require_success(response: reqwest::Response, endpoint: &str) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<failed to read error body>".to_string());
    bail!("model server returned {status} from {endpoint}: {body}");
}

fn normalize_endpoint(endpoint: &str) -> String {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.ends_with("/v1/completions") || endpoint.ends_with("/v1/chat/completions") {
        return endpoint.to_string();
    }
    if endpoint.ends_with("/v1") {
        return format!("{endpoint}/chat/completions");
    }
    format!("{endpoint}/v1/chat/completions")
}

async fn collect_stream(
    response: reqwest::Response,
    endpoint: &str,
    delta_tx: UnboundedSender<String>,
) -> Result<String> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("failed while streaming from {endpoint}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline) = buffer.find('\n') {
            let line = buffer.drain(..=newline).collect::<String>();
            process_sse_line(line.trim(), &delta_tx, &mut full)?;
        }
    }

    if !buffer.trim().is_empty() {
        process_sse_line(buffer.trim(), &delta_tx, &mut full)?;
    }

    Ok(full)
}

fn process_sse_line(
    line: &str,
    delta_tx: &UnboundedSender<String>,
    full: &mut String,
) -> Result<()> {
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(());
    };
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return Ok(());
    }

    let value: serde_json::Value =
        serde_json::from_str(data).context("failed to parse stream chunk")?;
    let delta = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(extract_delta)
        .unwrap_or_default();

    if !delta.is_empty() {
        full.push_str(&delta);
        let _ = delta_tx.send(delta);
    }

    Ok(())
}

fn extract_delta(choice: &serde_json::Value) -> Option<String> {
    choice
        .get("delta")
        .and_then(|delta| delta.get("content"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| choice.get("text").and_then(serde_json::Value::as_str))
        .map(ToString::to_string)
}

#[derive(Debug, Serialize)]
struct CompletionRequest {
    model: String,
    prompt: String,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
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

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    stop: Vec<String>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::normalize_endpoint;

    #[test]
    fn normalizes_lm_studio_base_url() {
        assert_eq!(
            normalize_endpoint("http://127.0.0.1:1234"),
            "http://127.0.0.1:1234/v1/chat/completions"
        );
        assert_eq!(
            normalize_endpoint("http://127.0.0.1:1234/v1"),
            "http://127.0.0.1:1234/v1/chat/completions"
        );
        assert_eq!(
            normalize_endpoint("http://127.0.0.1:1234/v1/completions"),
            "http://127.0.0.1:1234/v1/completions"
        );
    }
}
