# ProviderRegion Entry Conditions (2026-04-02)

## Purpose

Define exactly when providerRegion should move from “deferred” into an active implementation stage.

## Current position

providerRegion remains deferred because:
- providerScope was the dominant hotspot
- providerScope lazy refresh has only recently reached a stable phase
- selection behavior is intentionally unchanged
- explain-side version visibility was a safer next consumer than region-scope expansion

## Entry conditions

providerRegion should enter implementation only when **all** of the following are true:

### 1. providerScope conclusion is stable
- repeated samples continue to show providerScope lazy refresh as the normal path
- no new evidence suggests providerScope needs a second redesign pass

### 2. current explain/selection boundary stays acceptable
- explain-side visibility is readable enough
- selection is not showing ranking mistakes caused by stale provider-scope cache state

### 3. providerRegion becomes a demonstrated bottleneck
At least one of the following should be observed:
- repeated providerRegion scope hits in real-path samples
- providerRegion refresh cost becomes materially noticeable
- operator-visible issues are traced to provider-region drift rather than providerScope drift

### 4. stage-2 scope is protected
- providerRegion work is isolated from selection redesign
- providerRegion work is isolated from broader explainability rewrites
- implementation can stay as a bounded refresh/caching change, not a full semantics rewrite

## Do-not-enter signals

providerRegion should stay deferred if any of the following is true:
- providerScope still needs more redesign
- selection ranking correctness is still under question
- explain visibility is still unstable or unclear
- providerRegion is only hypothetically useful, without sample-backed pressure

## Stage-2 boundary

### Can enter stage 2
- providerRegion entry-condition validation
- providerRegion refresh/caching design constrained to region scope only
- limited profiling to justify the expansion

### Should stay out of stage 2 for now
- selection ranking redesign
- broad trust-score semantics rewrite
- large explainability wording overhaul
- unrelated proxy/fingerprint strategy changes

## Decision

> providerRegion should not enter because it is interesting; it should enter only after providerScope is stable, current boundaries are acceptable, and providerRegion itself becomes a demonstrated bottleneck.
