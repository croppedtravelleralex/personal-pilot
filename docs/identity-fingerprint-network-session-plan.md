# Identity / Fingerprint / Network / Session model plan

## Purpose

This document turns the current architectural direction into a concrete model plan for adding:
- fingerprint
- identity
- network / proxy
- session continuity

without collapsing product-layer semantics into the Lightpanda execution core.

The governing rule is:

> declare intent at the API layer,
> resolve and explain it in the control plane,
> consume only the truly supported minimum inside the runner.

---

## 1. Layering principle

### API layer should declare intent
Examples:
- what browser environment is desired
- what identity/session continuity is desired
- what network path is desired

### Control plane should resolve and explain
Examples:
- profile lookup
- compatibility checks
- region consistency checks
- selection / fallback / explainability
- determining which fields are actually consumable by the current runner

### Runner should consume only the minimum real subset
Examples:
- env vars / flags / args / runtime options
- applied fields vs ignored fields
- execution output / failure signals

This keeps product design stable while the engine layer matures.

---

## 2. Concept model

## 2.1 Fingerprint profile

Purpose:
- describe browser/device/runtime environment traits
- model “what environment should this browser look like?”

Suggested fields:
- `id`
- `name`
- `version`
- `platform`
- `user_agent`
- `accept_language`
- `locale`
- `timezone`
- `viewport_width`
- `viewport_height`
- `screen_width`
- `screen_height`
- `device_pixel_ratio`
- `hardware_concurrency`
- `device_memory_gb`
- `profile_json` (source payload / extension surface)
- `tags`
- `status`

Notes:
- fingerprint profile is not the same as user identity
- fingerprint profile is not the same as session continuity
- profile fields may exist before all of them are truly consumed by Lightpanda

---

## 2.2 Identity profile

Purpose:
- describe stable user-facing identity semantics above browser environment
- answer “what kind of user/persona should this browsing behavior resemble?”

Suggested fields:
- `id`
- `name`
- `version`
- `region`
- `country`
- `language`
- `locale`
- `timezone`
- `persona_type`
- `risk_tier`
- `behavior_hints_json`
- `preferred_fingerprint_profile_id`
- `preferred_network_profile_id`
- `status`

Identity profile may later influence:
- preferred locale/timezone alignment
- session reuse expectations
- navigation style or interaction strategy
- explainability summaries

Notes:
- identity is higher-level than fingerprint
- multiple fingerprint profiles may be compatible with one identity profile

---

## 2.3 Network profile

Purpose:
- describe network routing intent, not just one raw proxy
- answer “what network characteristics should this request use?”

Suggested fields:
- `id`
- `name`
- `version`
- `mode` (`required_proxy`, `preferred_proxy`, `direct`, `pool`)
- `target_region`
- `target_country`
- `provider_preference`
- `anonymity_requirement`
- `latency_preference`
- `rotation_policy`
- `sticky_session_supported`
- `network_policy_json`
- `status`

Notes:
- this should not be reduced forever to a single `proxy_id`
- a network profile may resolve to a concrete proxy at runtime
- this layer should support future pool-based selection

---

## 2.4 Session profile

Purpose:
- represent continuity across visits/tasks
- answer “what persists across repeated browser actions?”

Suggested fields:
- `id`
- `name`
- `version`
- `identity_profile_id`
- `fingerprint_profile_id`
- `network_profile_id`
- `cookie_jar_ref`
- `storage_state_ref`
- `session_region`
- `session_locale`
- `session_timezone`
- `continuity_mode`
- `last_used_at`
- `status`

Notes:
- session profile should model persistence and continuity
- this should stay separate from base fingerprint profile definition

---

## 3. Request model direction

The browser-facing API should gradually move toward explicit intent references.

Illustrative request direction:

```json
{
  "url": "https://example.com",
  "action": "extract_text",
  "fingerprint_profile_id": "fp-us-desktop-1",
  "identity_profile_id": "id-us-retail-1",
  "session_profile_id": "sess-us-retail-1",
  "network_profile_id": "net-us-residential-1",
  "proxy_id": "proxy-us-12",
  "timeout_seconds": 10
}
```

Rules:
- API layer should express desired intent references
- direct raw override fields should be used carefully
- control plane should decide what really reaches the runner

---

## 4. Control-plane responsibilities

The control plane should own the intelligence layer.

Core responsibilities:
- resolve IDs to concrete profiles
- check compatibility between fingerprint / identity / network / session
- decide final proxy selection
- compute region consistency
- compute trust / explainability signals
- surface what is actually consumable by the current runner
- preserve stable external response semantics even when runner capability is partial

Important output concepts:
- `applied_fields`
- `ignored_fields`
- `consumption_status`
- compatibility warnings
- selection explanations
- identity/network consistency explanations

This is where product truth should live.

---

## 5. Runner responsibilities

The Lightpanda runner should remain intentionally narrow.

Runner should own:
- real navigation / page-read execution
- consumption of supported runtime fields
- output of execution result and failure signals
- surfacing which runtime options were truly applied

Runner should NOT own:
- product-layer profile resolution
- broad identity reasoning
- high-level selection policy
- scheduling decisions
- long-term session orchestration policy

That prevents the execution engine from swallowing product logic.

---

## 6. Consumption model

The current and future model should explicitly distinguish:

### Declared intent
What the API/control plane asked for.

### Resolved runtime
What the control plane translated into executable context.

### Applied runtime
What the runner truly consumed.

### Ignored runtime
What was provided but not actually supported yet.

This model is critical for keeping fake/real runner behavior explainable.

---

## 7. Recommended evolution phases

### Phase 1 — current baseline
- browser-facing API v1 exists
- content/result contract exists
- fingerprint runtime consumption reporting exists in early form
- real `LIGHTPANDA_BIN` still missing on this machine

### Phase 2 — consumption explainability hardening
- strengthen `applied_fields`
- strengthen `ignored_fields`
- strengthen `consumption_status`
- add clearer compatibility / partial-support explanations

### Phase 3 — minimal real fingerprint consumption
- truly apply a bounded core subset of fingerprint fields
- verify env/arg propagation with real runner once binary exists
- confirm actual effects, not just metadata presence

Suggested first truly-consumed subset:
- `user_agent`
- `accept_language`
- `timezone`
- `locale`
- `viewport_width`
- `viewport_height`
- `screen_width`
- `screen_height`
- `device_pixel_ratio`
- `platform`

### Phase 4 — identity and session layering
- add `identity_profile_id`
- add `session_profile_id`
- separate continuity semantics from fingerprint semantics
- begin persistence design for cookie/storage continuity

### Phase 5 — network / region / trust convergence
- add `network_profile_id`
- strengthen region-consistency logic
- integrate proxy/network identity alignment into explainability and trust score

### Phase 6 — richer real-engine validation
- wire real `LIGHTPANDA_BIN`
- validate real runner consumption and output
- measure real gaps between declared intent and actual execution

---

## 8. Design guardrails

Do not:
- push all semantics into runner-specific fields
- treat fingerprint == identity
- treat session == fingerprint
- bind API contract too tightly to one engine’s implementation details
- claim real-engine validation from script-runner tests

Do:
- keep API intent stable
- keep control-plane resolution explicit
- keep runner consumption narrow and measurable
- keep explainability honest about partial support

---

## 9. Practical summary

The long-term clean architecture is:

- **API layer** says what kind of browser/use context is desired
- **control plane** resolves, checks, explains, and selects
- **Lightpanda runner** executes the minimum real subset it truly supports

That is the safest path for adding fingerprint, identity, network, and session capability without turning the execution engine into an unmaintainable god-object.
