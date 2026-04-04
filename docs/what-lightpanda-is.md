# What Lightpanda Is (for this project)

## One-line definition

Lightpanda should currently be treated as:

> a **real browser-execution core for page access and lightweight content extraction**, sitting behind a browser-facing API surface.

It is **not** the whole product.
It is **not yet** a full browser automation platform.
It is the execution engine layer that should sit underneath the API and task/control plane.

---

## Product position in this repo

The project should converge to this stack:

- **browser-facing API surface** = external product entry
- **task system** = internal scheduling / control plane
- **Lightpanda runner** = real fingerprint-browser execution core

So the project is not “just expose Lightpanda directly”.
The project is:
- shape browser capabilities as a stable API product
- keep scheduling / retries / status / logs in the control plane
- let Lightpanda handle real execution underneath

---

## What Lightpanda is good for right now

At the current stage, Lightpanda is best used for bounded browser actions that keep scope under control.

### V1-suitable actions

- `open_page`
  - visit a URL
  - confirm basic success/failure

- `get_html`
  - return bounded HTML-oriented output
  - support preview / length / truncation style metadata

- `get_title`
  - return page title information

- `get_final_url`
  - resolve redirect destination / final landing URL

- `extract_text`
  - return bounded text-oriented output
  - support preview / length / truncation style metadata

These actions fit the current project direction because they:
- are single-step page-read style actions
- keep contracts narrow and explicit
- work well with browser-facing API productization
- do not force multi-step browser workflow semantics too early

---

## What Lightpanda should NOT be treated as yet

At this stage, Lightpanda should **not** be marketed or planned as if it already fully covers:

- complex DOM interaction workflows
- click / type / multi-step browser operation orchestration
- screenshot / PDF / heavy artifact lifecycle
- general script execution platform
- full Playwright/Puppeteer-class browser automation replacement
- deeply integrated fingerprint+proxy orchestration with mature guarantees

Those may become later phases, but they should not be assumed in V1.

---

## Current reality check

What is already true in this repo:

- browser-facing API entry surface already exists
  - `/browser/open`
  - `/browser/html`
  - `/browser/title`
  - `/browser/final-url`
  - `/browser/text`
- result contracts have already started forming
- content-oriented result fields already exist
- Lightpanda runner contract / framework validation has been done through script-runner style tests

What is **not yet** true:

- real Lightpanda binary integration is not yet confirmed on this machine
- `LIGHTPANDA_BIN` is not currently wired to a real binary
- true real-engine validation is therefore still blocked

That means the current repo state is best described as:

> **API product surface + runner contract are maturing, while real Lightpanda engine wiring is still pending.**

---

## Engineering implication

The right order is:

1. keep refining the browser-facing API product surface
2. keep result contracts explicit and stable
3. wire real `LIGHTPANDA_BIN`
4. validate real integration
5. only then deepen into heavier browser-automation capability

This avoids the common trap of making the surface look powerful before the engine layer is actually stable.

---

## Practical rule

When discussing Lightpanda in this project, default to this wording:

> Lightpanda is the **real execution core** behind the browser-facing API, not the API product itself.

That keeps product boundaries clean.
