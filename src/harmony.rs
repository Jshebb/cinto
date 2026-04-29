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

#[derive(Debug, Clone)]
struct ToolSpec {
    name: String,
    description: String,
    schema: &'static str,
}

fn default_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "functions.list_files".to_string(),
            description: "List files under a relative workspace path.".to_string(),
            schema: r#"{"type":"object","properties":{"path":{"type":"string"}},"additionalProperties":false}"#,
        },
        ToolSpec {
            name: "functions.read_file".to_string(),
            description: "Read a UTF-8 text file from the workspace.".to_string(),
            schema: r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#,
        },
        ToolSpec {
            name: "functions.search".to_string(),
            description: "Search the workspace with ripgrep.".to_string(),
            schema: r#"{"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string"}},"required":["query"],"additionalProperties":false}"#,
        },
    ]
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
    let recipient = after_to[..message_index].trim().to_string();
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
}
