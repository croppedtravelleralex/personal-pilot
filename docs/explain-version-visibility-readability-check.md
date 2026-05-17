# Explain Version Visibility Readability Check (2026-04-02)

## Goal

Validate whether the new explain-side version visibility fields are understandable enough without forcing them into the main summary sentence.

## Checked fields

- `provider_risk_version_current`
- `provider_risk_version_seen`
- `provider_risk_version_status`

## Readability result

Current judgment: **readable enough as structured fields**.

Why:
- the field names are explicit
- `aligned / stale / not_applicable` is simple enough to interpret
- the fields support operator diagnosis without requiring ranking-behavior changes
- they do not need to be promoted into the main human summary sentence yet

## Recommended interpretation

- `aligned`: this proxy cache has already seen the current provider risk version
- `stale`: provider risk changed after this proxy cache was last refreshed
- `not_applicable`: no provider-linked version state applies

## Decision

> Keep these fields as structured explain visibility for now. Do not force them into the primary summary sentence in this stage.

## Follow-up threshold

Only consider stronger human wording if later usage shows that operators miss important state when reading the explain response.
