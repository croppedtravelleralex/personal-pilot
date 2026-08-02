# API Operations Guide

This guide is the operator-facing acceptance flow for Task Contract / Control-Plane Visibility V1.

## Goals

Use this flow to validate that:
- task creation works
- task detail reflects the canonical task state
- task runs reflects per-attempt state
- /status reflects the same execution contract at system level
- cancellation closes as `status=cancelled` with `failure_scope=runner_cancelled`

---

## Base URL

```bash
BASE_URL=http://127.0.0.1:3000
```

---

## 1. Create a task

```bash
curl -s -X POST "$BASE_URL/tasks" \
  -H "Content-Type: application/json" \
  -d '{
    "kind": "open_page",
    "url": "https://example.com",
    "timeout_seconds": 30,
    "priority": 0
  }'
```

Expected shape:
- returns `201 Created`
- response includes `id`, `kind`, `status=queued`

Save the returned task id:

```bash
TASK_ID=<task-id>
```

---

## 2. Inspect task detail

```bash
curl -s "$BASE_URL/tasks/$TASK_ID"
```

Check for:
- canonical `status`
- `execution_identity`
- `failure_scope`
- `summary_artifacts`
- browser content summary fields when present

---

## 3. Inspect task runs

```bash
curl -s "$BASE_URL/tasks/$TASK_ID/runs"
```

Check for:
- run `status`
- run-level `execution_identity`
- run-level `failure_scope`
- run-level `summary_artifacts`

---

## 4. Inspect status snapshot

```bash
curl -s "$BASE_URL/status"
```

Check for:
- top-level `counts`
- `latest_tasks`
- `latest_browser_tasks`
- `latest_execution_summaries`
- same `execution_identity` / `failure_scope` / `summary_artifacts` semantics when the task is surfaced here

---

## 5. Cancel a running task

```bash
curl -s -X POST "$BASE_URL/tasks/$TASK_ID/cancel"
```

Expected cancel contract:
- task closes with `status=cancelled`
- cancellation reason is exposed as `failure_scope=runner_cancelled`
- runner/result vocabulary uses `error_kind=runner_cancelled`

---

## 6. Re-check detail, runs, and status

```bash
curl -s "$BASE_URL/tasks/$TASK_ID"
curl -s "$BASE_URL/tasks/$TASK_ID/runs"
curl -s "$BASE_URL/status"
```

Verify the shared contract:
- detail shows `status=cancelled`
- runs shows the cancelled run with `status=cancelled`
- status counts include the cancelled task
- status/detail/runs agree on `execution_identity`
- status/detail/runs agree on `failure_scope=runner_cancelled`
- `summary_artifacts` contain the corresponding cancellation summaries

---

## Acceptance summary

Task Contract / Control-Plane Visibility V1 is accepted when:
- create -> inspect -> cancel -> inspect works end-to-end
- `/tasks/:id`, `/tasks/:id/runs`, and `/status` do not disagree on execution identity
- `cancelled` is the visible terminal state
- `runner_cancelled` stays a reason-layer contract, not a replacement for task status
