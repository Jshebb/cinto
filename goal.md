Em model.rs:316-326 (struct ChatCompletionRequest), adicionar campo opcional tools: Option<Vec<ToolSpec>> e tool_choice.
Em model.rs:339-347 (struct ChatCompletionMessage), adicionar tool_calls: Option<Vec<ToolCallResponse>>.
Trait PromptAdapter em src/adapter/mod.rs com 3 métodos: render_request(&history) -> Request, parse_response(&response) -> AssistantOutput, render_tool_message(&Message) -> ApiMessage.
Em session.rs:106, AgentSession::new dispatcha pelo config.model.format.
README ganha seção "Supported model formats".

