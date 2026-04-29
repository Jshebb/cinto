# OpenHarness

OpenHarness is a Rust terminal UI for experimenting with a Harmony-based local
coding agent loop against open-weight `gpt-oss-20b` and `gpt-oss-120b` model
servers.

The first milestone is intentionally small:

- render Harmony prompts with `system`, `developer`, user, assistant, and tool messages
- call an OpenAI-compatible local `/v1/completions` endpoint
- run in a terminal UI with a persistent transcript
- expose read-only workspace tools through Harmony commentary tool calls

## Why Harmony

`gpt-oss` models expect the Harmony conversation format. OpenHarness keeps that
format explicit so local inference servers can be swapped while the harness keeps
control over agent state, tools, and workspace policy.

## Quick Start

Start a local server that exposes `/v1/completions`, then run:

```sh
cargo run
```

The default endpoint is:

```text
http://127.0.0.1:8000/v1/completions
```

Use `/prompt` inside the TUI to inspect the exact Harmony prompt being sent.

## Configuration

Create `~/.config/openharness/config.toml` or pass `--config path/to/config.toml`.

```toml
[model]
endpoint = "http://127.0.0.1:8000/v1/completions"
model = "openai/gpt-oss-20b"
max_tokens = 4096
temperature = 0.2
stop = ["<|return|>", "<|call|>"]

[harness]
workspace = "/home/you/project"
allow_shell = false
system_prompt = "You are OpenHarness, a local coding agent running in a terminal UI."
developer_prompt = "Use concise reasoning, ask before destructive actions, and prefer small verifiable edits."
```

For `gpt-oss-120b`, change `model` to the model name exposed by your local
server and increase context/token settings on the server side.

## Current Scope

The implemented tools are read-only:

- `functions.list_files`
- `functions.read_file`
- `functions.search`

Editing and shell execution are deliberately gated for a later milestone so the
harness can grow an explicit approval flow instead of mutating a workspace
silently.

See [docs/architecture.md](docs/architecture.md) for the design notes and next
milestones.

## Useful Commands

```sh
cargo test
cargo run -- --print-prompt
cargo run
```
