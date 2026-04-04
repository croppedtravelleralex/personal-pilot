# API and Ops Notes

## Core API surface

### Health / status
- `GET /health`
- `GET /status`

### Browser-facing API v1
- `POST /browser/open`
  - maps to task kind: `open_page`
- `POST /browser/html`
  - maps to task kind: `get_html`
- `POST /browser/title`
  - maps to task kind: `get_title`
- `POST /browser/final-url`
  - maps to task kind: `get_final_url`
- `POST /browser/text`
  - maps to task kind: `extract_text`

Current browser-facing API v1 contract:
- all endpoints accept `url`
- optional fields: `timeout_seconds`, `priority`, `fingerprint_profile_id`, `proxy_id`, `network_policy_json`
- current product shape: browser-facing API is the external entry surface; task queue remains the underlying control plane

Current result-shape notes:
- `get_html` currently returns `content_kind=text/html` plus `html_preview`, `html_length`, `html_truncated`
- `extract_text` currently returns `content_kind=text/plain` plus `text_preview`, `text_length`, `text_truncated`
- both content-oriented actions now also expose unified fields: `content_preview`, `content_length`, `content_truncated`
- result depth is still evolving; current previews are useful for lightweight inspection, not yet the final rich content contract

### Tasks
- `POST /tasks`
- `GET /tasks/:id`
- `POST /tasks/:id/retry`
- `POST /tasks/:id/cancel`
- `GET /tasks/:id/runs`
- `GET /tasks/:id/logs`

### Fingerprint profiles
- `POST /fingerprint-profiles`
- `GET /fingerprint-profiles`
- `GET /fingerprint-profiles/:id`

### Proxies
- `POST /proxies`
- `GET /proxies`
- `GET /proxies/:id`
- `POST /proxies/:id/smoke`

## Proxy smoke response fields
- `reachable`
- `protocol_ok`
- `upstream_ok`
- `exit_ip`
- `anonymity_level`
- `latency_ms`
- `status`
- `message`

## Persisted proxy verification fields
- `last_smoke_status`
- `last_smoke_protocol_ok`
- `last_smoke_upstream_ok`
- `last_exit_ip`
- `last_anonymity_level`
- `last_smoke_at`

## Runtime tuning
### Runner env vars
- `AUTO_OPEN_BROWSER_RUNNER_CONCURRENCY`
- `AUTO_OPEN_BROWSER_RUNNER_RECLAIM_SECONDS`
- `AUTO_OPEN_BROWSER_RUNNER_HEARTBEAT_SECONDS`
- `AUTO_OPEN_BROWSER_RUNNER_CLAIM_RETRY_LIMIT`
- `AUTO_OPEN_BROWSER_RUNNER_IDLE_BACKOFF_MIN_MS`
- `AUTO_OPEN_BROWSER_RUNNER_IDLE_BACKOFF_MAX_MS`
- `AUTO_OPEN_BROWSER_RUNNER_IDLE_BACKOFF_JITTER_MS`
- `AUTO_OPEN_BROWSER_RUNNER_ERROR_BACKOFF_MAX_MS`

## Current ops guidance
- Prefer `/status` for top-level system view
- Use task detail + runs + logs for incident drill-down
- Use proxy `smoke` before promoting uncertain proxies
- Treat `transparent` anonymity as lower trust than `anonymous` / `elite`
- Reclaim-related tests that only verify state transitions should avoid background worker noise

## Known current limitations
- browser-facing API v1 currently focuses on entry unification; result-shape depth is still evolving
- runner support is still staged, not a full rich browser automation surface yet
- proxy verification is still V1 and not a full external probe chain
- no dedicated `/proxies/:id/verify` slow path yet
- proxy selection is still light query-based ordering
- status metrics are not yet a fully separate stats subsystem
