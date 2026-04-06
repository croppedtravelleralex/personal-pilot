# agent.alexstudio.top gateway runbook

## Start locally
```bash
export GATEWAY_BIND=127.0.0.1:8787
export GATEWAY_ADMIN_TOKEN=change-me-admin
export GATEWAY_DOWNSTREAM_TOKENS_JSON='[{"key":"local-test-token","label":"alex-local","enabled":true}]'
# optional when upstream is ready
export UPSTREAM_BASE_URL=https://your-upstream.example.com
export UPSTREAM_BEARER_TOKEN=your-upstream-token
cargo run -- gateway
```

## Smoke test
```bash
curl http://127.0.0.1:8787/health
curl -H 'Authorization: Bearer local-test-token' http://127.0.0.1:8787/v1/models
curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H 'Authorization: Bearer local-test-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"agent-proxy-v0","messages":[{"role":"user","content":"hello"}]}'
curl -H 'Authorization: Bearer change-me-admin' http://127.0.0.1:8787/admin/usage
```

## Suggested Cloudflare path
- Keep `agent.alexstudio.top` proxied by Cloudflare.
- Point origin to the machine running the gateway.
- Do not expose upstream credentials through Cloudflare rules.

## v0 notes
- `sanitize_chat_payload` strips dangerous auth-like fields from downstream payloads.
- Downstream auth and upstream auth are isolated.
- Upstream timeout / invalid json / unavailable are classified separately.
