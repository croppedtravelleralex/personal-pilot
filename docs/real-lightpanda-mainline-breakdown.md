# Real Lightpanda Mainline Breakdown (2026-04-03)

## Why this is the next real mainline

After resetting progress reporting, one of the largest unfinished blocks is no longer documentation/control-surface work.
It is the **real Lightpanda execution deepening** line.

This line matters because the project cannot honestly approach completion while real execution remains relatively shallow.

## Current conservative estimate

Current completion: **~40%**

## Mainline goal

Turn the current Lightpanda path from a minimal real-runner boundary into a more reliable, observable, and operationally useful real execution path.

The externally-facing product target for this line is now explicit:

> **The fingerprint browser should ultimately be operated through a clear API surface.**

That means the project should converge toward a browser-facing API product shape, while the current task system remains the underlying scheduling/control layer.

## Work modules

### 1. Execution-path hardening
Status: **~45%**
Includes:
- stdout / stderr capture stability
- timeout / exit-code handling maturity
- failure surfacing consistency
- runner error categorization

### 2. Real capability expansion beyond minimal fetch
Status: **~35%**
Includes:
- moving beyond the current minimal fetch-style action
- clarifying what real browser actions are first-class in v1
- defining safe progression from minimal execution to richer browser automation
- defining the minimal browser-facing API surface that should sit above the task layer

Current v1 browser API candidates:
- `open_page`
- `get_html`
- `get_title`
- `get_final_url`
- `extract_text`

### 3. Runner observability and artifact quality
Status: **~35%**
Includes:
- clearer run summaries
- better artifact/log surfacing for real execution
- stronger distinction between runner failure vs task failure vs browser failure

### 4. Fingerprint consumption boundary inside real runner
Status: **~25%**
Includes:
- which profile fields are truly consumed by Lightpanda path
- how unsupported fields are surfaced instead of silently ignored
- keeping fake/real runner input model aligned while real consumption deepens

## Recommended order

1. **execution-path hardening first**
2. **runner observability/artifact quality second**
3. **fingerprint real-consumption boundary third**
4. **capability expansion beyond minimal fetch last**

## Why this order

Because execution-path reliability and observability are prerequisites.
There is little value in expanding real browser capability if the real runner path is still weak to diagnose or trust.

## Stage recommendation

> The next major unfinished mainline should start with **real Lightpanda execution-path hardening**, not with providerRegion expansion and not with more control-surface refinement.

## Product-shape clarification

The project already has a usable internal API entry path through `POST /tasks` plus the Lightpanda runner.
But that is still a task-oriented control surface, not yet a clear browser-product API.

So the intended convergence is:
- browser-facing API surface = external operation entry
- task system = internal scheduling/control plane
- lightpanda runner = real fingerprint-browser execution core

This should guide future endpoint design and naming.

## Why Lightpanda should currently be treated as an execution core, not the whole product

This is the correct current framing because:

- the browser-facing API surface is what has already become concrete in this repo
- the result contract and content contract are being shaped above the runner layer
- real `LIGHTPANDA_BIN` wiring is still missing on this machine
- script-runner / framework validation has been done, but real-engine validation is still blocked

So the stable architecture boundary is:
- product layer = browser-facing API
- control layer = task / scheduling / status / logs
- execution layer = Lightpanda

That framing is more honest and more durable than presenting Lightpanda itself as the full external product.

## API layer vs execution core responsibilities

### Browser-facing API layer should own
- endpoint naming
- request / response contracts
- product-facing result semantics
- stable external operation entry

### Task / control plane should own
- scheduling
- retries
- status transitions
- logs / runs / visibility

### Lightpanda execution core should own
- page visit / navigation execution
- bounded content extraction execution
- runner-level success / failure signals
- real browser-engine behavior underneath the API
