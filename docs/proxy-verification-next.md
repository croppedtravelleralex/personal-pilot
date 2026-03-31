# Proxy Verification Next Step Design

## Goal
Move proxy verification from minimal smoke checks to a more realistic external validation chain that can answer:
- can the proxy reach a real upstream target?
- what exit IP / country / region does it present?
- what anonymity signals does it leak?
- should the proxy score / cooldown / selection weight change?

## Proposed staged validation chain

### Stage 1: local smoke (already done)
- TCP connect
- HTTP CONNECT response shape
- upstream echo signal
- rough anonymity classification

### Stage 2: external probe
Use a configurable probe endpoint that returns JSON like:

```json
{
  "ip": "203.0.113.8",
  "country": "US",
  "region": "Virginia",
  "via": "...",
  "forwarded": "...",
  "x_forwarded_for": "...",
  "user_agent": "..."
}
```

Add environment variables:
- `AUTO_OPEN_BROWSER_PROXY_PROBE_URL`
- `AUTO_OPEN_BROWSER_PROXY_PROBE_TIMEOUT_MS`
- `AUTO_OPEN_BROWSER_PROXY_PROBE_EXPECT_COUNTRY` (optional)

### Stage 3: verdict model
Compute and persist:
- `probe_ok`
- `exit_ip`
- `exit_country`
- `exit_region`
- `anonymity_level`
- `geo_match_ok`
- `verification_score_delta`

## Suggested persistence additions
Add to `proxies`:
- `last_exit_country`
- `last_exit_region`
- `last_geo_match_ok`
- `last_probe_latency_ms`
- `last_probe_error`

## Suggested scoring rules V1
- protocol_ok=false -> strong negative
- upstream_ok=false -> stronger negative
- transparent -> small negative
- anonymous -> neutral
- elite -> small positive
- region/country mismatch -> negative
- stable passing repeated probes -> gradual positive

## Suggested API evolution
Current:
- `POST /proxies/:id/smoke`

Next:
- keep `POST /proxies/:id/smoke` as fast check
- add `POST /proxies/:id/verify` as slower external probe
- optionally add `POST /proxies/verify-batch` later

## Recommended implementation order
1. add external probe design + env vars
2. add `verify` response DTO with country/region fields
3. persist geo/anonymity signals
4. hook verification score delta into proxy health score updates
