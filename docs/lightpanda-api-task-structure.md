# Lightpanda / API / Task System structure

## Three-layer view

### 1. Browser-facing API layer
Purpose:
- external product entry
- stable operation surface
- clear request / response contracts

Examples:
- `POST /browser/open`
- `POST /browser/html`
- `POST /browser/title`
- `POST /browser/final-url`
- `POST /browser/text`

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
- `open_page`
- `get_html`
- `get_title`
- `get_final_url`
- `extract_text`

Owns:
- actual execution against a browser engine
- bounded result generation
- runner-level failure reasons
- browser-engine-side behavior

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

Right now the repo is still blocked at:
- real `LIGHTPANDA_BIN` wiring on this machine
- real-engine validation with a confirmed binary

So the correct current summary is:

> browser-facing API is becoming the product surface,
> task system is the control plane,
> Lightpanda is the execution core still waiting for real binary wiring.
