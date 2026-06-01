# M4-M20 Execution Board

Updated: 2026-06-01 (Asia/Shanghai)

## Live Truth Boundary

- Mainline delivery remains `100% / 0% / green`.
- Overall end-state remains `40% / 60% / yellow` until new real evidence changes it.
- M4-M20 work must improve user-visible capability and evidence depth without claiming external proof that has not been recorded.
- Provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, and full `450` observed/replay coverage stay blocked or deferred until matching reports exist.

## Branch And Delivery Rule

- Current implementation branch: `codex/m4-m20-full-implementation`.
- Keep unrelated pre-existing dirty files intact; do not revert them.
- Commit by evidence slice, not by aspiration:
  - `m4-harness`
  - `m4-workflow-runtime`
  - `m4-provider-dryrun`
  - `m5-performance`
  - `m6-observed-fingerprint`
  - later milestone slices only after fresh gates pass.

## M4 Definition

M4 is the first practical utility milestone after the current contract-heavy state. It turns workflow, automation, evidence, provider readiness, SessionBundle, and runtime adapter reports into an operator-usable loop inside the existing `personal-pilot-tauri.exe` path.

### Required M4 Outcomes

| ID | Outcome | Local acceptance | External boundary |
| --- | --- | --- | --- |
| M4.1 | M4 acceptance harness | `scripts/m4_acceptance_gate.ps1` emits JSON under `data/reports/m4-acceptance` | Expected blockers remain explicit |
| M4.2 | Workflow task center | User can create/run/inspect workflow task status through existing UI/API path | No real provider claim without credentials |
| M4.3 | Automation primitive v1 | Wait/select/dialog/download/upload/iframe/tab support has targeted tests or explicit gap report | No target-site production claim |
| M4.4 | Provider dry-run closure | CAPTCHA/SMS/Email dry-run path and failure taxonomy are visible | Real provider smoke remains blocked |
| M4.5 | Evidence console | Latest reports show passed/warning/blocked with path and failure reason | Blocked is not failure when expected |
| M4.6 | SessionBundle operator loop | Export/import/preflight/dry-run/confirmed local restore remains reproducible | Cross-machine remains blocked |
| M4.7 | Runtime adapter operator loop | Camoufox/headed_external probes are visible and ranked by evidence strength | Full headed realism remains pending |
| M4.8 | Typed facade shrink | Synchronizer/browser high-traffic APIs move away from dynamic RPC where practical | Wails bridge remains transitional |
| M4.9 | Safety and logging | Config/load errors and credential redaction rules are explicit | No credential values in reports/logs |
| M4.10 | M4 total gate | One command summarizes the M4 state | Status may be `passed_with_expected_external_blockers` |

## M5-M20 Roadmap

| Milestone | Theme | Required shipped evidence |
| --- | --- | --- |
| M5 | Release performance and health | Fresh release smoke, detailed health report, startup/RSS/process mitigation notes |
| M6 | Observed fingerprint comparison | Same-run desktop WebView vs profile-browser comparison and repeatability sampling |
| M7 | Provider production closure | Real credential-backed CAPTCHA/SMS/Email smoke, CDP detect/fill, failure handling |
| M8 | Cross-machine SessionBundle | Second Win11 target preflight, dry-run, confirmed restore, restart continuity |
| M9 | Remote proxy/TLS reality | Remote proxy egress/TLS report, direct baseline, DNS/WebRTC leak relation |
| M10 | Runtime realism | Headed repeatability/coherence matrix, long-task stability, Camoufox task metrics |
| M11 | Workflow marketplace | Versioned built-in templates and import/export for auth/register/form/provider flows |
| M12 | Live behavior replay | `450` behavior taxonomy connected to replay runtime and deterministic audit reports |
| M13 | Evidence operations | Report history, diffs, risk trends, failure reason taxonomy, governance guardrails |
| M14 | Data export | CSV plus optional Notion/Sheets/Airtable adapters with redaction controls |
| M15 | Browser pool | Real prewarm/acquire/release with resource budgets and cleanup proof |
| M16 | Trust inheritance | Credential chain, encrypted token store, SessionBundle migration integration |
| M17 | Device family consistency | Business-laptop and desktop-family generators tied to consistency scoring |
| M18 | AdsPower score refresh | B1-B5 evidence-backed rescore only; no score lift from contract-only work |
| M19 | External distribution | Clean Win11 install/start/uninstall and full-page operator smoke |
| M20 | Product hardening | Audit, permissions, rollback, backup, release notes, maintenance handoff |

## Extreme Acceptance And Drift Rules

| Dimension | Hard target | 10% drift rule |
| --- | --- | --- |
| Startup | cold start `<= 2.0s` | `<= 2.2s` only if reason recorded |
| Memory | idle RSS `<= 220MB` | `<= 242MB` only if reason recorded |
| Process count | `<= 4` | `<= 5` only if sidecar exception recorded |
| Workflow mock E2E | `>= 95%` pass | `>= 90%` only for timing/network variance |
| Evidence schema | required paths/status/failureReason present | no drift allowed |
| Safety | no secrets in logs/reports | no drift allowed |
| Live truth | no false `Overall` uplift | no drift allowed |
| Provider | no fake `accepted` without credentials and smoke | no drift allowed |
| Portability | no cross-machine claim without second machine report | no drift allowed |
| Runtime | no full realism claim from single probe | no drift allowed |
| Docs | current-state/roadmap/backlog/logs synchronized | no drift allowed |

## First Implementation Batch

1. Land M4 acceptance harness and machine-readable report.
2. Fill low-risk automation primitive tests and explicit unsupported-action errors.
3. Wire M4 report visibility through existing evidence report history where feasible.
4. Run `live_truth_guard`, M4 harness, targeted Go/Rust tests, and `pnpm typecheck`.
5. Commit only the files touched by this batch after reviewing dirty worktree origin.

## Second Implementation Batch

Status: started 2026-06-01.

1. Close M4.2 local workflow task center by making scheduler task runtime state durable.
2. Show task run status, last run, failure reason, retry count, and step count in the existing Automation page.
3. Preserve the boundary that this is local scheduler/operator-loop evidence only.
4. Verify with scheduler/database Go tests and `pnpm typecheck`.
5. Continue next with same-run desktop/profile comparison, provider dry-run/operator closure, and typed facade shrink.

## Third Implementation Batch

Status: started 2026-06-01.

1. Close the local M4.3 primitive gap by adding typed scheduler actions for `select`, `dialog`, `download`, `upload`, `iframe`, and `tab`.
2. Keep raw `cdp` available for escape hatches, but make common operator actions first-class runner behavior.
3. Verify with mock CDP runner tests; this is not a target-site production claim.
4. Preserve external blockers for provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, and full `450` observed/replay coverage.
5. Continue next with provider dry-run/operator closure, same-run desktop/profile comparison, and typed facade shrink.
