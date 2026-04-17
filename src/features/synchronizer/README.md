# Synchronizer Feature

Owns the synchronizer matrix view, operator controls, and native capability feedback.
UI code does not invoke Tauri directly; all native access stays behind `src/services/desktop.ts`.

## Current capability boundary

- `listSyncWindows` / `readSynchronizerSnapshot` / `readSyncLayoutState`
  - Read native synchronizer snapshot and layout state.
- `setMainSyncWindow`
  - Update the synchronizer main-window anchor in native state.
- `applyWindowLayout`
  - Apply native window layout planning and surface physical placement outcome through the returned snapshot and message.
- `focusSyncWindow`
  - Use native Win32 focus control.
- `broadcastSyncAction`
  - Execute the typed native broadcast-intent contract.
  - Successful runs record native intent, source/target scope, and layout-flag state.
  - Physical multi-window event dispatch is still not implemented.

## Module split

- `store.ts`: snapshot/state orchestration, capability transitions, action feedback
- `hooks.ts`: page-friendly action wiring
- `model.ts`: types, defaults, templates, capability copy
