# Cinto — Strategic Goal

## Thesis

Small local models fail as agents not because they lack intelligence,
but because they are asked to do too many things at once: hold conversation
state, reason about code, decide which tools to call, produce structured
output, and stay coherent across a long context.

Cinto solves this by providing a **kernelized execution runtime**: discrete
operations, structured memory, verifiable calls, controlled retries, and a
clean separation between reasoning, action, and verification.

The kernel does not make models smarter. It makes models obedient to a
well-defined interface — and that is enough to close most of the reliability
gap on small hardware.

> Cinto is not a terminal with AI. It is a kernelized runtime for small
> agentic models.

---

## North Metric

**Time until a user on a normal machine gets a small, correct, approved
code edit from a local model on a real codebase.**

Every proxy metric (CRP compliance, stage success rate, task completion)
must trace back to this.

---

## Phased Roadmap

### Phase 1 — Prove the Kernel Is the Differentiator (now)

Run the same model with and without Cinto Kernel on a public benchmark.
Publish the comparison.

Target claim:
> Qwen 3.5 9B + free-form prompting: ~10% reliable tool calls  
> Qwen 3.5 9B + Cinto Kernel: ~80% CRP-valid stage completions

Benchmark requirements:
- 100 simple tasks (rename, getter, small fix)
- 100 medium tasks (bugfix with test, refactor)
- 50 code tasks (real public repo bugs)
- 50 file-navigation tasks (find + read + patch)
- 50 repair tasks (first patch fails, model must recover)

Tasks must come from outside Cinto to avoid overfitting. Real bugs from
real public repos are the credible signal.

Metrics per model/mode:
- `crp_valid_rate` — fraction of stages that produced valid CRP
- `workflow_success_rate` — fraction of full pipelines completed
- `task_success_rate` — fraction where validation command passes
- `repair_success_rate` — fraction where model recovers from a failed patch
- `tokens_per_task` — total tokens consumed
- `latency_p50_ms` — median wall time per task
- `vram_peak_mb` — peak VRAM during a task

### Phase 2 — Dataset Pipeline (parallel with Phase 1)

Every successful kernel batch run is a labeled training example. Build
the pipeline to capture and curate these traces.

A trace record contains:
```
problem description
→ kernel stage (interpret / locate / hypothesize / patch / report)
→ system prompt (CRP brief + one-shot example + context pack)
→ model response
→ crp_valid flag
→ workflow_succeeded flag
→ validation_passed flag (cargo test / cargo check)
```

Filter criteria for high-quality training examples:
- `crp_valid = true` for the stage
- `workflow_succeeded = true` for the full pipeline
- `validation_passed = true` (ground truth label)

This dataset is the foundation for CintoLM fine-tuning. It grows
automatically as users run the kernel.

### Phase 3 — CintoLM Fine-tune

Fine-tune a small existing model (1.5B–3B) on the Cinto kernel trace
dataset using LoRA/SFT. The goal is not a smarter model — it is a
**more obedient one**.

The model must learn one thing: given a bounded context pack and a stage
description, emit valid kernel instructions in CRP format.

Starting points (in order of preference):
1. Qwen2.5-Coder-1.5B-Instruct — already strong at code, small footprint
2. BitNet b1.58 2B4T — ternary weights, ~375MB, designed for CPU inference
3. Phi-3.5-mini — strong instruction following at 3.8B

Training signal: traces where `validation_passed = true`. Everything else
is a proxy.

### Phase 4 — Constrained Decoding

Replace the generate → parse → validate → retry loop with grammar-guided
sampling. The model cannot produce invalid CRP because the sampler enforces
the grammar at the token level.

Tools:
- `llama.cpp` GBNF grammars (supported by LM Studio and Ollama)
- `vLLM` guided decoding backend
- `outlines` for Python-based inference pipelines

A GBNF grammar for CRP slots eliminates the retry budget as a failure mode.
The model does not need to "learn" the format — the format is structurally
enforced. This is the largest single reliability improvement available
without changing the model.

### Phase 5 — Architecture Research (deferred)

Only after Phase 1–4 are solid:

- Fine-tune BitNet b1.58 2B4T on Cinto kernel traces
- Investigate Mamba-hybrid architectures for O(n) inference
- Explore distillation from a larger fine-tuned model into a smaller base
- Target: a CintoLM that fits in 1GB VRAM weights and runs on CPU/integrated GPU

The key insight: Cinto does not need a model that knows everything. It needs
a model that knows how to operate the Cinto ISA reliably under 1GB VRAM.
That is a dramatically narrower learning problem than general intelligence.

---

## CintoLM Vision

```
Cinto Kernel Model (CintoLM)
A small model specialized in emitting Cinto Kernel instructions.
Trained on kernelized agentic execution traces.
Optimized for local execution, tool reliability, and low-resource hardware.
```

Positioning:
- Not competing with GPT-4 or Claude on general tasks
- Competing on: runs locally, reliable tool calls, fits on consumer hardware
- Target hardware: 8GB RAM laptop, integrated GPU, Raspberry Pi 5
- Target market: local AI enthusiasts, privacy-first developers, low-resource
  environments (including markets without reliable cloud GPU access)

---

## Current Status (v0.2-dev)

Completed:
- `cinto index` — static workspace indexer (project_map + symbol_index)
- `cinto search` — scoped ripgrep syscall (15 results max, 3/file)
- `read_range`, `read_around`, `list_symbols` — bounded read syscalls
- `context_pack` — budget-enforced prompt assembly per stage
- `WorkerLoop` — 5-stage pipeline (interpret → locate → hypothesize → patch → report)
- Kernel mode toggle in TUI (F5 / `/kernel`)
- `cinto batch --kernel` — headless pipeline evaluation with per-stage metrics
- Plain-text fallback for models that don't follow CRP format
- `<think>` block stripping for reasoning models (Qwen3, DeepSeek-R1)
- One-shot CRP examples per stage in system prompt
- `lm-studio-small` preset for sub-4B models
- `cinto use-preset` / `cinto presets` CLI commands

Benchmark results so far (baseline tasks, 5 tasks):
- Gemma 4 E2B: 5/5 workflows, 60% CRP compliance
- Qwen 3.5 9B: 4/5 workflows, 81% CRP compliance

Next:
- Trace logging for dataset generation
- Locate stage resilience (fallback to interpret search terms)
- `/no_think` config flag for reasoning models
- Expanded benchmark (real repo tasks)
- CintoLM fine-tuning pipeline
