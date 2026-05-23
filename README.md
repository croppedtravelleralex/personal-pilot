# PersonaPilot

PersonaPilot is a Windows 11 local desktop operator console for managing browser-profile work surfaces, proxy posture, automation runs, synchronization, logs, settings, and validation evidence from one Tauri app.

This repository targets **Tauri 2 + Vite + React + TypeScript**. Electron, Node backend services, embedded Python runtimes, and multi-window embedded-browser architectures are intentionally out of scope unless explicitly approved.

## Current Status

- Mainline delivery: `100% / 0% / green`.
- Overall end-state: `30% / 70% / yellow`.
- Win11 release gate passed on 2026-05-23.
- Latest release build produces `src-tauri/target/release/bundle/nsis/PersonaPilot_0.1.0_x64-setup.exe`.
- Validation Board MVP is available in the desktop navigation and separates `declared / applied / observed` evidence.

For canonical maintenance entrypoints, read [`/docs/README.md`](docs/README.md) and [`/docs/root-entrypoint-map.md`](docs/root-entrypoint-map.md). For the canonical live truth, read [`/docs/02-current-state.md`](docs/02-current-state.md). Do not treat this root README as the maintenance source of record.

## What Is Included

- Dashboard, profiles, proxies, automation, synchronizer, logs, settings, and validation surfaces.
- Native desktop boundary centralized through `src/services/desktop.ts`.
- Provider-aware proxy rotation contract with rollback, cooldown, and retry semantics.
- Native-first recorder/template flow with fallback reserved for recovery paths.
- Synchronizer read/focus/set-main/layout support through native desktop contracts.
- Validation Board MVP covering detector, leak, DNS, WebRTC, canvas, audio, worker, and transport evidence categories.

## Important Boundaries

- Current runtime fingerprint depth is still incomplete: `80` declared controls and `26` runtime projected fields (`25` control-supported + derived `platform`).
- Validation `observed` evidence is not a full detector loop yet; native collectors and persistent reports are the next slice.
- CAPTCHA/SMS/Email handlers and routes are partially landed, but production manager wiring, provider acceptance, and operator UI closure are not complete.
- `450+` fingerprint signals and `450+` behavior taxonomy remain target-track work, not shipped runtime depth.

## Quick Start

Prerequisites:

- Windows 11
- Node.js and pnpm
- Rust toolchain compatible with Tauri 2

Install dependencies:

```powershell
pnpm install
```

Run the web dev shell:

```powershell
pnpm dev
```

Build the frontend:

```powershell
pnpm build
```

Build the Windows desktop release:

```powershell
pnpm desktop:release
```

Run the project baseline enforcement:

```powershell
powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot D:\SelfMadeTool\personal-pilot
```

## Quality Gate

Every meaningful code change should pass:

- `pnpm typecheck`
- `pnpm build`
- Win11/Tauri baseline enforcement
- `pnpm desktop:release` for release-impacting changes

The broader local verification script is:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\windows_local_verify.ps1 -SkipContinuityTest
```

## Project Layout

```text
src/
  app/
  pages/
  components/
  features/
  hooks/
  store/
  services/
  types/
  utils/
src-tauri/
  src/
  capabilities/
  tauri.conf.json
data/
scripts/
docs/
```

Native and system-capability calls must flow through:

```text
pages/components -> features/hooks/store -> src/services/desktop.ts -> tauri
```

UI code must not call Tauri directly.

## Maintenance Docs

Canonical maintenance entrypoints:

- [`/docs/README.md`](docs/README.md)
- [`/docs/02-current-state.md`](docs/02-current-state.md)
- [`/docs/03-roadmap.md`](docs/03-roadmap.md)
- [`/docs/04-improvement-backlog.md`](docs/04-improvement-backlog.md)
- [`/docs/05-ai-maintenance-playbook.md`](docs/05-ai-maintenance-playbook.md)
- [`/docs/root-entrypoint-map.md`](docs/root-entrypoint-map.md)

When status changes, update the matching canonical docs instead of relying on this root README.

## Next Work

1. Connect Validation Board `observed` layer to native collectors.
2. Persist repeatable validation reports and add profile-level evidence export.
3. Deepen fingerprint runtime coverage while keeping declared, applied, and observed evidence separate.
4. Wire CAPTCHA/SMS/Email production manager/config paths without claiming automation closure before provider acceptance.
