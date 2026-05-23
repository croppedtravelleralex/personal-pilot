# TODO.md

This root `TODO.md` stays as a thin execution checklist.
Canonical stage stack, task volume, and agent plan live in `/docs/19-phase-plan-and-scorecard.md`.

## Closed In This Round

- [x] Merge `Tasks` into `Automation` and close the orphan route gap
- [x] Restore the full Rust gate to green
- [x] Promote `scripts/windows_local_verify.ps1` into the primary Win11 acceptance entry
- [x] Push `changeProxyIp` to a provider-aware / sticky-aware local closure
- [x] Push `Synchronizer` to live snapshot / native focus with honest staged-only unsupported writes
- [x] Push `Recorder` to desktop step-write
- [x] Fix the remaining `cargo test --quiet` integration failures and stabilize the full Rust gate
- [x] Clear the Vite chunk warning with route-level code splitting

## Completed In This Round (Multi-Agent Closeout)

- [x] Finish provider-grade proxy API write behind the now-stable local `changeProxyIp` contract
- [x] Finish `Synchronizer` native batch / broadcast writes and move more staged paths out of the default route
- [x] Finish `Recorder / Templates` native-first de-fallback closure
- [x] Fix hardcoded SQLite path with env var fallback
- [x] Delete stale `package-lock.json` (pnpm project)
- [x] Add `.env` to `.gitignore`
- [x] Add CI workflow (`cargo test` + `cargo clippy` + `pnpm typecheck`)
- [x] Fix review findings (DB error handling, div-by-zero, HWND validation, non-deterministic ordering)

## Completed In Current Follow-up

- [x] Add backend handler/route and solver/provider code boundary for CAPTCHA solving (2Captcha/Capsolver); production manager wiring/config remains open
- [x] Add backend handler/route and provider code boundary for SMS verification (5sim/SMSPool); production manager wiring/config remains open
- [x] Land EmailService/API boundary for temporary inbox creation, lookup, release, and wait-code; session persistence is currently best-effort
- [x] Add provider production readiness contract for CAPTCHA/SMS/Email blockers
- [x] Split backup implementation away from the old `app_backup_ops.go` monolith
- [x] Move browser launch args and process monitor into `backend/internal/browser/`

## Mainline Release Evidence

- [x] Run Win11 local release gate without reopening scope (`scripts/windows_local_verify.ps1 -SkipContinuityTest`)
- [ ] Optional: run manual operator smoke before external distribution

## Overall End-State 70%

- [x] Build Validation Board MVP for detector, leak, DNS, WebRTC, canvas, audio, worker, and transport evidence
- [x] Keep Validation Board reports split into declared / applied / observed
- [x] Connect first Validation Board observed collectors for DNS/transport and write repeatable local JSON reports
- [x] Add Validation report history list and profile evidence export
- [x] Add WebRTC/leak observed contract warning signals to validation reports
- [x] Add desktop WebView scoped WebRTC/canvas/audio/storage observed probes without claiming profile browser coverage
- [x] Add profile browser runtime / CDP scoped `validation_probe` runner action and merge its signals into validation reports
- [x] Run real Lightpanda/CDP operator smoke to confirm WebRTC/leak/canvas/audio profile runtime reports are repeatable outside FakeRunner
  - [x] Add repeatable local smoke entry: `scripts/validation_lightpanda_smoke.ps1`
  - [x] Install/use official Lightpanda nightly through WSL2 and run `validation_lightpanda_smoke --use-wsl-lightpanda` with `status=passed`
- [x] Converge validation evidence schema with explicit scope / adapter / target-profile / failure fields
- [x] Deepen fingerprint runtime projection from `12` to `26` env-backed fields while preserving declared / applied / observed separation
- [x] Deepen fingerprint observed coverage beyond projected/applied fields
  - [x] Add Validation Board fingerprint observation audit from real WebRTC/canvas/audio/leak observed signals
  - [x] Keep `80` declared controls, `26` runtime projected fields, and `450+` target-only signals out of observed proof counts
- [x] Formalize `SessionBundle` and profile portability around the already-landed restart continuity assets
  - [x] Add confirmed restore write path after preflight
  - [x] Preserve dry-run as non-writing portability check
  - [x] Restore target profile and proxy session bindings from included local bundle payloads
  - [x] Add profile-scoped export contract with redacted-by-default session evidence
  - [x] Add import preflight and restore contract
  - [ ] Verify profile portability end to end with restart continuity
- [ ] Grow the behavior layer from `13` shipped primitives toward a replayable `450+` event taxonomy
- [x] Add runtime adapter / release smoke contract without hosting Chromium/Firefox forks
- [ ] Deepen headed realism, kernel strategy, and AdsPower-boundary catch-up without breaking the Win11 / Tauri baseline
- [ ] Land the highest-ROI parts of the external browser integration plan into maintainable main-repo assets

## Notes

- [ ] Cookie / localStorage / sessionStorage persistence across app restart is already landed; future work is validation, operatorization, and broader session-bundle tooling
- [ ] `50+` should be read as the minimum control-plane threshold; the current first-family schema already declares `80` core controls
