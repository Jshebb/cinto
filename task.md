# Task: Relax CRP Without Losing Typed Validation

## Goal

Make the CRP loop less brittle for local models while keeping structured,
typed validation where it matters.

## Current Direction

- Keep `FINAL_RESPONSE` as the hard-required slot.
- Treat other template `required = true` slots as recommended structure:
  missing or empty content should warn, not force retry.
- Keep typed validation strict when typed slots are present, especially
  `BulletList<FilePath>` slots such as `RELEVANT_FILES`.
- Keep parse errors as retryable failures.
- Keep retry feedback typed enough for the model and UI to understand what
  failed.

## Work Items

- [x] Create this task tracker.
- [x] Relax CRP required-slot validation while preserving typed slot checks.
- [x] Update prompt/docs language so models understand the softer contract.
- [x] Add tests covering relaxed structural slots and strict typed slots.
- [x] Split CRP retry decisions into typed outcomes instead of returning `bool`.
- [x] Add one bounded retry for empty model responses.
- [x] Capture model `finish_reason` and use truncation-aware CRP retry guidance.
- [x] Separate budgets for tool calls, model rounds, CRP retries, empty
  responses, and transport retries.
- [x] Re-run focused formatting, tests, and package script checks.
