# Explain Visibility Version Fields Plan (2026-04-02)

## Goal

If provider-risk version semantics need a new consumer, start with **explain visibility** rather than changing selection behavior.

## Why explain first

- safer than changing ranking behavior
- easier to validate
- preserves current conservative rollout of providerScope lazy refresh
- helps detect version drift without forcing immediate cache or ranking redesign

## Proposed fields

For `/proxies/:id/explain`, consider adding:

- `provider_risk_version_current`
- `provider_risk_version_seen`
- `provider_risk_version_status`
  - `aligned`
  - `stale`
  - `not_applicable`

## Minimal semantics

- if proxy has no provider: `not_applicable`
- if current version == seen version: `aligned`
- if current version != seen version: `stale`

## Non-goals

- do not change selection ordering
- do not auto-refresh non-current proxies from explain path in this stage
- do not expand providerRegion in this stage

## Recommendation

> The next concrete implementation candidate should be **adding explain-side version visibility fields**, not changing selection semantics.


## Minimal wording boundary

Recommended API-facing wording:

- `aligned` -> provider risk version is up to date
- `stale` -> provider risk version changed after this proxy cache was last refreshed
- `not_applicable` -> no provider-linked version state applies

Recommended UI/summary boundary:

- expose these fields as structured machine-readable fields first
- do **not** force them into the main human summary sentence in this stage
- only surface them in human wording when there is evidence they improve operator decisions
