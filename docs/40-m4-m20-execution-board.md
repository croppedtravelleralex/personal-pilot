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

## Fourth Implementation Batch

Status: started 2026-06-01.

1. Close M4.4 local provider dry-run/operator visibility by upgrading `provider_acceptance_preflight` to v3.
2. Expose CAPTCHA/SMS/Email dry-run contract, next action, failure taxonomy, latest report path, and real smoke blocker through desktop readiness API and Settings.
3. Extend M4 gate with `provider_dry_run_contract` local report validation while keeping real provider acceptance as `expected_blocked`.
4. Verify with provider preflight, M4 gate, Rust provider readiness test, `pnpm typecheck`, `cargo fmt --check`, and diff hygiene.
5. Continue next with same-run desktop/profile comparison, typed facade shrink, SessionBundle operator evidence loop, and M5 performance work.

## Fifth Implementation Batch

Status: started 2026-06-01.

1. Add a Dashboard operator action that samples desktop WebView fingerprint/canvas/audio/WebRTC/storage signals in the actual app WebView and writes them through `collect_validation_report`.
2. Upgrade `profile_browser_comparison_gate` to v3 with same-run pairing fields: `sameRunStatus`, `maxPairAgeMinutes`, and `desktopProfileDeltaMinutes`.
3. Keep comparison blocked until desktop WebView and profile-browser reports both exist within the configured window and all required categories are comparable.
4. Verify with `pnpm typecheck`, `profile_browser_comparison_gate`, M4 gate, and diff hygiene.
5. Continue next by actually collecting desktop WebView evidence from the running app, refreshing profile-browser evidence in the same window, and then moving to typed facade shrink or SessionBundle operator loop.

## Sixth Implementation Batch

Status: started 2026-06-01.

1. Close the local M4.6 SessionBundle operator loop by exposing export, preflight, dry-run, and confirmed local restore in the existing Settings page.
2. Keep confirmed restore behind an explicit local confirmation and show restore plan, missing references, conflicts, `writePerformed`, and restored binding count.
3. Extend M4 gate with `session_bundle_operator_contract` source checks for UI, desktop service wrapper, TS types, and Rust implementation/test markers.
4. Verify with `pnpm typecheck`, M4 gate, and diff hygiene.
5. Continue next with actual desktop/profile same-run evidence, typed facade shrink, runtime adapter operator loop, and M5 performance work.

## Seventh Implementation Batch

Status: started 2026-06-01.

1. Start M4.8 typed facade shrink by moving high-traffic synchronizer/workbench core bridge DTOs into `src/types/desktop.ts`.
2. Update `src/services/desktop.ts` so synchronizer list/arrange/log/tasks wrappers return shared DTOs instead of `unknown[]`.
3. Remove matching `as Promise<...>` casts from `src/modules/synchronizer/api.ts` while preserving the transitional Wails/core bridge boundary.
4. Add a typed `BrowserNativeBindings` / `getWindowGoApp()` boundary in `src/modules/browser/api.ts`, replacing browser Wails binding `any` access without changing runtime behavior.
5. Extend M4 gate with `typed_facade_shrink_contract` source checks for both synchronizer DTOs and browser Wails bindings.
6. Continue next with browser payload schema normalize, actual desktop/profile same-run evidence, and remaining workbench/core bridge shrink.

## Eighth Implementation Batch

Status: started 2026-06-01.

1. Close the local M4.7 runtime adapter operator visibility gap by reading the release smoke contract in Dashboard.
2. Rank runtime adapters by evidence strength and show adapter id, runner kind, status, profile evidence, fingerprint depth, and top blockers.
3. Extend M4 gate with `runtime_adapter_operator_contract` source checks for Dashboard UI, Dashboard API, desktop service wrapper, and shared TS types.
4. Keep full headed realism, B1-B5, remote proxy/TLS, provider, cross-machine portability, AdsPower refresh, and full `450` observed/replay coverage externally blocked until matching reports exist.
5. Continue next with actual desktop/profile same-run evidence, remaining browser facade shrink, and M5 release performance mitigation.

## Ninth Implementation Batch

Status: started 2026-06-01.

1. Close the local M4.9 safety/logging contract by adding default logger redaction for sensitive field keys and common inline secret forms.
2. Apply redaction before logger writers receive entries and before Text/JSON formatters serialize entries, so direct `LogEntry` paths do not bypass the safety boundary.
3. Extend default interceptor sensitive fields beyond password/token/secret to include API keys, authorization, credentials, cookies, client secrets, private keys, and related aliases.
4. Extend M4 gate with `safety_logging_contract` source/test checks and keep it local evidence only.
5. Keep provider credentials, historical report scrubbing, external log audit, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, and full `450` observed/replay coverage blocked until their own reports exist.

## Tenth Implementation Batch

Status: started 2026-06-01.

1. Close M4.10 by making the M4 acceptance report operator-friendly: `status` / `operatorStatus` becomes `passed_with_expected_external_blockers` when local gates pass and only expected external blockers remain.
2. Preserve `gateClassificationStatus=expected_blocked` so old live-truth semantics and blocker classification remain machine-readable.
3. Add `m4TotalGate` to the report with local contract status, passed gate ids, failed gate ids, expected blocker ids, external blocker count, and next action.
4. Update Dashboard evidence history so the new status still renders as expected external blockers, not as a failed gate.
5. Continue to M5 performance/health only after the v8 gate reports `failed=0`.
