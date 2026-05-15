# Cinto VS Code Extension — Full Plan

## Why This, Why Now

Cinto's kernel is the product. The TUI is proof that the kernel works. A VS Code extension is
distribution — it puts the kernel where developers already are, without requiring them to change
their workflow. Every install is a potential trace for CintoLM. Every trace is a step toward a
model that can be shipped with the extension, closing the monetization loop.

The sequence is locked:

```
Extension → adoption → traces → CintoLM → bundled model → monetization
```

Nothing in this document should be built out of order.

---

## Architecture

Three distinct layers. Each has one job. None leaks into the others.

```
┌────────────────────────────────────────────────────┐
│  VS Code Extension (TypeScript)                    │
│  Sidebar UI, diff preview, approval buttons,       │
│  progress display, settings webview                │
└────────────────────┬───────────────────────────────┘
                     │  JSON over stdio (IPC protocol)
                     │  stdout: kernel → extension (events)
                     │  stdin:  extension → kernel (responses)
┌────────────────────▼───────────────────────────────┐
│  cinto agent (Rust subprocess)                     │
│  Kernel pipeline: interpret → locate →             │
│  hypothesize → patch → report                      │
│  WorkerLoop, CRP, context pack, patch apply        │
└────────────────────────────────────────────────────┘
                     │  Ollama / LM Studio / any OpenAI-compatible
┌────────────────────▼───────────────────────────────┐
│  Local model server                                │
│  Ollama, LM Studio, vLLM, llamafile, etc.          │
└────────────────────────────────────────────────────┘
```

**The kernel is never rewritten in TypeScript.** All agent logic, CRP parsing, retry budgets,
context packing, and patch application lives in the Rust binary. The extension is a thin shell
that renders kernel output and routes user decisions back.

---

## JSON IPC Protocol

The extension spawns `cinto agent` as a child process and communicates over stdio. Every message
is a single JSON line terminated by `\n`. The protocol is unidirectional by default (kernel
pushes events to extension) with one bidirectional exchange: patch approval.

### Invocation

```sh
cinto agent \
  --task "Fix the off-by-one in find_max" \
  --workspace /path/to/project \
  --config /path/to/cinto.toml \   # optional, falls back to ~/.config/cinto/config.toml
  --traces-dir /path/to/traces     # optional, enables trace collection
```

### Outbound messages (kernel → extension, via stdout)

Every message has a `type` field. Additional fields depend on type.

#### `kernel_ready`
Emitted once at startup, before any stage begins. Signals that the kernel has loaded config
and is about to run.

```json
{"type":"kernel_ready","version":"0.2.0","workspace":"/home/user/myproject","model":"qwen/qwen3.5-9b","context_budget":32000}
```

#### `stage_started`
A pipeline stage has begun. `stage` is one of `interpret | locate | hypothesize | patch | report`.

```json
{"type":"stage_started","stage":"interpret"}
```

#### `context_pack_ready`
Context pack assembled for this stage. Use to show "loaded N chars of context" in the UI.

```json
{"type":"context_pack_ready","stage":"locate","chars_used":8432,"budget":32000}
```

#### `stage_retry`
The model response was not usable. The kernel is retrying with feedback.

```json
{"type":"stage_retry","stage":"interpret","attempt":2,"reason":"FINAL_RESPONSE slot was empty"}
```

#### `stage_completed`
Stage finished. `crp_valid` indicates whether the model followed the CRP format.

```json
{"type":"stage_completed","stage":"interpret","crp_valid":true}
```

#### `stage_skipped`
Stage was skipped due to a recoverable failure (e.g. locate returning no files).

```json
{"type":"stage_skipped","stage":"locate","reason":"no relevant files found, continuing with search terms"}
```

#### `stage_failed`
Stage exhausted its retry budget and could not recover. The pipeline may stop.

```json
{"type":"stage_failed","stage":"hypothesize","error":"model call failed: connection refused"}
```

#### `patch_approval_requested`
The kernel wants to write a file and is waiting for user approval. The extension must respond
with a `patch_approval_response` message on stdin before the kernel can continue.

```json
{
  "type": "patch_approval_requested",
  "id": "f3a2c1b0",
  "path": "src/lib.rs",
  "preview": "--- src/lib.rs\n+++ src/lib.rs\n@@ -12,7 +12,7 @@\n-    for i in 1..=data.len() {\n+    for i in 1..data.len() {\n"
}
```

`id` is a random hex string used to correlate the response. The preview is a unified diff
or a human-readable summary of the proposed change.

#### `patch_applied`
One or more files were written after approval.

```json
{"type":"patch_applied","files_changed":["src/lib.rs"]}
```

#### `workflow_complete`
All stages finished. `final_response` is the report stage summary.

```json
{"type":"workflow_complete","final_response":"Fixed the off-by-one in find_max by changing `1..=data.len()` to `1..data.len()`. Run `cargo test` to verify."}
```

#### `workflow_failed`
The pipeline could not complete.

```json
{"type":"workflow_failed","error":"stage hypothesize exhausted retries: empty model response"}
```

#### `error`
Unexpected runtime error (config missing, workspace not found, etc.).

```json
{"type":"error","message":"workspace /home/user/myproject does not exist"}
```

---

### Inbound messages (extension → kernel, via stdin)

#### `patch_approval_response`
Must be sent in response to every `patch_approval_requested`. The kernel blocks until it
receives this.

```json
{"type":"patch_approval_response","id":"f3a2c1b0","approved":true}
```

`id` must match the corresponding request. If the IDs don't match, the kernel logs a warning
and rejects the patch (safe default).

---

### Protocol guarantees

- The kernel emits exactly one `workflow_complete` or `workflow_failed` before exiting.
- Every `patch_approval_requested` is followed by exactly one `patch_applied` (if approved)
  or nothing (if rejected). The kernel never applies a patch without a matching approval.
- Lines that fail JSON parsing are silently ignored on the inbound side. The extension should
  log them for debugging.
- The kernel exits with code `0` on success, `1` on any error.

---

## VS Code Extension — Phases

### Phase 1 — Working skeleton (weeks 1–4)

Goal: a user can run a task against their open workspace and see results. No polish.

**Features:**
- Sidebar panel (Cinto icon in activity bar)
- Text input for task description
- "Run" button that spawns `cinto agent` subprocess
- Real-time event display: stage progress, current stage indicator
- Patch approval modal: shows preview, Approve / Reject buttons
- Final response rendered as markdown in the panel
- Extension reads `cinto` binary path from settings (defaults to `cinto` on `$PATH`)

**Not in phase 1:**
- Settings UI
- Model configuration
- Diff gutter decorations
- Trace collection toggle

**File structure:**
```
cinto-vscode/
  package.json
  tsconfig.json
  src/
    extension.ts        — activate(), register commands
    agent.ts            — AgentProcess: spawn, parse events, emit typed EventEmitter
    sidebar/
      SidebarProvider.ts  — WebviewViewProvider
      panel.html          — webview HTML + JS (vanilla, no framework)
  media/
    cinto.svg
```

**Key technical decision:** The webview uses vanilla JS + CSS, not React. The sidebar is simple
enough that a framework adds more complexity than it removes. Revisit if the UI grows.

---

### Phase 2 — Usability (weeks 5–8)

Goal: the extension feels intentional, not hacked together.

**Features:**
- Inline diff decoration in the editor when a patch is proposed (VS Code `TextEditorDecorationType`)
- Approve/Reject directly from the editor gutter (CodeLens or inline button)
- Stage progress shown as a status bar item during a run
- Settings UI: endpoint, model name, `cinto` binary path
- "Open workspace" validation — warn if no `cinto.toml` found, offer to create one
- Auto-detect LM Studio/Ollama on localhost:1234 and localhost:11434

---

### Phase 3 — Trace collection (weeks 9–12)

Goal: every successful run generates a trace for CintoLM training.

**Features:**
- Opt-in telemetry toggle in settings ("Help improve CintoLM")
- When enabled, `--traces-dir` is passed to `cinto agent`, pointing to a local spool dir
- A background job uploads completed traces to `traces.cinto.dev` (simple HTTPS POST, gzip)
- Traces are filtered client-side: only `workflow_succeeded = true` and `validation_passed = true`
  runs are uploaded
- Upload batches once per day, not after every run (avoids disruption)
- Traces never include file content beyond what the model saw — no secrets beyond what
  you already sent to the model

**Privacy model:** traces contain system prompt, user message, model response, and outcome flags.
They do not contain workspace file paths, usernames, or machine identifiers beyond a random
installation ID. Users can inspect, export, or delete their local spool at any time.

---

### Phase 4 — CintoLM integration (months 4–12)

Goal: ship the model with the extension. No API key needed. It just works.

**Features:**
- Extension checks for `ollama` on localhost:11434 at startup
- If Ollama is present, offer "Download CintoLM" (pulls `cinto/cintolm-mini` from Ollama registry)
- If Ollama is absent, show "Install Ollama to use CintoLM" with a link
- CintoLM-mini (1.5B) is the free default, no key required
- Extension config: `cinto.model` can be overridden to any OpenAI-compatible endpoint

---

## CintoLM

### What it is

A small model (1.5–3B) fine-tuned on Cinto kernel execution traces. It is not a general-purpose
coding model. It has one job: given a bounded context pack and a stage description, emit valid
CRP-format output.

It does not need to know how to write arbitrary code. It needs to know how to:
- Parse a task description into search terms (interpret)
- Identify relevant file paths from a project map (locate)
- Propose a fix approach from file content (hypothesize)
- Write a `<FILE_EDITS>` block with correct syntax (patch)
- Summarize what was done (report)

This is a dramatically narrower learning problem than general intelligence. A 1.5B model can
learn it if it is trained on enough clean, labeled examples.

### Training data requirements

Each training example is a tuple:
```
(stage, system_prompt, user_message) → model_response
```

Labeled with:
- `crp_valid`: was the response parseable?
- `workflow_succeeded`: did the full pipeline complete?
- `validation_passed`: did `cargo test` / `pytest` pass after patching?

Only examples where all three are `true` are used for supervised fine-tuning. Everything else
is discarded or used for DPO (rejected samples).

**Scale target:** 10,000 high-quality traces before a meaningful fine-tune. At 5 stages per
run and assuming ~30% pass rate, that's ~6,600 runs needed. With 1,000 monthly active users
each running 10 tasks/month, you reach this in ~2 months. With 100 MAU, it's ~20 months.
**Distribution is the bottleneck, not engineering.**

### Starting base model

1. `Qwen2.5-Coder-1.5B-Instruct` — already strong at code structure, small footprint
2. `Phi-3.5-mini-instruct` — strong instruction following at 3.8B
3. `BitNet b1.58 2B4T` — ternary weights, ~375MB on disk, CPU-viable

Fine-tuning method: LoRA/QLoRA via `unsloth` or `axolotl`. Training on a single A100 or
equivalent cloud GPU (rented, ~$2–5/run).

### Model variants

| Variant | Size | License | Delivery |
|---------|------|---------|----------|
| CintoLM-mini | 1.5B Q4 | MIT | Ollama pull, free |
| CintoLM-standard | 7B Q4 | Commercial | Ollama pull, paid tier |
| CintoLM-cloud | hosted | usage-based | Cinto API endpoint, paid |

The mini variant is always free. It is the hook. The standard and cloud variants are the product.

---

## Monetization

### Principle

The extension is free and open-source. The kernel is free and open-source. The model is the
asset. Users who run CintoLM-mini get a working product. Users who need more accuracy or don't
have local hardware pay.

### Tiers

**Free (personal)**
- Extension + kernel, unlimited usage
- CintoLM-mini (1.5B), local via Ollama
- BYO model: any OpenAI-compatible endpoint
- Community support (GitHub issues)

**Pro ($9/month or $79/year — individual)**
- CintoLM-standard (7B), local via Ollama
- Session history and export
- Model comparison mode (run same task on two models, compare results)
- Priority support

**Team ($19/seat/month)**
- Everything in Pro
- Shared model configuration
- Audit log (which tasks ran, which files were changed, who approved)
- Allowlist/blocklist of models
- Offline mode (air-gapped installs, model served internally)

**CintoLM Cloud (usage-based)**
- No local hardware required
- Runs CintoLM-standard on Cinto's infrastructure
- Billed per task ($0.05–0.10/task, depending on model)
- Appeals to users without a GPU or who prefer not to run Ollama

### Licensing model for CintoLM

CintoLM-mini: MIT. Free for all use, including commercial.
CintoLM-standard: custom license, free for personal/non-commercial, paid for commercial.

This mirrors the GitLens / JetBrains model that has been successfully used in VS Code tooling.
The trigger for "commercial" is using the standard model in a work context (not personal
projects). Self-certification on the honor system initially; contract-based for teams.

---

## Timeline

| Phase | Deliverable | Duration |
|-------|-------------|----------|
| 0 | JSON IPC protocol in Rust (`cinto agent` subcommand) | 1 week |
| 1 | VS Code extension skeleton (sidebar, run, approval) | 3 weeks |
| 2 | Usability (inline diff, status bar, settings) | 4 weeks |
| 3 | Trace collection + upload pipeline | 4 weeks |
| Parallel | Accumulate traces, benchmark, publish results | ongoing |
| 4 | CintoLM-mini fine-tune + Ollama registry | ~month 6 |
| 5 | Pro tier + payment (Stripe, Paddle, or LemonSqueezy) | ~month 7 |
| 6 | CintoLM-standard + team tier | ~month 10 |

Phase 0 is what we start today. Nothing else unblocks until the IPC protocol exists.

---

## Key Technical Decisions

**Why stdio, not a local HTTP server?**
Stdio is simpler: no port conflicts, no auth, no firewall issues, no "is the server running?"
dance. The extension owns the process lifecycle. HTTP would be appropriate if multiple
extensions or processes needed to share a single kernel instance — not a concern yet.

**Why not a language server (LSP)?**
LSP is designed for incremental document analysis, not multi-step agentic workflows. The
request/response model doesn't fit the kernel's event stream. And LSP implementations
carry significant complexity overhead that isn't justified here.

**Why keep the kernel in Rust?**
The kernel is the differentiator. It must be fast (local model latency is already high),
correct (CRP parsing, retry logic, patch application are load-bearing), and portable
(Linux, macOS, Windows). Rust is the right tool. Rewriting it in TypeScript to "simplify"
would trade correctness for convenience and would likely be slower.

**Why Ollama for model delivery?**
Ollama is already installed by the majority of Cinto's target users. It handles quantization,
GPU detection, and model versioning. Building a custom model server would duplicate this work
with no user-visible benefit. If Ollama adds a pull API for custom registries (it already
supports this), pushing CintoLM there costs almost nothing.

**What about JetBrains / Neovim / other editors?**
The JSON IPC protocol is editor-agnostic. Once the VS Code extension exists and the protocol
is stable, porting the UI layer to JetBrains or Neovim is a separate project. The kernel and
protocol don't change. VS Code first because it has the largest install base and the extension
API is mature.
