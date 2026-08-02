# Proxy Batch Verify / Periodic Inspection Plan

## Goal
Turn single-proxy `smoke` / `verify` into a controllable batch verification pipeline.

## Why now
Current proxy health model already stores:
- smoke status
- verify status
- exit ip / country / region
- geo match result
- anonymity level

What is missing is the refresh mechanism.

## Proposed API

### 1. batch verify trigger
`POST /proxies/verify-batch`

Request draft:
```json
{
  "provider": "pool-a",
  "region": "us-east",
  "limit": 50,
  "only_stale": true,
  "min_score": 0.3
}
```

Response draft:
```json
{
  "requested": 50,
  "accepted": 37,
  "skipped": 13,
  "status": "scheduled"
}
```

### 2. verification policy
Add optional fields:
- `stale_after_seconds`
- `concurrency`
- `cooldown_on_failure_seconds`
- `promote_on_success`

## Execution model options

### Option A: inline loop
- simple
- bad for latency
- not recommended past tiny batches

### Option B: create verify tasks in task queue
- reuses existing DB-first queue
- allows retry / logs / status reuse
- easier to observe
- recommended

## Recommended direction
Represent batch verify as normal tasks:
- `kind = verify_proxy`
- input contains `proxy_id`
- runner path calls existing verify logic

Then batch trigger only schedules N tasks.

## Staleness rules
Prefer verifying proxies when:
- `last_verify_at IS NULL`
- or `now - last_verify_at > stale_after_seconds`
- or `last_verify_status != 'ok'`
- or `cooldown_until` recently elapsed and proxy should be reconsidered

## Batch selection priority
Suggested order:
1. provider / region match
2. stale verify first
3. previously failed but cooldown expired
4. higher score first only after freshness rules

## Periodic inspection
Recommended periodic jobs later:
- fast smoke sweep every 10-20 min for recently used proxies
- slower verify sweep every 1-3 h for active pools
- nightly wider verify for low-confidence proxies

## Metrics to expose later
- verified_ok_count
- verified_failed_count
- stale_verify_count
- geo_match_ok_count
- anonymity_distribution
- avg_verify_latency_ms

## Minimal implementation order
1. add plan/doc (this file)
2. add `verify_proxy` task kind
3. add `POST /proxies/verify-batch`
4. persist verification logs / metrics summary
5. add periodic inspection trigger
