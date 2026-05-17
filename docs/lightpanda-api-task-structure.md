# Lightpanda / API / Task System structure

## Three-layer view

### 1. Browser-facing API layer
Purpose:
- external product entry
- stable operation surface
- clear request / response contracts

Examples:
- POST /browser/open
- POST /browser/html
- POST /browser/title
- POST /browser/final-url
- POST /browser/text

Owns:
- endpoint naming
- product-facing semantics
- unified content contract
- browser API examples / docs

---

### 2. Task / control layer
Purpose:
- internal control plane
- queueing / scheduling / retries / visibility

Examples:
- task creation
- status transitions
- runs / logs
- retry / cancel

Owns:
- scheduling
- retries
- state transitions
- observability / logs / run history

---

### 3. Lightpanda execution core
Purpose:
- real page execution
- navigation / read-style browser actions
- runner-level output and failure signals

Examples:
- POST /browser/open
- POST /browser/html
- POST /browser/title
- POST /browser/final-url
- POST /browser/text

Owns:
- actual execution against a browser engine
- bounded result generation
- runner-level failure reasons
- browser-engine-side behavior

---

## Task Contract / Control-Plane Visibility V1

Current control-plane contract is now anchored on three stable views of the same execution:

1. GET /status
   - top-level system view
   - current counts
   - latest task list
   - latest browser-ready task list
   - latest execution summary artifacts

2. GET /tasks/:id
   - canonical task detail view
   - current task status
   - current explainability snapshot
   - current execution identity snapshot

3. GET /tasks/:id/runs
   - drill-down execution history
   - per-attempt trace metadata
   - run-level explainability snapshot
   - run-level summary artifacts

These three views now share the same V1 contract surface for execution identity and failure semantics.

---

## Execution identity V1

`execution_identity` is now the unified task/run/status-facing identity snapshot.

It is the stable outward shape for:
- fingerprint profile identity
- fingerprint resolution status
- fingerprint runtime explainability
- proxy identity
- proxy resolution status
- selection summary
- selection explainability
- trust score total

Current fields:
- fingerprint_profile_id
- fingerprint_profile_version
- fingerprint_resolution_status
- fingerprint_runtime_explain
- proxy_id
- proxy_provider
- proxy_region
- proxy_resolution_status
- selection_reason_summary
- selection_explain
- trust_score_total

This means the control plane no longer requires downstream readers to reconstruct identity from scattered proxy / fingerprint / selection fields.

---

## Cancelled terminal semantics

`cancelled` is now a formal terminal state.

Current V1 rules:
- task detail returns `status=cancelled` when a running task is cancelled successfully
- runs view returns the corresponding run with `status=cancelled`
- /status counts cancelled tasks in top-level counts and can surface the cancelled task in latest task views
- cancelled execution writes structured result fields instead of relying on an implicit empty result

Current cancelled failure contract:
- `status = cancelled`
- `error_kind = runner_cancelled`
- `failure_scope = runner_cancelled`
- `message = task cancelled while running`

This keeps cancellation separate from generic execution failure and timeout semantics.

---

## Consistency contract across status / detail / runs

For the same task, the following fields are expected to stay aligned across the three views whenever they are present:
- execution_identity
- failure_scope
- browser_failure_signal
- summary_artifacts
- browser content summary fields
- high-level selection / fingerprint explainability fields

Interpretation rule:
- /status is the top-level operational snapshot
- /tasks/:id is the canonical task state snapshot
- /tasks/:id/runs is the attempt-level drill-down view

The views are intentionally different in scope, but they should not disagree on the same execution identity or terminal meaning.

---

## Why this boundary matters

If these three layers are mixed together too early:
- the external product surface becomes unstable
- runner reality gets overstated
- execution gaps look like product gaps
- product naming gets dragged around by engine details

If they stay separated:
- API can stabilize before engine maturity is complete
- control-plane features stay reusable
- Lightpanda can evolve underneath without constantly breaking the product surface

---

## Current reality

Right now the repo is strongest at:
- API surface shaping
- control-plane continuity
- runner contract / framework validation
- execution identity visibility
- cancelled terminal-state closure

Right now the repo is still blocked at:
- real LIGHTPANDA_BIN wiring on this machine
- real-engine validation with a confirmed binary beyond stubbed cancel verification

So the correct current summary is:

> browser-facing API is becoming the product surface,
> task system is the control plane,
> Lightpanda is the execution core,
> and Task Contract / Control-Plane Visibility V1 is the current stability layer between them.
