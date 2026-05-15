use serde_json::Value;

use crate::session::{Channel, Message, Role};

use super::{
    ApiFunctionCall, ApiMessage, ApiToolCall, ApiToolFunction, ApiToolSpec, AssistantOutput,
    ChatRequestPayload, CompletionResult, PromptAdapter, RequestMode,
};

#[derive(Debug, Clone)]
pub struct OpenAiAdapter {
    system_prompt: String,
    developer_prompt: String,
}

impl OpenAiAdapter {
    pub fn new(system_prompt: impl Into<String>, developer_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            developer_prompt: developer_prompt.into(),
        }
    }

    fn build_messages(&self, history: &[Message]) -> Vec<ApiMessage> {
        let mut messages = Vec::with_capacity(history.len() + 1);

        let combined_system = if self.developer_prompt.trim().is_empty() {
            self.system_prompt.clone()
        } else {
            format!("{}\n\n{}", self.system_prompt, self.developer_prompt)
        };

        messages.push(ApiMessage {
            role: "system".to_string(),
            content: Some(combined_system),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        });

        let mut tool_call_index: u32 = 0;
        let mut last_tool_id_per_recipient: Vec<(String, String)> = Vec::new();

        for message in history {
            match (message.role, message.channel) {
                (Role::User, _) => messages.push(ApiMessage {
                    role: "user".to_string(),
                    content: Some(message.content.clone()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: None,
                }),
                (Role::Assistant, Some(Channel::Final)) | (Role::Assistant, None) => {
                    messages.push(ApiMessage {
                        role: "assistant".to_string(),
                        content: Some(message.content.clone()),
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                    });
                }
                (Role::Assistant, Some(Channel::Commentary)) => {
                    let recipient = message
                        .recipient
                        .clone()
                        .unwrap_or_else(|| "functions.unknown".to_string());
                    let function_name = function_name_from_recipient(&recipient);
                    let id = format!("call_{tool_call_index}");
                    tool_call_index += 1;
                    last_tool_id_per_recipient.push((recipient.clone(), id.clone()));

                    messages.push(ApiMessage {
                        role: "assistant".to_string(),
                        content: None,
                        name: None,
                        tool_call_id: None,
                        tool_calls: Some(vec![ApiToolCall {
                            id,
                            kind: "function".to_string(),
                            function: ApiFunctionCall {
                                name: function_name,
                                arguments: message.content.clone(),
                            },
                        }]),
                    });
                }
                (Role::Assistant, Some(Channel::Analysis)) => {
                    // Analysis is private chain-of-thought; do not forward to OpenAI history.
                }
                (Role::Tool, _) => {
                    let recipient = message
                        .recipient
                        .clone()
                        .unwrap_or_else(|| "functions.unknown".to_string());
                    let id = pop_matching_id(&mut last_tool_id_per_recipient, &recipient)
                        .unwrap_or_else(|| format!("call_{recipient}"));
                    let function_name = function_name_from_recipient(&recipient);
                    messages.push(ApiMessage {
                        role: "tool".to_string(),
                        content: Some(message.content.clone()),
                        name: Some(function_name),
                        tool_call_id: Some(id),
                        tool_calls: None,
                    });
                }
            }
        }

        messages
    }
}

fn function_name_from_recipient(recipient: &str) -> String {
    let cleaned = recipient
        .split("<|")
        .next()
        .unwrap_or(recipient)
        .split_whitespace()
        .next()
        .unwrap_or(recipient)
        .trim();
    cleaned
        .strip_prefix("functions.")
        .unwrap_or(cleaned)
        .to_string()
}

fn pop_matching_id(stack: &mut Vec<(String, String)>, recipient: &str) -> Option<String> {
    let needle = function_name_from_recipient(recipient);
    let position = stack
        .iter()
        .rposition(|(stored, _)| function_name_from_recipient(stored) == needle)?;
    Some(stack.remove(position).1)
}

impl PromptAdapter for OpenAiAdapter {
    fn render_request(&self, history: &[Message]) -> ChatRequestPayload {
        ChatRequestPayload {
            mode: RequestMode::OpenAiChat {
                messages: self.build_messages(history),
                tools: openai_tool_specs(),
            },
        }
    }

    fn parse_response(&self, raw: &CompletionResult) -> AssistantOutput {
        if let Some(call) = raw.tool_calls.first() {
            return AssistantOutput::ToolCall {
                recipient: format!("functions.{}", call.function.name),
                arguments: call.function.arguments.clone(),
            };
        }

        let trimmed = raw.text.trim();
        if trimmed.is_empty() {
            AssistantOutput::Raw(raw.text.clone())
        } else {
            AssistantOutput::Final(trimmed.to_string())
        }
    }

    fn debug_render(&self, history: &[Message]) -> String {
        let payload = openai_payload_preview(self.build_messages(history), openai_tool_specs());
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "<failed to serialize>".into())
    }

    fn tool_details(&self) -> String {
        let mut details = String::from("Available agent tools (OpenAI tool-calling)\n\n");
        for tool in openai_tool_specs() {
            details.push_str("**functions.");
            details.push_str(&tool.function.name);
            details.push_str("**\n");
            details.push_str(&tool.function.description);
            details.push_str("\nSchema: ");
            details.push_str(&tool.function.parameters.to_string());
            details.push_str("\n\n");
        }
        details.trim_end().to_string()
    }
}

#[derive(serde::Serialize)]
struct PayloadPreview {
    messages: Vec<ApiMessage>,
    tools: Vec<ApiToolSpec>,
}

fn openai_payload_preview(messages: Vec<ApiMessage>, tools: Vec<ApiToolSpec>) -> PayloadPreview {
    PayloadPreview { messages, tools }
}

fn openai_tool_specs() -> Vec<ApiToolSpec> {
    vec![
        spec(
            "list_files",
            "List immediate child entries under a relative workspace path. Directories are suffixed with `/`.",
            serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "additionalProperties": false
            }),
        ),
        spec(
            "read_file",
            "Read a UTF-8 text file from the workspace. The `path` must be relative and cannot escape the workspace.",
            serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": false
            }),
        ),
        spec(
            "write_file",
            "Create or replace a UTF-8 text file inside the workspace. The `path` must be relative and cannot escape the workspace.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        ),
        spec(
            "delete_file",
            "Delete one regular file inside the workspace. The `path` must be relative.",
            serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": false
            }),
        ),
        spec(
            "search",
            "Search the workspace with ripgrep and return matching lines with line numbers.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "path": {"type": "string"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        spec(
            "todo_read",
            "Read the current in-memory todo list for the active task.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ),
        spec(
            "todo_write",
            "Replace the current in-memory todo list for the active task.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": {"type": "string"},
                                "status": {"type": "string", "enum": ["pending", "in_progress", "done", "blocked"]},
                                "detail": {"type": "string"}
                            },
                            "required": ["title", "status"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["items"],
                "additionalProperties": false
            }),
        ),
    ]
}

fn spec(name: &str, description: &str, parameters: Value) -> ApiToolSpec {
    ApiToolSpec {
        kind: "function".to_string(),
        function: ApiToolFunction {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Message;

    #[test]
    fn build_messages_pairs_tool_calls_with_results() {
        let adapter = OpenAiAdapter::new("sys", "dev");
        let history = vec![
            Message::user("hello"),
            Message::assistant_tool_call("functions.read_file", "{\"path\":\"x\"}"),
            Message::tool("functions.read_file", "contents"),
            Message::assistant_final("done"),
        ];

        let messages = adapter.build_messages(&history);

        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
        let call = messages[2]
            .tool_calls
            .as_ref()
            .expect("tool_calls present")
            .first()
            .unwrap();
        assert_eq!(call.function.name, "read_file");
        assert_eq!(messages[3].role, "tool");
        assert_eq!(messages[3].tool_call_id.as_deref(), Some(call.id.as_str()));
        assert_eq!(messages[3].name.as_deref(), Some("read_file"));
        assert_eq!(messages[4].role, "assistant");
        assert_eq!(messages[4].content.as_deref(), Some("done"));
    }

    #[test]
    fn parse_response_prefers_tool_call() {
        let adapter = OpenAiAdapter::new("sys", "dev");
        let raw = CompletionResult {
            text: String::new(),
            tool_calls: vec![ApiToolCall {
                id: "call_0".into(),
                kind: "function".into(),
                function: ApiFunctionCall {
                    name: "read_file".into(),
                    arguments: "{\"path\":\"x\"}".into(),
                },
            }],
            finish_reason: None,
        };
        assert_eq!(
            adapter.parse_response(&raw),
            AssistantOutput::ToolCall {
                recipient: "functions.read_file".into(),
                arguments: "{\"path\":\"x\"}".into(),
            }
        );
    }
}
