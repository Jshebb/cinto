# CRP - Cinto Reasoning Protocol

**Version:** 1.0-draft  
**Status:** Draft / Under development  
**Last updated:** 2026-05-01

## 1. Overview

The Cinto Reasoning Protocol (CRP) is a structured output protocol for coding
agents. It replaces free-form reasoning ("chain of thought") with typed,
slot-based reasoning that can be parsed, validated, and audited
programmatically.

CRP exists because free-form reasoning has known failure modes in small or
compressed language models: drift across long traces, unverifiable
confabulation, and structural inconsistency that prevents downstream tooling
from acting on the output reliably. CRP constrains reasoning into a predictable
shape that a harness can validate slot-by-slot, reject when malformed, and route
to specialized handlers when valid.

CRP is the native reasoning protocol of the Cinto harness. Other harnesses or
tools may implement CRP independently; this specification is the reference.

### 1.1 Design Goals

- **Auditability.** Every reasoning step occupies a labeled slot with a known
  type. Validation is mechanical, not subjective.
- **Recoverability.** When a slot fails validation, the harness can request a
  targeted retry rather than discarding the whole trace.
- **Model-agnosticism.** CRP works with any model capable of emitting structured
  text. No special tokenization required.
- **Hardware-friendliness.** Small models (1B-7B parameters) at aggressive
  quantization (Q4 and below) can produce valid CRP output reliably when
  fine-tuned on it.
- **Composability.** Slots can be processed independently. Multiple models or
  tools can fill or audit different slots in the same trace.

### 1.2 Non-goals

CRP does not specify the underlying transport between harness and model (HTTP,
OpenAI-compatible API, local llama.cpp, etc). It does not mandate specific
tool-calling formats. It does not replace function calling or tool invocation;
CRP describes reasoning, tools handle action.

## 2. Syntax

CRP uses XML-like slot tags. Each slot is delimited by an opening tag and a
matching closing tag, with the slot name in uppercase ASCII.

### 2.1 Basic Form

```text
<SLOT_NAME>
content goes here
</SLOT_NAME>
```

Slot content may span multiple lines and may include any text content, including
code blocks, markdown formatting, lists, or special characters. Parsers must
treat content as opaque text until the closing tag.

### 2.2 Parsing Rules

- Slot names are case-sensitive and uppercase. `<TASK_INTERPRETATION>` is
  valid; `<task_interpretation>` is not.
- Whitespace surrounding slot content is trimmed during parsing.
- Slots appear sequentially. Nested slots are not permitted in v1.0.
- Unknown slots are preserved by the parser and passed to user handlers, but
  standard validation does not apply.
- Malformed slots (missing closing tag, mismatched names) trigger validation
  failure.

### 2.3 Reserved Characters

Content may contain `<` and `>` freely. Parsers must use the closing tag pattern
`</SLOT_NAME>` to delimit, not raw bracket detection. This allows CRP slots to
contain HTML, XML, code, or any markup without escaping.

The literal sequence `</SLOT_NAME>` (matching the active opening tag exactly)
cannot appear inside content. If a model needs to reference such a sequence in
content, it must use the slot type that does not include the conflicting tag, or
escape per slot-type-specific rules.

## 3. Standard Slot Types (v1.0)

The following slots are defined in v1.0. Implementations should support all of
them. Custom slots may be defined per-template; standard slots are the lingua
franca.

### 3.1 Core Slots

**TASK_INTERPRETATION**  
A single paragraph rephrasing the user's request in the agent's own words.
Establishes shared understanding before action.

- Type: `Paragraph`
- Required: yes (in most templates)
- Validation: non-empty, less than 500 words

**ASSUMPTIONS**  
Explicit list of assumptions the agent is making to disambiguate the request.

- Type: `BulletList<Assumption>`
- Required: yes when ambiguity is detected
- Validation: each item is one sentence, falsifiable in principle

**RELEVANT_FILES**  
Files the agent considers necessary to read or modify.

- Type: `BulletList<FilePath>`
- Required: yes for tasks involving existing code
- Validation: each path must exist in the workspace; non-existent paths trigger
  retry

**PROPOSED_APPROACH**  
High-level strategy in 3-7 bullet points. Describes what will be done, not how.

- Type: `BulletList<Step>`
- Required: yes
- Validation: each bullet is one sentence, no implementation details

**RISKS**  
Things that could go wrong with the proposed approach.

- Type: `BulletList<Risk>`
- Required: optional
- Validation: each item describes an observable failure mode

**DELIVERABLE_SPEC**  
Description of what success looks like, observably.

- Type: `Paragraph`
- Required: yes for executable tasks
- Validation: includes at least one observable outcome (file changed, test
  passes, output produced)

### 3.2 Action Slots

**FILE_EDITS**  
Concrete file edits to apply. Wraps one or more `<EDIT>` sub-blocks.

- Type: `Sequence<EditDirective>`
- Required: when task involves file changes
- Validation: each `EDIT` references an existing file, syntactically valid for
  target language when language is known

**COMMAND_PROPOSALS**  
Shell commands the agent proposes to execute.

- Type: `BulletList<Command>`
- Required: when task involves command execution
- Validation: commands are non-destructive by default, destructive ones flagged

**CHECKPOINTS**  
Intermediate states the agent will verify before proceeding.

- Type: `BulletList<Checkpoint>`
- Required: optional, recommended for multi-step tasks
- Validation: each checkpoint is observable

### 3.3 Communication Slots

**CLARIFICATION_REQUEST**  
Question to the user when the agent cannot proceed without input.

- Type: `Paragraph`
- Required: when ambiguity blocks progress
- Validation: phrased as direct question, single subject

**FINAL_RESPONSE**  
The user-facing answer or summary, after action is complete.

- Type: `Paragraph` or `Markdown`
- Required: yes (terminates the trace)
- Validation: non-empty, addresses the original task

## 4. Slot Type System

Slot types define what valid content looks like. Validation is performed by the
harness; the model is informed of types via the system prompt.

### 4.1 Primitive Types

- `Paragraph` - one or more sentences of natural text, no list markers.
- `Markdown` - full markdown allowed.
- `Sentence` - single sentence, no period required.
- `Identifier` - alphanumeric plus underscores, valid as a programming
  identifier.
- `FilePath` - relative or absolute path; harness validates existence in
  workspace.
- `Command` - shell command line; harness may apply safety classification.

### 4.2 Composite Types

- `BulletList<T>` - sequence of items each of type T, expressed as markdown
  list.
- `Sequence<T>` - ordered sequence of typed sub-blocks (used for `FILE_EDITS`).
- `Optional<T>` - slot may be empty.

### 4.3 Semantic Types

These require LLM-based audit (deferred to v1.1+ or implementation-specific):

- `Assumption` - a falsifiable claim.
- `Risk` - an observable failure mode.
- `Checkpoint` - an observable intermediate state.

In v1.0, semantic types are validated heuristically (length, format) and audited
by humans or LLM auditors when configured.

## 5. Validation and Retry Protocol

### 5.1 Validation Outcomes

When a CRP trace is parsed, each slot receives one of three outcomes:

- **VALID** - slot present, content matches type, all checks pass.
- **WARNING** - slot present, content matches type, but a soft check failed
  (e.g., proposed file does not exist but path is well-formed).
- **INVALID** - slot is missing when required, malformed, or fails a hard check.

A trace is **executable** if all required slots are VALID and no INVALID slots
exist. WARNING slots do not block execution but are surfaced to the user.

### 5.2 Retry Message Format

When a trace contains INVALID slots, the harness sends a retry message to the
model. The retry message uses the same CRP syntax to be parseable:

```text
<RETRY_REASON>
The previous response had validation issues. Please provide a corrected response.
</RETRY_REASON>

<SLOT_FEEDBACK slot="RELEVANT_FILES" status="INVALID">
The file 'src/auth/old_session.rs' does not exist in the workspace.
Available files in src/auth/ are: session.rs, middleware.rs, jwt.rs.
Please reconsider which files are actually relevant.
</SLOT_FEEDBACK>

<SLOT_FEEDBACK slot="ASSUMPTIONS" status="INVALID">
This slot is required for ambiguous requests but was empty.
The original request mentioned "improve auth" without specifying scope.
Please list at least one assumption you are making.
</SLOT_FEEDBACK>
```

The model is expected to produce a complete new CRP trace addressing the
feedback. v1.0 does not support partial retries (slot-by-slot replacement); the
entire trace is regenerated.

### 5.3 Retry Budget

Implementations should enforce a retry budget. Recommended default: 3 retries
before giving up and surfacing the best-effort trace to the user with warnings.

### 5.4 Kill Conditions

A trace may be killed (no retry, immediate failure) when:

- Slot syntax is so malformed that parsing cannot recover (e.g., no
  recognizable slots in output).
- Required `FINAL_RESPONSE` slot indicates the model cannot complete the task
  and is requesting clarification (in which case execution stops and the request
  is surfaced to the user, not killed as an error).
- Configured safety policies are violated (e.g., `COMMAND_PROPOSALS` contains
  explicitly forbidden commands).

## 6. Lifecycle (Level 2: Output + Structured Retry)

CRP v1.0 specifies the **Level 2 lifecycle**: the model receives an
unstructured prompt (with system instructions to respond in CRP), produces a
complete CRP trace, and may be asked to retry with structured feedback if
validation fails.

### 6.1 Standard Flow

1. **User input.** User provides natural language request to harness.
2. **Prompt construction.** Harness constructs prompt: system instructions
   describing CRP + applicable template + user request.
3. **Model generation.** Model produces output. Harness parses as CRP.
4. **Validation.** Harness validates each slot per its type and applicability.
5. **Branch:**
   - If valid: harness renders trace, executes any actions (edits, commands)
     per user approval, and returns result.
   - If invalid and retry budget remains: harness sends retry message (per
     section 5.2) and returns to step 3.
   - If invalid and retry budget exhausted: harness surfaces best-effort trace
     with warnings.

### 6.2 System Prompt Content

Implementations must include the following information in the system prompt:

- A brief explanation of CRP and its slot syntax.
- The list of slot types relevant to the current template, with type
  signatures.
- An example of a valid CRP trace for the task category at hand.
- A reminder that the model must respond entirely in CRP format.

Implementations may compress this into a `CRP_BRIEF` section if the model is
fine-tuned on CRP and does not require full instructions.

## 7. Templates

A **template** is a named bundle of slot requirements appropriate for a category
of task. Templates are expressed as TOML files in the harness configuration.

Example template (`code_edit.toml`):

```toml
name = "code_edit"
description = "Modify existing files in the workspace"

[slots]
TASK_INTERPRETATION = { required = true }
ASSUMPTIONS = { required = "when_ambiguous" }
RELEVANT_FILES = { required = true, type = "BulletList<FilePath>" }
PROPOSED_APPROACH = { required = true }
RISKS = { required = false }
FILE_EDITS = { required = true }
DELIVERABLE_SPEC = { required = true }
FINAL_RESPONSE = { required = true }
```

The harness selects a template based on task classification (manual via flag, or
automatic via classifier). Custom templates may be defined per-project in
`.cinto/templates/`.

CRP v1.0 ships three reference templates: `code_edit`, `code_explanation`, and
`design_proposal`. Implementations should treat these as a baseline.

## 8. Versioning

CRP follows semantic versioning at the protocol level.

- **MAJOR** version increments on breaking changes to slot syntax, type system,
  or lifecycle semantics.
- **MINOR** version increments on additions (new standard slots, new types, new
  lifecycle levels) that remain backward compatible.
- **PATCH** version increments on clarifications and editorial fixes.

Implementations should declare their supported CRP version in their handshake or
system prompt. Models fine-tuned on a specific CRP version should declare that
version in their model card.

## 9. Future Directions (Informative, Not Normative)

The following are anticipated extensions in CRP v1.x and v2.0. They are
documented here to guide implementation choices that preserve compatibility.

### 9.1 Lifecycle Level 3 (Slot-by-slot)

A fuller lifecycle in which the harness drives generation slot-by-slot rather
than receiving the full trace at once. This enables earlier auditing (kill on
first invalid slot, before downstream slots are wasted) and finer-grained
retries (regenerate only the failing slot).

### 9.2 LLM-based Semantic Auditors

Validation of semantic types (`Assumption`, `Risk`, `Checkpoint`) by a separate
auditor model. The auditor reads each slot and emits a structured judgment, also
in CRP. Composes naturally with section 5.

### 9.3 Multi-model Role Specialization

Different slots produced by different models. For example, a small planner
model produces `TASK_INTERPRETATION` and `PROPOSED_APPROACH`; a code-specialized
model fills `FILE_EDITS`; an auditor model reviews. Each specialist is smaller
than a generalist would be, but the composition matches generalist quality.

### 9.4 Cross-session Memory Binding

Slots that reference persistent notes (file-based memory, like an Obsidian
vault). The model can declare in `RELEVANT_FILES` not just code files but
knowledge files, and the harness loads them into context.

### 9.5 Streaming and Partial Validation

Streaming CRP output with validation triggered as each closing tag is emitted.
Allows immediate feedback on early errors without waiting for full trace
completion.

## 10. Reference Implementation

The Cinto harness (https://github.com/Jshebb/cinto) is the reference
implementation of CRP. Where this specification is ambiguous, the reference
implementation defines the expected behavior, and ambiguity is treated as a
specification bug to be resolved in subsequent versions.

## Appendix A: Minimal Example Trace

User request: "Add a hello function to src/main.rs that prints 'hello, world'"

Model response:

```text
<TASK_INTERPRETATION>
The user wants me to add a new function named `hello` to the file src/main.rs. The function should print the literal string "hello, world" to standard output.
</TASK_INTERPRETATION>

<ASSUMPTIONS>
- The function should be named exactly `hello` (lowercase, no parameters).
- "hello, world" should be printed with a newline (using println! in Rust).
- The function does not need to be called from main; it just needs to be defined.
</ASSUMPTIONS>

<RELEVANT_FILES>
- src/main.rs
</RELEVANT_FILES>

<PROPOSED_APPROACH>
- Add a new function `fn hello()` to src/main.rs.
- Use `println!("hello, world");` as the function body.
- Place the function above main() for visibility.
</PROPOSED_APPROACH>

<FILE_EDITS>
<EDIT path="src/main.rs" mode="prepend_to_existing">
fn hello() {
    println!("hello, world");
}

</EDIT>
</FILE_EDITS>

<DELIVERABLE_SPEC>
After the edit, src/main.rs contains a function named `hello` that, when called, prints "hello, world" followed by a newline. The existing main function and other code are unchanged.
</DELIVERABLE_SPEC>

<FINAL_RESPONSE>
I've added a `hello()` function to src/main.rs that prints "hello, world". You can call it from main() or anywhere else in your code. Let me know if you'd like me to wire it into main() or modify the implementation.
</FINAL_RESPONSE>
```

## Appendix B: Draft Notes

The `FILE_EDITS` subformat is intentionally not final in v1.0-draft. Edit modes
such as prepend, append, replace, and string replacement should be specified
after dogfooding against existing agent editing conventions.

Template classification is intentionally underspecified. v1.0 should assume
manual template selection; automatic classification belongs in a future minor
version.

Slot names are always uppercase English identifiers for parser stability.
Slot content may be written in any natural language.
