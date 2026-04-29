# OpenHarness

OpenHarness is a Rust terminal UI for experimenting with a Harmony-based local
coding agent loop against open-weight `gpt-oss-20b` and `gpt-oss-120b` model
servers.

The first milestone is intentionally small:

- render Harmony prompts with `system`, `developer`, user, assistant, and tool messages
- call an OpenAI-compatible local `/v1/completions` endpoint
- run in an `OH!` terminal UI with chat and settings views
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
Use `/settings`, `Tab`, or `F2` to open API settings.

## Configuration

Create `~/.config/openharness/config.toml` or pass `--config path/to/config.toml`.

```toml
[model]
endpoint = "http://127.0.0.1:8000/v1/completions"
model = "openai/gpt-oss-20b"
api_key_env = "OPENAI_API_KEY"
max_tokens = 4096
temperature = 0.2
stop = ["<|return|>", "<|call|>"]
request_timeout_secs = 600

[harness]
workspace = "/home/you/project"
allow_shell = false
system_prompt = "You are OpenHarness, a local coding agent running in a terminal UI."
developer_prompt = "Use concise reasoning, ask before destructive actions, and prefer small verifiable edits."
```

Leave `api_key_env` blank or remove it for local servers that do not need bearer
auth. When it is set, OpenHarness reads the secret from that environment
variable and sends it as a bearer token. The TUI saves the variable name, not the
secret.

For `gpt-oss-120b`, change `model` to the model name exposed by your local
server and increase context/token settings on the server side.

## TUI Controls

- `Tab` or `F2`: switch between Chat and Settings
- `Enter`: send a chat message or edit/apply a setting
- `Up`/`Down`: move through settings
- `Space`: toggle boolean settings
- `s`: save settings to TOML
- `Ctrl-C`: quit

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
