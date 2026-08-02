# Agent AlexStudio Gateway Runbook
Updated: 2026-04-16 (Asia/Shanghai)

## Purpose

This file is the gateway-specific operational entrypoint.
It is intentionally thin: the canonical product state still lives in `docs/02-current-state.md` and `docs/final-goal-progress-breakdown.md`.

## Read First

1. `docs/02-current-state.md`
2. `docs/final-goal-progress-breakdown.md`
3. `docs/agent-alexstudio-gateway-v0.md`

## Truth Markers

- Gateway shell health is not the same as upstream readiness.
- `upstream_configured=false` means the shell is online, not that the gateway is productized.
- `verify_proxy` traffic volume does not count as browser mainline acceptance.
- real-upstream acceptance blocked by current runtime state until the upstream path is explicitly configured and verified.

## Minimal Operating Rule

1. Keep the gateway private and local-first.
2. Keep auth headers and upstream secrets out of downstream logs.
3. Treat the gateway as an entry surface, not as a replacement for the canonical desktop app state.

## Acceptance

- The gateway should only be considered ready when the upstream path is explicitly configured and verified.
- If the upstream path is not configured, report shell-online status honestly and stop there.
