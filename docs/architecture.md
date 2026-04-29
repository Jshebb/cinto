# OpenHarness Architecture

OpenHarness is meant to become a local coding harness for `gpt-oss-20b` and
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
- `ui`: provides the ratatui/crossterm terminal interface, including the `OH!`
  chat and settings views

## Tool Policy

Milestone 1 exposes read-only tools:

- `functions.list_files`
- `functions.read_file`
- `functions.search`

Tool loops are capped by `harness.max_tool_turns`, which defaults to 16. Tool
execution errors are returned to the model as tool output so the model can
recover instead of crashing the UI turn.

Milestone 2 should add write tools with a diff preview and a user approval step.
Milestone 3 can add shell commands with allowlists, working-directory controls,
and visible stdout/stderr in the TUI.

## Model Backends

The default backend is LM Studio or any OpenAI-compatible local server. The
default endpoint is a base URL:

```text
http://127.0.0.1:1234
```

OpenHarness expands base URLs to:

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

## Next Milestones

1. Replace the prompt parser with canonical Harmony parsing where backend support
   allows it.
2. Add streaming completions and incremental transcript rendering.
3. Add editable file patches with preview, approval, and rollback metadata.
4. Add a planning panel that separates requested work, tool calls, and final
   answers.
5. Persist sessions under `.openharness/sessions`.
