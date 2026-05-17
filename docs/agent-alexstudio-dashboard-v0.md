# agent.alexstudio.top dashboard v0

## Goal
先做一个轻量 dashboard，不追求花哨，先能监测 token 开销与请求状态。

## Recommended path
- API gateway backend keeps usage events.
- Dashboard reads:
  - `GET /admin/usage`
  - `GET /admin/stats`
- Suggested dashboard host:
  - `lab.alexstudio.top`
  - or `/admin` behind the gateway machine

## v0 metrics
- total events
- by token
- by status code
- by model
- recent requests

## Why this first
- Data comes from your own gateway, not guessed from CF traffic.
- Fastest path to something useful.
- Good enough for your own testing stage.

## Next step after v0
- add token cost estimate
- add hourly aggregation
- add upstream latency buckets
- add real token usage from upstream when available
