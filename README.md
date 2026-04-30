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

## Quick Start

Start a local server that exposes `/v1/completions`, then run:

```sh
cargo run
```

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
max_tool_turns = 16
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

The MVP should stay focused on Harmony-capable `gpt-oss` models behind
OpenAI-compatible local or remote endpoints. External APIs can fit the same
endpoint/auth shape when they are OpenAI-compatible, but provider-specific
adapters and non-Harmony prompt formats for model families such as Qwen or Gemma
should wait until the agent loop, tool visibility, and task tracking are solid.

If the model keeps requesting tools without answering, raise `max_tool_turns` or
ask for a narrower step. Cinto returns a normal assistant message when the
budget is exhausted instead of aborting the turn.

See [docs/architecture.md](docs/architecture.md) for the design notes and next
milestones.

## Useful Commands

```sh
cargo test
cargo run -- --print-prompt
cargo run
```
