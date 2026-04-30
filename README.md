# Cinto

Cinto is a Rust terminal UI for experimenting with a Harmony-based local
coding agent loop against open-weight `gpt-oss-20b` and `gpt-oss-120b` model
servers.

The first milestone is intentionally small:

- render Harmony prompts with `system`, `developer`, user, assistant, and tool messages
- call an OpenAI-compatible local `/v1/completions` endpoint
- run in an `[◉]` terminal UI with chat and settings views
- expose read-only workspace tools through Harmony commentary tool calls
- keep an in-memory task todo list that the agent can create, display, and update

## Why Harmony

`gpt-oss` models expect the Harmony conversation format. Cinto keeps that
format explicit so local inference servers can be swapped while the harness keeps
control over agent state, tools, and workspace policy.

## Install

The release build ships precompiled binaries for Linux, macOS, and Windows.

```sh
curl -fsSL https://raw.githubusercontent.com/joaoh/cinto/main/install.sh | sh
```

The installer detects the current platform, downloads the latest matching
`cinto-<target>.tar.gz` from GitHub Releases, and installs `cinto` into
`${XDG_BIN_HOME:-$HOME/.local/bin}`. Override the destination with
`CINTO_INSTALL_DIR=/path/to/bin`.

Node users can install the npm wrapper, which depends on the matching optional
platform package:

```sh
npm install -g cinto
npx cinto
```

For source builds or Rust development:

```sh
cargo install --git https://github.com/joaoh/cinto
```

## Quick Start

Start a local server that exposes `/v1/completions`, then run:

```sh
cinto
```

From a source checkout, use `cargo run` instead.

On first run, Cinto opens a setup TUI with a large `CINTO` greeter. Pick a
server preset, confirm the endpoint/model/workspace, then save and enter chat.
You can reopen it later with:

```sh
cinto setup
# or inside the TUI
/setup
```

Use `cinto --skip-setup` to go straight to chat even when no config file exists.

The default endpoint is LM Studio's local server base URL:

```text
http://127.0.0.1:1234
```

Cinto normalizes that to `/v1/chat/completions`. You can still provide an
explicit `/v1/completions` endpoint for servers that accept raw text completions.

Use `/prompt` inside the TUI to inspect the exact Harmony prompt being sent.
Use `/tools` to inspect the detailed tool catalog exposed to the agent, and
`/todos` to display the current task todo list.
Use `/diff` before and after risky work to inspect the workspace diff. Use
`/checkpoint [label]` to save a non-destructive patch snapshot under
`.cinto/checkpoints`, and `/checkpoints` to list saved snapshots.
Use `/settings`, `Tab`, or `F2` to open API settings.

## Configuration

Create `~/.config/cinto/config.toml` or pass `--config path/to/config.toml`.

```toml
[model]
endpoint = "http://127.0.0.1:1234"
model = "openai/gpt-oss-20b"
format = "harmony"            # or "openai-tools" for Qwen / Llama / etc.
api_key_env = "OPENAI_API_KEY"
max_tokens = 4096
temperature = 0.2
thinking_effort = "medium"
stream = true
stop = ["<|return|>", "<|call|>"]
request_timeout_secs = 600
context_window = 8192

[harness]
workspace = "/home/you/project"
allow_shell = false
require_edit_approval = true
max_tool_turns = 16
auto_context_compression = true
context_compression_threshold = 80
context_compression_keep_recent = 18
system_prompt = "You are Cinto, a local coding agent running in a terminal UI."
developer_prompt = "Use concise reasoning, ask before destructive actions, and prefer small verifiable edits."
```

Leave `api_key_env` blank or remove it for local servers that do not need bearer
auth. When it is set, Cinto reads the secret from that environment
variable and sends it as a bearer token. The TUI saves the variable name, not the
secret.

`thinking_effort` can be `none`, `low`, `medium`, or `high`. Cinto sends
it as `reasoning_effort` to compatible OpenAI-style servers. Set `stream = true`
to render model output continuously as chunks arrive.

For `gpt-oss-120b`, change `model` to the model name exposed by your local
server and increase context/token settings on the server side.

## Workspace Instructions

Cinto reads `AGENTS.md` from the configured workspace root on startup and injects
it into the model-facing developer instructions. Use it for project context,
coding conventions, common commands, and anti-patterns to avoid.

The file is optional. If present, Cinto includes a bounded copy so large
instructions cannot dominate the prompt. Reopen `/prompt` to inspect the exact
instructions being sent.

## TUI Controls

- `Tab` or `F2`: switch between Chat and Settings
- `Enter`: send a chat message or edit/apply a setting
- `Right`: accept the first workspace path suggestion when one is visible
- `Up`/`Down`: move through settings
- `Space`: toggle boolean settings
- `s`: save settings to TOML
- `Ctrl-C`: quit

Cinto suggests workspace paths while you type path-like tokens such as
`src/`, `Cargo`, or `docs/read`. Suggestions are read from the configured
workspace and skip `.git`, `.cinto`, and `target`.

## Safety Commands

- `/git` or `/changes`: show staged, unstaged, and untracked files
- `/stage <path|all>`: stage one or more paths, or all changes
- `/unstage <path|all>`: unstage one or more paths, or all staged changes
- `/commit <message>`: commit currently staged changes
- `/diff`: show git status, diff stat, and a truncated tracked diff
- `/checkpoint [label]`: save the current tracked diff plus status as a patch snapshot
- `/checkpoints`: list saved checkpoint patch files

Checkpoints do not commit, stash, or roll back anything. They are plain files in
the workspace so you can inspect them before applying anything manually.

## Current Scope

The implemented workspace tools are:

- `functions.list_files`
- `functions.read_file`
- `functions.write_file`
- `functions.delete_file`
- `functions.search`

The agent can also maintain in-memory task state:

- `functions.todo_read`
- `functions.todo_write`

`write_file` creates or replaces UTF-8 files beneath the configured workspace,
and `delete_file` removes a single regular file. File edits require an explicit
TUI approval by default; toggle `edit approval` in settings to unlock direct
model edits.
Shell execution is deliberately gated for a later milestone so the harness can
grow an explicit approval flow for commands.

Large tool calls and tool results render as compact transcript previews with
size metadata plus first/last snippets. The session still keeps the full tool
content for the active model loop.

Very large tool results are also compacted before being added back to model
context, with an explicit end marker. This prevents a large `read_file` from
turning the next prompt into a wall of source text; the model can use `search`
or a narrower request if it needs omitted middle sections.

When `harness.auto_context_compression = true`, Cinto also watches the estimated
prompt size against `model.context_window`. Once it reaches
`harness.context_compression_threshold` percent, older transcript messages are
replaced with a bounded `<CINTO_CONTEXT_COMPACTED>` outline while the most recent
`harness.context_compression_keep_recent` messages stay exact. The TUI adds a
visible "Context Compressed" transcript event whenever this happens.

The MVP supports two prompt formats — see the section below.

## Supported model formats

Cinto routes the agent loop through a `PromptAdapter` chosen by
`model.format` (TOML) or the `format` row in the Settings panel. Switch with
`Space` or by typing the value.

| Format | When to use | Tool-calling | Endpoint |
| --- | --- | --- | --- |
| `harmony` (default) | `gpt-oss-20b`, `gpt-oss-120b`, any model trained on the Harmony channel format | Embedded as `commentary to=functions.X` text | `/v1/chat/completions` (preferred) or `/v1/completions` |
| `openai-tools` | Qwen 2.5 Instruct, Llama 3.1 Instruct, and other models served by **LM Studio** or **Ollama** that expose native OpenAI-style `tools` and `tool_calls` | Native `tool_calls` field | `/v1/chat/completions` |

The Harmony adapter renders the channel-tagged Harmony prompt as a single
message and parses tool calls out of the assistant text. The OpenAI adapter
sends a structured `messages` array, advertises every workspace tool through the
`tools` field, and reads `tool_calls` directly from the response (including
streamed `delta.tool_calls` chunks).

### LM Studio with `openai-tools`

1. Load a tool-calling model (e.g. `Qwen2.5-Coder-7B-Instruct`) and start the
   local server.
2. Set `model.format = "openai-tools"`, `model.endpoint = "http://127.0.0.1:1234"`
   and `model.model` to the LM Studio model id.
3. Set `model.thinking_effort = "none"` — `reasoning_effort` is gpt-oss-only.

### Ollama with `openai-tools`

1. Pull a tool-capable tag (e.g. `ollama pull qwen2.5-coder:7b-instruct`).
2. Set `model.endpoint = "http://127.0.0.1:11434"` and
   `model.format = "openai-tools"`. Ollama exposes the OpenAI-compatible
   chat-completions API at `/v1/chat/completions`.
3. Leave `api_key_env` empty.

Provider-specific tweaks (Anthropic, Gemini, etc.) can layer on top of
`openai-tools` whenever the upstream server emits the same response shape.

If the model keeps requesting tools without answering, raise `max_tool_turns` or
ask for a narrower step. Cinto returns a normal assistant message when the
budget is exhausted instead of aborting the turn.

See [docs/architecture.md](docs/architecture.md) for the design notes and next
milestones.

## Useful Commands

```sh
cargo test
cargo run -- setup
cargo run -- --print-prompt
cargo run
```
