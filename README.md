# Cinto

[![CI](https://github.com/joaoh/cinto/actions/workflows/ci.yml/badge.svg)](https://github.com/joaoh/cinto/actions/workflows/ci.yml)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

**Cinto is a local terminal coding-agent harness for OpenAI-compatible model
servers.** It gives local and open-weight models a focused workspace loop:
read files, search code, propose edits, keep todos, inspect prompts, and stay
inside explicit safety rails.

[Português](README_pt.md) · [Architecture notes](docs/architecture.md)

![Cinto TUI](docs/demo.png)

## Why Cinto

Most coding agents hide the harness. Cinto keeps it visible.

- **Local-first:** works with LM Studio, Ollama, and other OpenAI-compatible
  servers.
- **Two tool modes:** Harmony prompt rendering for `gpt-oss` models and native
  OpenAI `tool_calls` for Qwen, Llama, and similar tool-capable models.
- **TUI setup:** first-run greeter, model/server presets, settings, chat, and
  path suggestions.
- **Workspace tools:** `list_files`, `read_file`, `search`, `write_file`, and
  `delete_file`.
- **Safety controls:** edit approvals, `/diff`, `/checkpoint`, protected
  `.git`/`.cinto` paths, and no shell execution in the current milestone.
- **Persistent project context:** optional `AGENTS.md` instructions at the
  workspace root.
- **Context management:** large tool outputs and older transcript history are
  compacted before they overwhelm the model context window.

## Install

### Source install

Use this today from a source checkout or a public GitHub repo:

```sh
cargo install --git https://github.com/joaoh/cinto
```

### Shell installer (Linux / macOS)

After the first `v*` release is tagged, GitHub Releases will publish
precompiled binaries:

```sh
curl -fsSL https://raw.githubusercontent.com/joaoh/cinto/main/install.sh | sh
```

The installer detects your platform, downloads `cinto-<target>.tar.gz` from the
latest GitHub Release, verifies the SHA-256 checksum, installs `cinto` into
`${XDG_BIN_HOME:-$HOME/.local/bin}`, and adds it to your shell profile.

Override the install directory:

```sh
curl -fsSL https://raw.githubusercontent.com/joaoh/cinto/main/install.sh \
  | CINTO_INSTALL_DIR="$HOME/bin" sh
```

### PowerShell installer (Windows)

```powershell
Invoke-WebRequest -Uri https://raw.githubusercontent.com/joaoh/cinto/main/install.ps1 -UseBasicParsing | Invoke-Expression
```

This downloads and extracts the latest Windows release to `~\.local\bin` and automatically adds it to your User `PATH`.

### npm wrapper

After the npm packages are published:

```sh
npm install -g cinto
npx cinto
```

The npm package is a small launcher that depends on the matching optional
platform package, such as `@cinto/linux-x64` or `@cinto/darwin-arm64`.

### Uninstall

```sh
cinto uninstall
cinto uninstall --purge-config
```

`--purge-config` also removes `~/.config/cinto`.

## Quick Start

1. Start an OpenAI-compatible local model server.
2. Run Cinto:

```sh
cinto
```

3. In the setup TUI, choose a preset, confirm the endpoint/model/workspace, and
   save.
4. Try a focused request:

```text
Summarize this repository and list the files you inspected.
```

Useful setup commands:

```sh
cinto setup          # reopen the first-run setup TUI
cinto --skip-setup   # go straight to chat
cinto --print-prompt # inspect the empty rendered prompt
cinto --config ./config.toml
```

## Model Setup

The default endpoint is the LM Studio local server base URL:

```text
http://127.0.0.1:1234
```

Cinto normalizes base URLs to `/v1/chat/completions`. You can still pass an
explicit `/v1/completions` endpoint for text-completion servers.

| Server | Example endpoint | Recommended format | Notes |
| --- | --- | --- | --- |
| LM Studio with `gpt-oss` | `http://127.0.0.1:1234` | `harmony` | Use the model id shown by LM Studio. |
| LM Studio with Qwen/Llama | `http://127.0.0.1:1234` | `openai-tools` | Set `thinking_effort = "none"`. |
| Ollama | `http://127.0.0.1:11434` | `openai-tools` | Pull a tool-capable model such as `qwen2.5-coder:7b-instruct`. |

Example Ollama flow:

```sh
ollama pull qwen2.5-coder:7b-instruct
cinto setup
```

Then set:

```toml
[model]
endpoint = "http://127.0.0.1:11434"
model = "qwen2.5-coder:7b-instruct"
format = "openai-tools"
thinking_effort = "none"
```

## TUI Workflow

Inside the TUI:

- Type a request and press `Enter`.
- Use `/tools` to inspect the tool catalog exposed to the model.
- Use `/prompt` to inspect the exact prompt/messages being sent.
- Use `/settings`, `Tab`, or `F2` to edit model and harness settings.
- Use `/diff` before and after risky work.
- Use `/checkpoint [label]` to save a patch snapshot under
  `.cinto/checkpoints`.
- Use `/git`, `/stage`, `/unstage`, and `/commit` for explicit git actions.

Keyboard shortcuts:

| Key | Action |
| --- | --- |
| `Tab` / `F2` | Switch between Chat and Settings |
| `Enter` | Send message or edit/apply a setting |
| `Right` | Accept the first workspace path suggestion |
| `Up` / `Down` | Move through settings |
| `Space` | Toggle boolean settings |
| `s` | Save settings to TOML |
| `Ctrl-C` | Quit |

## Workspace Instructions

Cinto reads `AGENTS.md` from the configured workspace root and injects it into
the model-facing developer instructions. Use it for:

- project context
- coding conventions
- common commands
- anti-patterns to avoid
- release or testing expectations

The file is optional and bounded so it cannot dominate the prompt. Reopen
`/prompt` to inspect the exact instructions being sent.

For untrusted repositories, disable it:

```toml
[harness]
load_workspace_instructions = false
```

Or toggle `workspace instructions` off in Settings.

## Safety Model

Cinto is a local agent harness, not a sandbox.

- Read tools can expose workspace contents to the configured model endpoint.
- File writes and deletes require TUI approval by default.
- Workspace paths cannot escape the configured root.
- `.git` and `.cinto` internals are protected from model tool access.
- Shell execution is intentionally not exposed in the current milestone.
- Release archives are checked with SHA-256 by the installer, but they are not
  code-signed or notarized yet.

Avoid pointing Cinto at repositories that contain `.env` files, private keys, or
production credentials unless you trust the model server and understand what the
agent can read.

## Configuration

Cinto stores config at `~/.config/cinto/config.toml` unless `--config` is
provided.

```toml
[model]
endpoint = "http://127.0.0.1:1234"
model = "openai/gpt-oss-20b"
format = "harmony"            # harmony or openai-tools
api_key_env = ""
max_tokens = 4096
temperature = 0.2
thinking_effort = "medium"    # none, low, medium, high
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
load_workspace_instructions = true
system_prompt = "You are Cinto, a local coding agent running in a terminal UI."
developer_prompt = "Use concise reasoning, ask before destructive actions, and prefer small verifiable edits."
```

When `api_key_env` is set, Cinto reads the secret from that environment variable
and sends it as a bearer token. The TUI stores the variable name, not the secret.

## Supported Tool Formats

| Format | Tool-calling shape | Best fit |
| --- | --- | --- |
| `harmony` | Tool calls embedded in Harmony-style assistant text | `gpt-oss-20b`, `gpt-oss-120b`, and Harmony-compatible servers |
| `openai-tools` | Native OpenAI-compatible `tools` and `tool_calls` fields | Qwen, Llama, Ollama, LM Studio, and other OpenAI-compatible chat servers |

If the model keeps requesting tools without answering, raise `max_tool_turns` or
ask for a narrower step. If the model returns neither text nor a tool call,
Cinto shows an `Empty Model Response` note with the active model and format.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- setup
cargo run
```

Release and packaging checks:

```sh
cargo package --allow-dirty --no-verify
sh -n install.sh
node --check npm-package/bin/cinto.js
npm pack --dry-run ./npm-package
```

## Project Status

Cinto is early-stage and intentionally small. The current release focuses on the
local agent loop, prompt/tool adapters, setup flow, install paths, and safety
controls. Shell tools, provider-specific adapters, richer persistence, and
signed binaries can layer on later.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
