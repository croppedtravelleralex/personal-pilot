# Provider Risk V1 Decision (2026-04-02)

## Decision

Continue with **providerScope-only** validation and **defer providerRegion** expansion for the next stage.

## Why

Recent validation samples confirm:

- `provider_scope_flip` now consistently lands on **`lazy_current_proxy`**
- provider-level cached trust refresh is no longer observed in the sampled v1 path
- `provider_region_scope_flip` still exists, but only on the provider-region path and is not yet the dominant hotspot

## Current conclusion

The current v1 implementation is already strong enough to support a conservative next-step decision:

> **Do not expand to providerRegion yet. Keep validating providerScope收益 first.**

## Next-stage focus

1. Add a few more real-path samples to strengthen the providerScope收益 conclusion
2. Keep providerRegion in evaluation, not implementation
3. Revisit providerRegion only after providerScope收益判断 is stable enough

## Reinforcement sample (round 3)

- providerScope lazy hits: **3**
- providerRegion scope hits: **1**
- proxy refresh samples: **[4, 8, 4] ms**
- providerRegion refresh samples: **[22] ms**
- status samples: **[7] ms**

### Reinforced judgment
The round-3 sample still supports the same conservative decision: keep validating providerScope收益 and continue deferring providerRegion implementation.
