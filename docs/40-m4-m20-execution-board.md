# M4-M20 Execution Board

Updated: 2026-06-12 (Asia/Shanghai)

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
| M8 | Cross-machine SessionBundle | Handoff package gate plus second Win11 target preflight, dry-run, confirmed restore, restart continuity |
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

## Eleventh Implementation Batch

Status: started 2026-06-01.

1. Start M5 by upgrading `scripts/release_performance_smoke.ps1` to v2 with `budgetStatus`, per-metric `budgetResults`, 10% drift targets, drift reason enforcement, `healthSummary`, `mitigationHints`, and process breakdown hints.
2. Add `scripts/m5_release_health_gate.ps1` so M5 validates the latest v2 release health report instead of relying on a raw smoke file.
3. Update desktop release contract and Dashboard evidence history to surface M5 Health, release health status, budget results, and mitigation hints.
4. Keep `passed_with_budget_overrun` as a valid M5 report-contract result while explicitly forbidding performance green until cold start/RSS/process budgets pass.
5. Continue next with actual metric mitigation, M6 same-run desktop/profile evidence, M13 report diff/risk trends, or M10 headed repeatability without claiming provider, proxy/TLS, cross-machine, AdsPower, or full `450` coverage.

## Twelfth Implementation Batch

Status: started 2026-06-02.

1. Start M13 evidence operations by adding latest-vs-previous trend fields to local evidence report history.
2. Compute trend only within the same report kind: previous status, previous failure reason, previous generated time, status trend, failure-reason trend, and a short trend summary.
3. Show trend text in Dashboard evidence rows without changing report status, evidence level, gate classification, or external blocker semantics.
4. Verify with `cargo test evidence_report_history`, `pnpm typecheck`, and diff hygiene.
5. Continue next with richer report diff views, M6 same-run desktop/profile evidence, M10 headed repeatability, or M15 browser pool cleanup.

## Thirteenth Implementation Batch

Status: started 2026-06-02.

1. Extend M13 evidence operations from status/failure trend to risk trend.
2. Derive `riskLevel` and `riskScore` from existing `status` and `evidenceLevel`; keep these fields read-only and report-history scoped.
3. Attach previous risk level/score and `riskTrend` for same-kind reports, using `reportPath` as a same-timestamp tie-breaker.
4. Show risk trend in Dashboard evidence rows without changing status colors, gate classification, or external blocker semantics.
5. Verify with `cargo test evidence_report_history`, `pnpm typecheck`, `live_truth_guard`, M4 gate, and diff hygiene.

## Fourteenth Implementation Batch

Status: started 2026-06-02.

1. Improve M6 profile-browser comparison operator readability without claiming same-run evidence is complete.
2. Parse `profile_browser_comparison` report fields into evidence history summary: same-run status, desktop/profile signal counts, comparable category count, pair delta/max window, and missing category statuses.
3. Generate status-specific next action for missing desktop WebView report, missing profile-browser report, stale pair, and partial category comparison.
4. Preserve the comparison report `status`, `failureReason`, `evidenceLevel`, M4 expected external blocker semantics, and Overall `40% / 60% / yellow`.
5. Verify with `cargo test evidence_report_history`, `pnpm typecheck`, `live_truth_guard`, M4 gate, and diff hygiene.

## Fifteenth Implementation Batch

Status: started 2026-06-02.

1. Extend M13 evidence operations with `failureReasonCategory`.
2. Categorize common local report blockers: release budget overrun, provider credentials missing, remote proxy missing, desktop/profile comparison gaps, cross-machine SessionBundle pending, runtime adapter evidence required, external operator smoke required, and expected external blockers.
3. Surface the category in Dashboard evidence rows while preserving original `failureReason`, `status`, `evidenceLevel`, and gate semantics.
4. Verify with evidence report history tests, `pnpm typecheck`, live truth guard, M4 gate, and diff hygiene.

## Sixteenth Implementation Batch

Status: started 2026-06-02.

1. Extend M13 evidence operations from category labels to structured report diff.
2. Add latest-vs-previous diff fields for same-kind reports: status, failureReason, failureReasonCategory, and risk.
3. Surface compact diff items in Dashboard evidence rows while preserving original report status, evidence level, expected blocker classification, and live-truth boundaries.
4. Keep `failureReasonCategory` derived only from kind/status/failureReason; do not read full report JSON for category classification.
5. Verify with evidence report history tests, `pnpm typecheck`, live truth guard, M4 gate, and diff hygiene.

## Seventeenth Implementation Batch

Status: started 2026-06-02.

1. Advance M10 by running headed_external validation probe repeatability with `-RepeatValidationProbeCount 2` against the real local Chromium binary.
2. Add `scripts/m10_headed_repeatability_gate.ps1` so the latest ranked headed_external repeatability report becomes a machine-readable M10 gate.
3. Surface `m10_headed_repeatability` in desktop evidence history and Dashboard without changing headed_external, runtime_adapter, B1-B5, or AdsPower semantics.
4. Keep `passed_repeatability_partial_coherence` scoped to multi-run validation_probe signal/category/status shape stability only.
5. Continue next with long-task stability, full headed coherence matrix, M6 same-run desktop/profile evidence, remote proxy/TLS, provider credentials, cross-machine SessionBundle, and full `450` observed/replay coverage as separate gates.

## Eighteenth Implementation Batch

Status: started 2026-06-02.

1. Advance M15 by turning the existing pool prewarm contract into a local lifecycle harness with usage accounting, budget status, release cleanup proof, expired lease cleanup, stale idle reclaim, and failed slot reclaim.
2. Add `scripts/m15_browser_pool_gate.ps1` so the local harness emits `data/reports/m15-browser-pool/*` with `passed_pool_lifecycle_harness` only when Go tests and source contract checks pass.
3. Surface `m15_browser_pool` in desktop evidence history and Dashboard as `M15 Pool` without changing runtime adapter, headed external, M4, M5, provider, SessionBundle, or AdsPower semantics.
4. Keep this evidence scoped to in-memory lifecycle behavior only; it does not prove real browser process prewarm, CDP readiness, RSS/process budgets, proxy/TLS behavior, provider closure, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.
5. Continue next with real browser process pool integration, process/RSS cleanup proof, timeout/cancel cleanup, M10 long-task stability, M6 same-run evidence, and M8 second-machine SessionBundle proof as separate gates.

## Nineteenth Implementation Batch

Status: started 2026-06-02.

1. Advance M8 by adding `scripts/m8_session_handoff_gate.ps1`, which refreshes local SessionBundle portability smoke, validates the refresh result, and checks the runbook, Settings operator surface, desktop command/tests, typed TS wrapper/types, and M4 boundary guard.
2. Surface `m8_session_handoff` in desktop evidence history and Dashboard as `M8 Handoff` with `session_handoff_package_partial` evidence while `passed_handoff_package_ready` remains a cross-machine pending category.
3. Keep this evidence scoped to handoff package readiness only; it does not prove second-machine portability, provider credentials, remote proxy/TLS, AdsPower refresh, full headed realism, or full `450` observed/replay coverage.

## Twentieth Implementation Batch

Status: started 2026-06-02.

1. Advance M4.8 by normalizing browser runtime event payloads behind `BrowserRuntimeEventPayload` and `normalizeBrowserRuntimeEventPayload(payload: unknown)`.
2. Replace Browser List/Detail runtime lifecycle event handlers that used `payload: any` with shared normalizer calls, and narrow selected browser API normalizer inputs from `any` to `unknown`.
3. Add `scripts/m4_browser_payload_schema_gate.ps1`, desktop evidence history support, and Dashboard `M4 Payload` row for `m4_browser_payload_schema` reports.
4. Extend M4 acceptance gate to v9 with `browser_payload_schema_contract`; latest gate remains `passed_with_expected_external_blockers` with `passed=8`, `expectedBlocked=4`, `failed=0`.
5. Keep this evidence scoped to browser module source-level payload schema normalization only; it does not prove `tauriWailsBridge` removal, low-frequency workbench/core bridge closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.

## Twenty-First Implementation Batch

Status: started 2026-06-02.

1. Advance M4.8 by routing Dashboard stats/license/config/CD key calls through `src/services/desktop.ts` typed wrappers instead of direct dynamic Wails imports.
2. Remove `const bindings: any` and `../../wailsjs/go/main/App` import usage from `src/modules/dashboard/api.ts` while preserving stats fallback, evidence history, release smoke contract, WebView evidence collection, reload config, and CD key behavior.
3. Add `scripts/m4_dashboard_facade_gate.ps1`, desktop evidence history support, and Dashboard `M4 Dashboard` row for `m4_dashboard_facade` reports.
4. Extend M4 acceptance gate to v10 with `dashboard_facade_contract`; latest gate remains `passed_with_expected_external_blockers` with `passed=9`, `expectedBlocked=4`, `failed=0`.
5. Keep this evidence scoped to Dashboard API source-level typed facade usage only; it does not prove `tauriWailsBridge` removal, profile/settings/logs facade closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.

## Twenty-Second Implementation Batch

Status: started 2026-06-02.

1. Advance M4.8 by routing Settings backup initialize/export/import and Browser logs read/clear through `src/services/desktop.ts` typed wrappers instead of direct dynamic Wails imports.
2. Preserve destructive backup preflight, native file dialog selection, confirmation prompts, and the Wails-compatible `tauriWailsBridge` proxy while removing raw module-level dynamic imports from `src/modules/settings/api.ts` and `src/modules/browser/pages/BrowserLogsPage.tsx`.
3. Add `scripts/m4_settings_logs_facade_gate.ps1`, desktop evidence history support, and Dashboard `M4 Settings/Logs` row for `m4_settings_logs_facade` reports.
4. Extend M4 acceptance gate to v11 with `settings_logs_facade_contract`; latest gate remains `passed_with_expected_external_blockers` with `passed=10`, `expectedBlocked=4`, `failed=0`.
5. Keep this evidence scoped to Settings backup API and Browser logs source-level typed facade usage only; it does not prove `tauriWailsBridge` removal, profile/workbench/core facade closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.

## Twenty-Third Implementation Batch

Status: started 2026-06-02.

1. Advance M4.8 by routing Profile remote author loading through `src/services/desktop.ts` typed wrapper `fetchRemoteAuthorProfileFromDesktop` instead of direct dynamic Wails imports.
2. Preserve the browser `fetch` fallback for non-desktop previews while removing `const bindings: any`, direct `../../wailsjs/go/main/App` dynamic import, and `(window as any).go?.main?.App` from `src/modules/profile/api.ts`.
3. Add `scripts/m4_profile_facade_gate.ps1`, desktop evidence history support, and Dashboard `M4 Profile` row for `m4_profile_facade` reports.
4. Extend M4 acceptance gate to v12 with `profile_facade_contract`; latest gate remains `passed_with_expected_external_blockers` with `passed=11`, `expectedBlocked=4`, `failed=0`.
5. Keep this evidence scoped to Profile remote author source-level typed facade usage only; it does not prove `tauriWailsBridge` removal, workbench/core facade closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.

## Twenty-Fourth Implementation Batch

Status: started 2026-06-02.

1. Advance M4.8 by routing FingerprintPanel behavior preset loading through `src/modules/browser/api.ts` `fetchBehaviorPresets` instead of direct Wails `BehaviorPresetList` imports.
2. Preserve the existing browser API facade and fallback behavior while removing component-level raw Wails access from `src/modules/browser/components/FingerprintPanel.tsx`.
3. Add `scripts/m4_behavior_preset_facade_gate.ps1`, desktop evidence history support, and Dashboard `M4 Behavior` row for `m4_behavior_preset_facade` reports.
4. Extend M4 acceptance gate to v13 with `behavior_preset_facade_contract`; latest gate remains `passed_with_expected_external_blockers` with `passed=12`, `expectedBlocked=4`, `failed=0`.
5. Keep this evidence scoped to FingerprintPanel behavior preset source-level facade usage only; it does not prove `tauriWailsBridge` removal, browser/workbench/core facade closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.

## Twenty-Fifth Implementation Batch

Status: started 2026-06-02.

1. Advance M4.8 by routing AutomationPage scheduler/rule calls through `src/modules/browser/api.ts` module facade instead of direct Wails `Scheduler*` / `AutomationRule*` imports.
2. Replace page-level `backend` model constructors with plain typed `SchedulerTaskInput` / `AutomationRuleInput` DTOs while preserving task template creation, delete, run-now, rule creation, toggle, delete, and test-fire behavior.
3. Add `scripts/m4_automation_facade_gate.ps1`, desktop evidence history support, and Dashboard `M4 Automation` row for `m4_automation_facade` reports.
4. Extend M4 acceptance gate to v14 with `automation_facade_contract`; latest gate remains `passed_with_expected_external_blockers` with `passed=13`, `expectedBlocked=4`, `failed=0`.
5. Keep this evidence scoped to AutomationPage scheduler/rule source-level facade usage only; it does not prove `tauriWailsBridge` removal, full browser/app shell/monitor/workbench/core facade closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.

## Twenty-Sixth Implementation Batch

Status: started 2026-06-11.

1. Advance M4.8 by routing `src/App.tsx` app shell close confirmation, notification subscriptions, environment lookup, tray/minimize, and quit actions through `src/services/desktop.ts` typed wrappers.
2. Remove App shell direct imports from `./wailsjs/go/main/App` and `./wailsjs/runtime/runtime`, and stop reading `(window as any).runtime` from the app root.
3. Add `scripts/m4_app_shell_facade_gate.ps1`, desktop evidence history support, and Dashboard `M4 App Shell` row for `m4_app_shell_facade` reports.
4. Extend M4 acceptance gate to v15 with `app_shell_facade_contract`; the gate must remain `passed_with_expected_external_blockers` with local failed gates at `0`.
5. Keep this evidence scoped to App shell source-level facade usage only; it does not prove `tauriWailsBridge` removal, full monitor/browser/workbench/core facade closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.
6. Continue next with EventMonitor/runtime event facade shrink or browser/workbench/core low-frequency bridge payloads.

## Twenty-Seventh Implementation Batch

Status: started 2026-06-11.

1. Advance M4.8 by routing `src/modules/monitor/EventMonitorPage.tsx` live runtime subscriptions and event-log history query/count/export/prune through `src/services/desktop.ts` typed wrappers.
2. Remove page-level direct Wails imports from `../../wailsjs/go/main/App` and `../../wailsjs/go/models`, stop reading `(window as any).runtime`, and keep monitor event payloads/error handling on `unknown` guards.
3. Fix the live monitor pause buffer closure so the one-time runtime subscription reads the latest `paused` state without resubscribing every event.
4. Add `scripts/m4_monitor_facade_gate.ps1`, desktop evidence history support, and Dashboard `M4 Monitor` row for `m4_monitor_facade` reports.
5. Extend M4 acceptance gate to v16 with `monitor_facade_contract`; the gate must remain `passed_with_expected_external_blockers` with local failed gates at `0`.
6. Keep this evidence scoped to EventMonitor source-level facade usage only; it does not prove `tauriWailsBridge` removal, full browser/workbench/core facade closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.
7. Continue next with browser runtime/workbench/core low-frequency bridge payloads or a higher-value external evidence blocker if a real target environment is available.

## Twenty-Eighth Implementation Batch

Status: started 2026-06-12.

1. Advance M4.8 by routing Browser List/Detail `browser:instance:*` runtime subscriptions through `src/modules/browser/api.ts` instead of direct Wails `EventsOn` imports.
2. Add `BrowserInstanceRuntimeEventName` / `BrowserInstanceRuntimeEvent` and expose `onBrowserInstanceRuntimeEvents`, with payload normalization kept inside the browser module API facade.
3. Add `scripts/m4_browser_runtime_facade_gate.ps1`, desktop evidence history support, and Dashboard `M4 Browser Runtime` row for `m4_browser_runtime_facade` reports.
4. Extend M4 acceptance gate to v17 with `browser_runtime_facade_contract`; the gate must remain `passed_with_expected_external_blockers` with local failed gates at `0`.
5. Keep this evidence scoped to Browser List/Detail runtime subscription source-level facade usage only; it does not prove `tauriWailsBridge` removal, settings/core/proxy/workbench bridge closure, real headed runtime, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full `450` coverage.
6. Continue next with settings/core/proxy/workbench bridge shrink, M6 same-run desktop/profile evidence, M10 long-task stability, M15 real process pool proof, or M8 second-machine SessionBundle proof.
