# ADR-001: Browser pool not wired for local self-use

Status: Accepted  
Date: 2026-07-11  
Related: docs/54 CP5

## Decision

Do **not** wire `internal/pool` into the production browser start path for the local self-use scope.

## Context

- `internal/pool` has Slot/Budget designs but zero production callers.
- Local self-use prioritizes one-shot start + CP1 port reserve + CP2 concurrency budget + CP3/CP4 orphan/detached recovery.
- Prewarm pool would add complexity without measured cold-start pain on this machine.

## Consequences

- Keep `internal/pool` as experimental / unused.
- Revisit only if cold-start latency becomes a measured blocker for N≥10 concurrent profiles.
- CP2 `MaxConcurrentInstances` remains the backpressure mechanism.
