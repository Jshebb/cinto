# Cinto Agent Instructions

## Project Context

Cinto is a Rust terminal UI for experimenting with local coding-agent loops
against OpenAI-compatible model servers. It supports Harmony prompts for
`gpt-oss` models and native OpenAI-style tool calls for other tool-capable local
models.

The main runtime shape is:

- `src/config.rs`: persisted model and harness settings
- `src/session.rs`: conversation history, tool execution, context compression
- `src/adapter/`: prompt rendering and response parsing per model format
- `src/model.rs`: OpenAI-compatible HTTP and streaming client
- `src/ui.rs` plus `src/ui/`: ratatui/crossterm interface
- `src/workspace.rs`: local git, diff, checkpoint, and path helpers

## Code Conventions

- Prefer existing local patterns over new abstractions.
- Keep model-protocol differences behind `PromptAdapter`.
- Keep workspace mutation narrow and explicit.
- Preserve TUI responsiveness and readable transcript output.
- Use bounded output for anything that can grow with repository size.
- Keep comments short and only where they clarify non-obvious behavior.

## Common Commands

```sh
cargo fmt --check
cargo test
cargo run -- --help
cargo run -- setup
cargo run -- --print-prompt
node --check npm-package/bin/cinto.js
sh -n install.sh
npm pack --dry-run ./npm-package
```

## Anti-Patterns

- Do not bypass adapter boundaries with format-specific branches in session code.
- Do not add unbounded file, git, or model output to prompt context.
- Do not make destructive workspace actions silent or automatic.
- Do not store API secrets in config files.
- Do not add network/provider-specific assumptions to the core agent loop unless
  they are behind a clear compatibility layer.
