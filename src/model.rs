use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::adapter::{
    ApiFunctionCall, ApiMessage, ApiToolCall, ApiToolSpec, ChatRequestPayload, CompletionResult,
    RequestMode,
};
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

    pub async fn complete(&self, payload: ChatRequestPayload) -> Result<CompletionResult> {
        let endpoint = normalize_endpoint(&self.config.endpoint, &payload.mode);
        match payload.mode {
            RequestMode::HarmonyText { prompt } => {
                if endpoint.ends_with("/chat/completions") {
                    self.harmony_chat_complete(endpoint, prompt, false, None)
                        .await
                } else {
                    self.text_complete(endpoint, prompt, false, None).await
                }
            }
            RequestMode::OpenAiChat { messages, tools } => {
                self.openai_chat_complete(endpoint, messages, tools, false, None)
                    .await
            }
        }
    }

    pub async fn complete_streaming(
        &self,
        payload: ChatRequestPayload,
        delta_tx: UnboundedSender<String>,
    ) -> Result<CompletionResult> {
        if !self.config.stream {
            return self.complete(payload).await;
        }

        let endpoint = normalize_endpoint(&self.config.endpoint, &payload.mode);
        match payload.mode {
            RequestMode::HarmonyText { prompt } => {
                if endpoint.ends_with("/chat/completions") {
                    self.harmony_chat_complete(endpoint, prompt, true, Some(delta_tx))
                        .await
                } else {
                    self.text_complete(endpoint, prompt, true, Some(delta_tx))
                        .await
                }
            }
            RequestMode::OpenAiChat { messages, tools } => {
                self.openai_chat_complete(endpoint, messages, tools, true, Some(delta_tx))
                    .await
            }
        }
    }

    async fn text_complete(
        &self,
        endpoint: String,
        prompt: String,
        stream: bool,
        delta_tx: Option<UnboundedSender<String>>,
    ) -> Result<CompletionResult> {
        let request = CompletionRequest {
            model: self.config.model.clone(),
            prompt,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            reasoning_effort: self.reasoning_effort(),
            stop: self.config.stop.clone(),
            stream,
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

        if stream {
            collect_stream(response, &endpoint, delta_tx).await
        } else {
            let body: CompletionResponse = response
                .json()
                .await
                .context("failed to decode completion response")?;
            let text = body
                .choices
                .into_iter()
                .next()
                .map(|choice| choice.text)
                .context("completion response had no choices")?;
            Ok(CompletionResult {
                text,
                tool_calls: Vec::new(),
            })
        }
    }

    async fn harmony_chat_complete(
        &self,
        endpoint: String,
        prompt: String,
        stream: bool,
        delta_tx: Option<UnboundedSender<String>>,
    ) -> Result<CompletionResult> {
        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![ChatMessage::user(prompt)],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            reasoning_effort: self.reasoning_effort(),
            stop: self.config.stop.clone(),
            stream,
            tools: None,
            tool_choice: None,
        };
        self.send_chat(endpoint, request, stream, delta_tx).await
    }

    async fn openai_chat_complete(
        &self,
        endpoint: String,
        messages: Vec<ApiMessage>,
        tools: Vec<ApiToolSpec>,
        stream: bool,
        delta_tx: Option<UnboundedSender<String>>,
    ) -> Result<CompletionResult> {
        let messages = messages.into_iter().map(ChatMessage::Api).collect();
        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            reasoning_effort: None,
            stop: self.config.stop.clone(),
            stream,
            tools: if tools.is_empty() { None } else { Some(tools) },
            tool_choice: Some("auto".to_string()),
        };
        self.send_chat(endpoint, request, stream, delta_tx).await
    }

    async fn send_chat(
        &self,
        endpoint: String,
        request: ChatCompletionRequest,
        stream: bool,
        delta_tx: Option<UnboundedSender<String>>,
    ) -> Result<CompletionResult> {
        let response = self
            .authorize(self.http.post(&endpoint))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("failed to call {endpoint}"))?;
        let response = require_success(response, &endpoint).await?;

        if stream {
            collect_stream(response, &endpoint, delta_tx).await
        } else {
            let body: ChatCompletionResponse = response
                .json()
                .await
                .context("failed to decode chat completion response")?;
            let choice = body
                .choices
                .into_iter()
                .next()
                .context("chat completion response had no choices")?;
            Ok(CompletionResult {
                text: choice.message.content.unwrap_or_default(),
                tool_calls: choice.message.tool_calls.unwrap_or_default(),
            })
        }
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

fn normalize_endpoint(endpoint: &str, mode: &RequestMode) -> String {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.ends_with("/v1/completions") || endpoint.ends_with("/v1/chat/completions") {
        return endpoint.to_string();
    }
    let suffix = match mode {
        RequestMode::OpenAiChat { .. } => "/v1/chat/completions",
        RequestMode::HarmonyText { .. } => "/v1/chat/completions",
    };
    if endpoint.ends_with("/v1") {
        return format!("{endpoint}{}", &suffix[3..]);
    }
    format!("{endpoint}{suffix}")
}

async fn collect_stream(
    response: reqwest::Response,
    endpoint: &str,
    delta_tx: Option<UnboundedSender<String>>,
) -> Result<CompletionResult> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full = String::new();
    let mut tool_calls: Vec<PartialToolCall> = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("failed while streaming from {endpoint}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline) = buffer.find('\n') {
            let line = buffer.drain(..=newline).collect::<String>();
            process_sse_line(line.trim(), delta_tx.as_ref(), &mut full, &mut tool_calls)?;
        }
    }

    if !buffer.trim().is_empty() {
        process_sse_line(buffer.trim(), delta_tx.as_ref(), &mut full, &mut tool_calls)?;
    }

    Ok(CompletionResult {
        text: full,
        tool_calls: tool_calls
            .into_iter()
            .filter_map(PartialToolCall::finish)
            .collect(),
    })
}

fn process_sse_line(
    line: &str,
    delta_tx: Option<&UnboundedSender<String>>,
    full: &mut String,
    tool_calls: &mut Vec<PartialToolCall>,
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
    let choice = value.get("choices").and_then(|choices| choices.get(0));
    if let Some(choice) = choice {
        if let Some(delta) = extract_delta(choice) {
            if !delta.is_empty() {
                full.push_str(&delta);
                if let Some(tx) = delta_tx {
                    let _ = tx.send(delta);
                }
            }
        }
        accumulate_tool_calls(choice, tool_calls);

        // Non-streamed payloads can come back through a single SSE-shaped line in
        // some servers — capture a complete `message.tool_calls` array as well.
        if let Some(message) = choice.get("message") {
            if let Some(arr) = message.get("tool_calls").and_then(|v| v.as_array()) {
                for (index, call) in arr.iter().enumerate() {
                    upsert_tool_call(tool_calls, index, call);
                }
            }
        }
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

fn accumulate_tool_calls(choice: &serde_json::Value, tool_calls: &mut Vec<PartialToolCall>) {
    let Some(delta) = choice.get("delta") else {
        return;
    };
    let Some(arr) = delta.get("tool_calls").and_then(|v| v.as_array()) else {
        return;
    };
    for (fallback_index, call) in arr.iter().enumerate() {
        let index = call
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(fallback_index);
        upsert_tool_call(tool_calls, index, call);
    }
}

fn upsert_tool_call(tool_calls: &mut Vec<PartialToolCall>, index: usize, call: &serde_json::Value) {
    while tool_calls.len() <= index {
        tool_calls.push(PartialToolCall::default());
    }
    let slot = &mut tool_calls[index];

    if let Some(id) = call.get("id").and_then(serde_json::Value::as_str) {
        if !id.is_empty() {
            slot.id = Some(id.to_string());
        }
    }
    if let Some(kind) = call.get("type").and_then(serde_json::Value::as_str) {
        slot.kind = Some(kind.to_string());
    }
    if let Some(function) = call.get("function") {
        if let Some(name) = function.get("name").and_then(serde_json::Value::as_str) {
            if !name.is_empty() {
                slot.name = Some(name.to_string());
            }
        }
        if let Some(args) = function
            .get("arguments")
            .and_then(serde_json::Value::as_str)
        {
            slot.arguments.push_str(args);
        }
    }
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: Option<String>,
    kind: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl PartialToolCall {
    fn finish(self) -> Option<ApiToolCall> {
        let name = self.name?;
        Some(ApiToolCall {
            id: self.id.unwrap_or_else(|| "call_0".to_string()),
            kind: self.kind.unwrap_or_else(|| "function".to_string()),
            function: ApiFunctionCall {
                name,
                arguments: self.arguments,
            },
        })
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiToolSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ChatMessage {
    Simple { role: String, content: String },
    Api(ApiMessage),
}

impl ChatMessage {
    fn user(content: String) -> Self {
        ChatMessage::Simple {
            role: "user".to_string(),
            content,
        }
    }
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
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_lm_studio_base_url() {
        let mode = RequestMode::HarmonyText {
            prompt: String::new(),
        };
        assert_eq!(
            normalize_endpoint("http://127.0.0.1:1234", &mode),
            "http://127.0.0.1:1234/v1/chat/completions"
        );
        assert_eq!(
            normalize_endpoint("http://127.0.0.1:1234/v1", &mode),
            "http://127.0.0.1:1234/v1/chat/completions"
        );
        assert_eq!(
            normalize_endpoint("http://127.0.0.1:1234/v1/completions", &mode),
            "http://127.0.0.1:1234/v1/completions"
        );
    }
}
