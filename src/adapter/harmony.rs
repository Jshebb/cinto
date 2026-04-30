use crate::harmony::{HarmonyPrompt, available_tool_details, parse_assistant_output};
use crate::session::Message;

use super::{AssistantOutput, ChatRequestPayload, CompletionResult, PromptAdapter, RequestMode};

#[derive(Debug, Clone)]
pub struct HarmonyAdapter {
    prompt: HarmonyPrompt,
}

impl HarmonyAdapter {
    pub fn new(system_prompt: impl Into<String>, developer_prompt: impl Into<String>) -> Self {
        Self {
            prompt: HarmonyPrompt::new(system_prompt, developer_prompt),
        }
    }
}

impl PromptAdapter for HarmonyAdapter {
    fn render_request(&self, history: &[Message]) -> ChatRequestPayload {
        ChatRequestPayload {
            mode: RequestMode::HarmonyText {
                prompt: self.prompt.render(history),
            },
        }
    }

    fn parse_response(&self, raw: &CompletionResult) -> AssistantOutput {
        match parse_assistant_output(&raw.text) {
            crate::harmony::AssistantOutput::Final(text) => AssistantOutput::Final(text),
            crate::harmony::AssistantOutput::ToolCall {
                recipient,
                arguments,
            } => AssistantOutput::ToolCall {
                recipient,
                arguments,
            },
            crate::harmony::AssistantOutput::Raw(text) => AssistantOutput::Raw(text),
        }
    }

    fn debug_render(&self, history: &[Message]) -> String {
        self.prompt.render(history)
    }

    fn tool_details(&self) -> String {
        available_tool_details()
    }
}
