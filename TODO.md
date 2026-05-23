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
- [ ] Extend Validation Board observed collectors to WebRTC/canvas/audio/leak
- [ ] Deepen fingerprint runtime from `80` declared controls and `12` projected fields toward broader applied / observed coverage
- [ ] Formalize `SessionBundle` and profile portability around the already-landed restart continuity assets
- [ ] Grow the behavior layer from `13` shipped primitives toward a replayable `450+` event taxonomy
- [ ] Deepen headed realism, kernel strategy, and AdsPower-boundary catch-up without breaking the Win11 / Tauri baseline
- [ ] Land the highest-ROI parts of the external browser integration plan into maintainable main-repo assets

## Notes

- [ ] Cookie / localStorage / sessionStorage persistence across app restart is already landed; future work is validation, operatorization, and broader session-bundle tooling
- [ ] `50+` should be read as the minimum control-plane threshold; the current first-family schema already declares `80` core controls
