# Stage-2 Boundary Plan (2026-04-02)

## Goal

Keep stage 2 narrow enough to preserve the gains from providerScope stabilization.

## Stage-2 should include

1. providerRegion entry-condition validation
2. providerRegion-focused refresh/caching design only if entry conditions are met
3. small profiling loops that justify or reject providerRegion implementation

## Stage-2 should exclude

1. selection ranking redesign
2. broad explainability rewrite
3. new trust-score semantics unrelated to current refresh-scope work
4. unrelated fingerprint / runner / scheduling expansion

## Mainline recommendation

> Stage 2 should be a **providerRegion decision stage**, not a broad architecture expansion stage.
