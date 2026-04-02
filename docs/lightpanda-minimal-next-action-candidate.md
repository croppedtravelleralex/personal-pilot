# Lightpanda Minimal Next Action Candidate (2026-04-03)

## Decision

The best candidate for the **next minimal bounded action** is:

> **`get_html`**

## Why this is the best next action

### 1. It stays close to the current execution path
The current Lightpanda real path already uses a fetch-style command and already captures stdout/stderr.

That means `get_html` is much closer to the current boundary than actions like:
- screenshot
- run_script
- click
- multi-step browser workflows

### 2. It expands capability without exploding scope
`get_html` is still a **single-step page-read style action**.
It does not require introducing:
- DOM interaction semantics
- element selection contracts
- artifact storage strategy
- multi-step browser state transitions

### 3. It matches the current bounded-expansion principle
Current stage priority is not “add many browser actions”.
It is:
- keep the runner diagnosable
- keep action contracts explicit
- expand only one step at a time

`get_html` fits that rule well.

## Why not screenshot first
Screenshot sounds attractive, but it would pull in more unresolved questions:
- file/artifact persistence
- binary output handling
- storage lifecycle
- response-size / retention strategy

That is a wider expansion than needed for the next step.

## Why not script execution first
Script execution would create much larger contract ambiguity:
- what script language?
- what execution sandbox?
- what result format?
- what timeout / side-effect model?

That is too wide for the next bounded move.

## Proposed action contract direction

Candidate request shape:

```json
{
  "action": "get_html",
  "url": "https://example.com",
  "timeout_seconds": 10
}
```

Candidate result direction:
- keep existing runner diagnostics
- add a bounded `content_preview` / `html_preview` style field first
- do **not** jump straight to full artifact persistence in the first step

## Recommendation

> The next implementation move should start by validating whether `get_html` can safely reuse the current fetch-style Lightpanda path with explicit result semantics.
