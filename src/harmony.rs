use chrono::Local;

use crate::session::{Channel, Message, Role};

const START: &str = "<|start|>";
const END: &str = "<|end|>";
const CHANNEL: &str = "<|channel|>";
const MESSAGE: &str = "<|message|>";
const RETURN: &str = "<|return|>";
const CALL: &str = "<|call|>";

#[derive(Debug, Clone)]
pub struct HarmonyPrompt {
    system_prompt: String,
    developer_prompt: String,
    tools: Vec<ToolSpec>,
}

impl HarmonyPrompt {
    pub fn new(system_prompt: impl Into<String>, developer_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            developer_prompt: developer_prompt.into(),
            tools: default_tools(),
        }
    }

    pub fn render(&self, history: &[Message]) -> String {
        let mut prompt = String::new();
        prompt.push_str(&self.render_preamble());

        for message in history {
            render_message(&mut prompt, message);
        }

        prompt.push_str(START);
        prompt.push_str("assistant");
        prompt
    }

    fn render_preamble(&self) -> String {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let mut prompt = String::new();

        prompt.push_str(START);
        prompt.push_str("system");
        prompt.push_str(MESSAGE);
        prompt.push_str(&self.system_prompt);
        prompt.push('\n');
        prompt.push_str("Knowledge cutoff: 2024-06\n");
        prompt.push_str("Current date: ");
        prompt.push_str(&today);
        prompt.push_str(END);

        prompt.push_str(START);
        prompt.push_str("developer");
        prompt.push_str(MESSAGE);
        prompt.push_str(&self.developer_prompt);
        prompt.push_str("\n\nAvailable commentary tools:\n");
        for tool in &self.tools {
            prompt.push_str("- ");
            prompt.push_str(&tool.name);
            prompt.push_str(": ");
            prompt.push_str(&tool.description);
            prompt.push('\n');
            prompt.push_str("  schema: ");
            prompt.push_str(tool.schema);
            prompt.push('\n');
        }
        prompt.push_str(END);

        prompt
    }
}

#[derive(Debug, Clone, Copy)]
struct ToolSpec {
    name: &'static str,
    description: &'static str,
    schema: &'static str,
}

pub fn available_tool_details() -> String {
    let mut details = String::from("Available agent tools\n\n");

    for tool in default_tools() {
        details.push_str("**");
        details.push_str(tool.name);
        details.push_str("**\n");
        details.push_str(tool.description);
        details.push_str("\nSchema: ");
        details.push_str(tool.schema);
        details.push_str("\n\n");
    }

    details.trim_end().to_string()
}

#[derive(Debug, Clone, Copy)]
struct ToolCatalogEntry {
    name: &'static str,
    description: &'static str,
    schema: &'static str,
}

fn default_tools() -> Vec<ToolSpec> {
    const TOOLS: &[ToolCatalogEntry] = &[
        ToolCatalogEntry {
            name: "functions.list_files",
            description: "List immediate child entries under a relative workspace path. Directories are suffixed with `/`.",
            schema: r#"{"type":"object","properties":{"path":{"type":"string"}},"additionalProperties":false}"#,
        },
        ToolCatalogEntry {
            name: "functions.read_file",
            description: "Read a UTF-8 text file from the workspace. The `path` must be relative and cannot escape the workspace. Large results may be compacted in model context; use search or narrower follow-up requests for omitted details.",
            schema: r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#,
        },
        ToolCatalogEntry {
            name: "functions.search",
            description: "Search the workspace with ripgrep and return matching lines with line numbers.",
            schema: r#"{"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string"}},"required":["query"],"additionalProperties":false}"#,
        },
        ToolCatalogEntry {
            name: "functions.todo_read",
            description: "Read the current in-memory todo list for the active task.",
            schema: r#"{"type":"object","properties":{},"additionalProperties":false}"#,
        },
        ToolCatalogEntry {
            name: "functions.todo_write",
            description: "Replace the current in-memory todo list for the active task. Use this to create a task plan, mark progress, and keep the user-facing task state current.",
            schema: r#"{"type":"object","properties":{"items":{"type":"array","items":{"type":"object","properties":{"title":{"type":"string"},"status":{"type":"string","enum":["pending","in_progress","done","blocked"]},"detail":{"type":"string"}},"required":["title","status"],"additionalProperties":false}}},"required":["items"],"additionalProperties":false}"#,
        },
    ];

    TOOLS
        .iter()
        .map(|tool| ToolSpec {
            name: tool.name,
            description: tool.description,
            schema: tool.schema,
        })
        .collect()
}

fn render_message(prompt: &mut String, message: &Message) {
    prompt.push_str(START);
    prompt.push_str(message.role.as_harmony());

    if let Some(channel) = message.channel {
        prompt.push_str(CHANNEL);
        prompt.push_str(channel.as_harmony());
    }

    if let Some(recipient) = &message.recipient {
        prompt.push_str(" to=");
        prompt.push_str(recipient);
    }

    prompt.push_str(MESSAGE);
    prompt.push_str(&message.content);
    prompt.push_str(END);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantOutput {
    Final(String),
    ToolCall {
        recipient: String,
        arguments: String,
    },
    Raw(String),
}

pub fn parse_assistant_output(text: &str) -> AssistantOutput {
    if let Some((recipient, arguments)) = parse_tool_call(text) {
        return AssistantOutput::ToolCall {
            recipient,
            arguments,
        };
    }

    if let Some(final_text) = parse_channel(text, "final") {
        return AssistantOutput::Final(strip_markers(final_text).trim().to_string());
    }

    let stripped = strip_markers(text).trim().to_string();
    if stripped.is_empty() {
        AssistantOutput::Raw(text.to_string())
    } else {
        AssistantOutput::Final(stripped)
    }
}

fn parse_tool_call(text: &str) -> Option<(String, String)> {
    let commentary = parse_channel(text, "commentary")?;
    let to_index = commentary.find(" to=functions.")?;
    let after_to = &commentary[to_index + " to=".len()..];
    let message_index = after_to.find(MESSAGE)?;
    let recipient = normalize_recipient(after_to[..message_index].trim());
    let arguments = after_to[message_index + MESSAGE.len()..]
        .split(CALL)
        .next()
        .unwrap_or_default()
        .split(END)
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    Some((recipient, arguments))
}

fn normalize_recipient(recipient: &str) -> String {
    recipient
        .split("<|")
        .next()
        .unwrap_or(recipient)
        .split_whitespace()
        .next()
        .unwrap_or(recipient)
        .trim()
        .to_string()
}

fn parse_channel<'a>(text: &'a str, channel: &str) -> Option<&'a str> {
    let marker = format!("{CHANNEL}{channel}");
    let index = text.find(&marker)?;
    Some(&text[index + marker.len()..])
}

fn strip_markers(text: &str) -> String {
    text.replace(START, "")
        .replace(END, "")
        .replace(CHANNEL, "")
        .replace(MESSAGE, "")
        .replace(RETURN, "")
        .replace(CALL, "")
        .replace("assistant", "")
        .replace("final", "")
        .replace("analysis", "")
        .replace("commentary", "")
}

impl Role {
    fn as_harmony(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

impl Channel {
    fn as_harmony(self) -> &'static str {
        match self {
            Channel::Analysis => "analysis",
            Channel::Commentary => "commentary",
            Channel::Final => "final",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_harmony_prompt() {
        let prompt = HarmonyPrompt::new("system", "developer");
        let rendered = prompt.render(&[Message::user("hello")]);

        assert!(rendered.contains("<|start|>system<|message|>system"));
        assert!(rendered.contains("<|start|>user<|message|>hello<|end|>"));
        assert!(rendered.ends_with("<|start|>assistant"));
    }

    #[test]
    fn parses_final_channel() {
        let parsed = parse_assistant_output("<|channel|>final<|message|>Done<|return|>");
        assert_eq!(parsed, AssistantOutput::Final("Done".to_string()));
    }

    #[test]
    fn parses_tool_call() {
        let parsed = parse_assistant_output(
            "<|channel|>commentary to=functions.read_file<|message|>{\"path\":\"Cargo.toml\"}<|call|>",
        );

        assert_eq!(
            parsed,
            AssistantOutput::ToolCall {
                recipient: "functions.read_file".to_string(),
                arguments: "{\"path\":\"Cargo.toml\"}".to_string(),
            }
        );
    }

    #[test]
    fn strips_constraint_marker_from_tool_recipient() {
        let parsed = parse_assistant_output(
            "<|channel|>commentary to=functions.list_files <|constrain|>json<|message|>{\"path\":\".\"}<|call|>",
        );

        assert_eq!(
            parsed,
            AssistantOutput::ToolCall {
                recipient: "functions.list_files".to_string(),
                arguments: "{\"path\":\".\"}".to_string(),
            }
        );
    }
}
