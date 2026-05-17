# Browser-facing API v1 examples

This note gives concrete request / response examples for the current browser-facing API v1 surface.

## Shared request shape

All browser-facing API v1 endpoints currently accept:
- `url`
- optional `timeout_seconds`
- optional `priority`
- optional `fingerprint_profile_id`
- optional `proxy_id`
- optional `network_policy_json`

## Example: POST /browser/text

Request:

```json
{
  "url": "https://example.com/article",
  "timeout_seconds": 10,
  "network_policy_json": {
    "mode": "required_proxy",
    "proxy_id": "proxy-browser-text-1"
  }
}
```

Current task-creation response shape:

```json
{
  "id": "task-123",
  "kind": "extract_text",
  "status": "queued",
  "priority": 0,
  "fingerprint_profile_id": null,
  "fingerprint_profile_version": null,
  "fingerprint_resolution_status": null,
  "proxy_id": null,
  "proxy_provider": null,
  "proxy_region": null,
  "proxy_resolution_status": "pending",
  "trust_score_total": null,
  "selection_reason_summary": null,
  "selection_explain": null,
  "fingerprint_runtime_explain": null,
  "identity_network_explain": null,
  "winner_vs_runner_up_diff": null,
  "summary_artifacts": []
}
```

Illustrative runner result shape for `extract_text`:

```json
{
  "runner": "lightpanda",
  "requested_action": "extract_text",
  "action": "extract_text",
  "content_kind": "text/plain",
  "text_preview": "hello world from lightpanda",
  "text_length": 27,
  "text_truncated": false,
  "content_preview": "hello world from lightpanda",
  "content_length": 27,
  "content_truncated": false,
  "content_encoding": "plain",
  "content_source_action": "extract_text",
  "content_ready": true
}
```

## Example: POST /browser/html

Request:

```json
{
  "url": "https://example.com/page",
  "timeout_seconds": 9
}
```

Illustrative runner result shape for `get_html`:

```json
{
  "runner": "lightpanda",
  "requested_action": "get_html",
  "action": "get_html",
  "content_kind": "text/html",
  "html_preview": "<html><body>ok</body></html>",
  "html_length": 28,
  "html_truncated": false,
  "content_preview": "<html><body>ok</body></html>",
  "content_length": 28,
  "content_truncated": false,
  "content_encoding": "html",
  "content_source_action": "get_html",
  "content_ready": true
}
```

## Current contract notes

- current browser-facing API v1 is still entry-focused; richer result depth is evolving
- `content_*` fields are the preferred unified content contract for content-oriented actions
- action-specific fields like `html_preview` / `text_preview` remain available for debugging and transition
- current examples show contract shape, not a guarantee that a real Lightpanda binary is already wired on this machine
