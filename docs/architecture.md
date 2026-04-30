# Cinto Architecture

Cinto is meant to become a local coding harness for `gpt-oss-20b` and
`gpt-oss-120b`, with Harmony as the model-facing protocol and a terminal UI as
the operator surface.

## Reference Shape

Pi's coding-agent docs are a useful reference for the outer loop: keep the agent
stateful, make tool calls explicit, show users what the model is doing, and put
workspace-changing actions behind clear policy.

Harmony is the inner protocol. The model should see role, channel, tool, and
final-answer structure rather than an untyped chat transcript. The current
implementation renders a text Harmony prompt because most local OpenAI-compatible
servers accept prompt text through `/v1/completions`.

The official `openai-harmony` Rust crate is the likely next step when we support
token-id based backends directly. That will let the harness rely on the canonical
renderer/parser for servers that accept pre-tokenized prompts.

## Components

- `config`: loads model endpoint, model name, workspace, and prompt policy
- `harmony`: renders Harmony text and parses final answers or tool calls
- `model`: calls an OpenAI-compatible local completion endpoint with optional
  bearer auth from an environment variable
- `session`: owns conversation history, tool execution, and tool-loop depth
- `ui`: provides the ratatui/crossterm terminal interface, including the `[◉]`
  chat, setup, and settings views

## First-Run Setup

If no config file exists, Cinto starts in a setup view instead of dropping the
user directly into chat. The setup view shows a large `CINTO` greeter and lets
the user choose a server preset, endpoint, model, prompt format, workspace, and
core safety defaults. Saving writes the normal config file and switches to chat.
The same screen is available later through `cinto setup` or `/setup`.

## Workspace Instructions

Cinto reads an optional `AGENTS.md` from the configured workspace root when the
session adapter is built. The file is appended to the developer instructions for
both Harmony and OpenAI tool-calling adapters, so project context applies across
model formats. The loaded content is bounded to keep prompt size predictable.

## Tool Policy

Milestone 1 exposes workspace tools:

- `functions.list_files`
- `functions.read_file`
- `functions.write_file`
- `functions.delete_file`
- `functions.search`

It also exposes in-memory task planning tools:

- `functions.todo_read`
- `functions.todo_write`

The todo tools are deliberately not persisted yet. They give the model a
structured way to create, display, and follow a detailed task list during a
session. `write_file` and `delete_file` are intentionally narrow: they only
mutate regular files beneath the configured workspace. They require explicit TUI
approval by default unless `harness.require_edit_approval` is disabled.

Tool loops are capped by `harness.max_tool_turns`, which defaults to 16. Tool
execution errors are returned to the model as tool output so the model can
recover instead of crashing the UI turn.

Milestone 2 should add a diff preview and a user approval step around write
tools.
Milestone 3 can add shell commands with allowlists, working-directory controls,
and visible stdout/stderr in the TUI.

## Safety Rails

The first safety features are local TUI commands, not model tools:

- `/git` or `/changes` shows staged, unstaged, and untracked files.
- `/stage <path|all>`, `/unstage <path|all>`, and `/commit <message>` cover
  a small manual commit flow.
- `/diff` shows git status, diff stat, and a bounded tracked diff.
- `/checkpoint [label]` writes a patch snapshot under `.cinto/checkpoints`.
- `/checkpoints` lists saved snapshots.

Checkpoints are intentionally non-destructive. They do not commit, stash, reset,
or apply changes. Rollback and patch-apply flows should be a later milestone with
explicit preview and approval.

The TUI also renders large tool calls and tool results as compact previews with
size metadata, first lines, and last lines. This is a display-only truncation so
the terminal stays usable after a large `read_file`; the session history still
carries the full tool content for the current model loop.

Very large tool results are compacted before being appended to model-facing
history. The compacted message includes original size metadata, first/last
sections, guidance to use search or narrower reads, and an explicit
`<CINTO_TOOL_OUTPUT_END>` marker. This keeps local models from spending a
turn continuing or reprocessing a huge tool blob.

The session also supports automatic context compression for long-running chats.
When enabled, the session estimates prompt size from the adapter-rendered
request and compares it to `model.context_window`. If the prompt reaches
`harness.context_compression_threshold` percent of that window, older messages
are replaced by a bounded `<CINTO_CONTEXT_COMPACTED>` outline while the latest
`harness.context_compression_keep_recent` messages are preserved exactly. The
compression is deterministic for now; a model-generated summary can be added
later once there is a clear approval and failure story around spending an extra
model call.

Workspace path suggestions are local UI affordances. They scan the configured
workspace for path-like input tokens, skip noisy directories such as `.git`,
`.cinto`, and `target`, and accept the first visible suggestion with
`Right`.

## Model Backends

The default backend is LM Studio or any OpenAI-compatible local server. The
default endpoint is a base URL:

```text
http://127.0.0.1:1234
```

Cinto expands base URLs to:

```text
POST /v1/chat/completions
```

Servers that support raw prompt completions can still be configured with:

```text
POST /v1/completions
```

and returns:

```json
{"choices":[{"text":"..."}]}
```

For `gpt-oss-20b`, this can be a local workstation target. For `gpt-oss-120b`,
the harness should treat latency and context size as first-class UI concerns:
show in-flight status, keep tool output compact, and eventually support
streaming.

API credentials are configured by environment variable name rather than by
storing the secret in TOML. This keeps the settings view useful for local and
remote OpenAI-compatible backends without turning the config file into a secret
store.

The model client supports server-sent event streaming for OpenAI-compatible chat
and completion endpoints. The session still owns Harmony parsing and tool-loop
control; the UI receives assistant deltas over an internal channel and replaces
the live draft with the parsed final/tool message once the model turn closes.

Thinking effort is stored as `model.thinking_effort` and sent as
`reasoning_effort` when it is not `none`. The UI displays the active effort in
the header, context rail, and settings view.

For the MVP, external API support should mean "works through the existing
OpenAI-compatible endpoint and bearer-token shape." That keeps remote and local
testing possible without introducing a provider abstraction before the core
agent loop settles.

Non-Harmony model families such as Qwen or Gemma should be treated as a later
protocol-adapter milestone. They may need different chat templates, tool-call
formats, stop sequences, and parsing rules. Supporting them cleanly means adding
a `protocol` boundary around prompt rendering, tool-call parsing, and response
normalization instead of branching throughout `session` and `model`.

## Next Milestones

1. Replace the prompt parser with canonical Harmony parsing where backend support
   allows it.
2. Add editable file patches with preview, approval, and rollback metadata.
3. Add a planning panel that separates requested work, todo state, tool calls, and final
   answers.
4. Persist sessions under `.cinto/sessions`.
5. Add protocol adapters for non-Harmony models once the Harmony path is stable.
