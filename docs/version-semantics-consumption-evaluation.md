# Version Semantics Consumption Evaluation (2026-04-02)

## Question

After providerScope validation, should **selection** and/or **explain** explicitly consume `provider_risk_version_seen` drift?

## Current state

Already implemented:
- `provider_risk_snapshots.version`
- `proxies.provider_risk_version_seen`
- `provider_scope_flip -> lazy_current_proxy`

Current behavior:
- providerScope drift is tolerated for non-current proxies
- selection still primarily consumes `cached_trust_score`
- explain still reports current trust/candidate view without explicitly surfacing version drift semantics

## Evaluation

### 1. Selection path

Recommendation: **not immediately consume version drift as a hard gate**.

Reason:
- current v1 is intentionally conservative
- forcing selection to actively reconcile version drift now would expand scope from refresh optimization into ranking semantics redesign
- providerScope收益判断 is good enough to justify keeping selection stable for one more stage

Suggested next move:
- keep selection behavior unchanged for now
- only add explicit selection-side version handling if later samples show stale cached trust is causing ranking mistakes

### 2. Explain path

Recommendation: **prefer explain-side visibility before selection-side behavior change**.

Reason:
- explain is safer than selection as a next consumer
- if version drift needs to be visible, explain can expose it without changing ranking behavior
- this preserves the current conservative rollout strategy

Suggested next move:
- if needed, add a lightweight explain field like `provider_risk_version_current` vs `provider_risk_version_seen`
- avoid changing explain summary wording until there is proof this helps decisions

### 3. Minimal consistency boundary

Recommended boundary:
- **verify / scoped refresh path:** should align current proxy to latest provider risk version
- **selection path:** may temporarily tolerate non-current proxy stale cached trust
- **explain path:** can become the first place to surface version drift if visibility becomes necessary
- **providerRegion path:** remains deferred

## Decision

> **Next stage should evaluate explain as the first possible consumer of version semantics, while keeping selection behavior unchanged unless real ranking errors appear.**

## Final recommendation

1. Keep providerRegion deferred
2. Do not expand selection semantics yet
3. If a new consumer is needed, start with explain visibility rather than selection behavior
