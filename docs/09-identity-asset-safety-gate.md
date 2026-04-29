# Identity Asset Safety Gate V1

Last updated: 2026-04-29

## Decision

Antbrowser manages identity assets, not just browser windows. The production mainline stays at L0:

- Real external fingerprint Chromium top-level windows.
- Tauri is only the desktop shell and management UI.
- Go sidecar remains the only owner of profile, user-data-dir, fingerprint args, proxy, debugPort, CDP, and browser launch.
- No target website in Tauri WebView.
- No HWND re-parent.
- No embedded-browser containerization, DWM mirror, or CDP screencast as the formal product path.

The product may sacrifice convenience and visual embedding to preserve fingerprint stability and profile correctness.

## Correctness Promise

The software is correct only if it can manage multiple external browser instances without changing the fingerprint chain, while protecting profile identity assets:

- Do not delete, recreate, overwrite, mix, or silently migrate user profiles.
- Keep `profileId -> canonical user-data-dir` stable.
- Preserve browser-native Cookie, localStorage, IndexedDB, session, cache, and login state through the same profile directory.
- Prefer failed operations over starting or controlling the wrong profile, path, arguments, proxy, port, or process.

Hard rule:

```text
If identity ownership is uncertain, stop. Do not guess, repair, migrate, or substitute.
```

## Priority Order

When product goals conflict, use this priority:

```text
state correctness > API operation correctness > window/visual convenience
```

- State correctness: profile identity, fingerprint args, proxy, user-data-dir, process ownership, Cookie safety.
- API operation correctness: navigation, refresh, screenshot, task queue, behavior playback.
- Window convenience: activate, arrange, preview, minimize, visual switching.

Window activation or arrangement failure must not corrupt profile state or block safe CDP operations.

## Fail-Closed Boundaries

### Allowed To Continue Automatically

These cases may continue without user confirmation:

- The profile path exists and canonicalizes to the bound user-data-dir.
- Cookie/localStorage/cache changes come from normal browser runtime writes.
- debugPort is not ready yet, but retry keeps the same profile, user-data-dir, fingerprint args, proxy, and launch intent.
- Window activation or arrangement fails, but process ownership and CDP ownership remain valid.
- UI or sidecar restarts and can rediscover an instance that is provably launched by this software and bound to the same profile.

### Requires Explicit Confirmation

These cases require a preflight summary and user confirmation:

- User-triggered backup import or restore that writes Cookie/profile data.
- User-triggered change of profile user-data-dir.
- Old-version migration that changes profile path mappings.
- Existing directory has no expected marker, but the user claims it is an old profile directory.
- Import/restore requires stopping a running instance first.
- Any destructive operation that overwrites, deletes, resets, or rewrites profile data.

### Must Fail Closed

These cases must stop the operation:

- The same canonical user-data-dir is referenced by multiple profiles.
- The same profile already has a running instance and another start is requested.
- CDP port is reachable but PID, executable path, profileId, user-data-dir, or launch ownership cannot be proven.
- fingerprint args, proxy, user-data-dir, profileId, or launch args hash differs from the expected binding.
- Canonical path escapes the allowed root or passes through unresolved symlink/junction ambiguity.
- Any automatic flow attempts deletion, rebuild, migration, overwrite, or cache clearing.
- Backup/import/restore target is ambiguous or could affect multiple profiles.
- Crash recovery cannot prove that an old process belongs to the current profile.
- Startup failure fallback wants to switch profile, user-data-dir, proxy, fingerprint args, or browser args.
- Behavior playback failure wants to substitute another profile or another group member.

## Profile Identity Rules

1. `profileId -> canonical user-data-dir` is a long-lived binding.
2. The canonical user-data-dir must be unique across profiles.
3. A profile can have at most one running browser instance.
4. The final browser command must be assembled only by Go sidecar.
5. Tauri and React may send typed intent, but must not assemble raw browser launch arguments.
6. Every launch should record an audit snapshot:
   - browser executable path
   - profileId
   - canonical user-data-dir
   - proxy binding/hash
   - debugPort
   - fingerprint args hash
   - launch args hash
   - PID/process tree evidence
   - timestamp and app mode

## Cookie Policy

Cookie handling uses policy `1 + 2`:

- Asset operations support Cookie by default: backup, export, import, restore, and verification.
- Startup may run read-only Cookie verification.
- Normal launch, browsing, API operation, navigation, screenshot, refresh, and behavior playback must not write Cookie.
- Automatic Cookie repair is forbidden.
- Cookie writes are allowed only when the user explicitly triggers import or restore and confirms the target.

Cookie verification failures are graded:

- File lock, temporary read failure, or browser currently writing Cookie DB: warn only.
- Cookie DB missing, unreadable, damaged, or cannot be decrypted: warn and recommend backup/restore review, but do not auto-repair.
- Profile ownership mismatch, path mismatch, backup signature mismatch, or cross-profile Cookie import risk: block startup/import/restore.

## Backup, Import, And Restore

Backup is read-only by default.

Import and restore must run preflight before writing. The preflight must display:

- target profileId
- target canonical user-data-dir
- whether an instance must be stopped
- what will be added
- what will be overwritten
- what will be skipped
- whether Cookie will be written
- exact directories/files that may be deleted, reset, or replaced

Running profiles cannot be imported/restored by default. If import/restore requires a stop, the user must explicitly confirm stopping that profile first.

Any operation that calls `RemoveAll` or equivalent destructive behavior must be guarded by preflight, ownership proof, and explicit confirmation.

## Automation And API Boundaries

- API and automation operate only on profiles explicitly selected by the user or bound by the task.
- Tasks must not automatically expand to unselected profiles.
- Failure retry must not change profile, user-data-dir, proxy, fingerprint args, or launch args.
- Behavior playback failure stops the current task.
- A failed instance in a batch must not be replaced by another same-group instance.
- Batch results must show per-profile success/failure instead of hiding partial failure.

## Fingerprint Acceptance

Fingerprint verification should align with AdsPower-level external fields, while keeping Antbrowser's safety policy stricter.

Stable fields must match across baseline/candidate, cold/hot starts, and repeated restarts for the same profile:

- canonical user-data-dir
- launch args hash
- fingerprint args hash
- proxy binding/hash
- Cookie persistence marker
- UA
- WebRTC policy
- Canvas
- Audio
- WebGL vendor/renderer
- Fonts
- timezone
- language
- screen/resolution
- hardwareConcurrency
- deviceMemory

Allowed differences:

- PID
- debugPort if intentionally dynamic and still correctly bound
- timestamps
- screenshot byte length
- random noise fields that are intentionally unstable
- local parent-process chain, recorded as non-website-JS-visible environment difference

Acceptance cannot be based on visual inspection. It must be scriptable diff evidence.

## V1 Implementation Slices

### Slice 1: Canonical Path Fence

- Canonicalize profile user-data-dir consistently.
- Reject duplicate canonical user-data-dir across profiles.
- Reject unresolved symlink/junction ambiguity when ownership cannot be proven.
- Add tests for absolute path, relative path, case differences, path traversal, duplicate path, and missing path.

### Slice 2: Profile Start Lock

- Prevent two starts for the same profile.
- Persist enough runtime ownership evidence to avoid reattaching to the wrong old process.
- Fail closed if a running instance exists but ownership cannot be proven.

### Slice 3: CDP Ownership Verification

- Do not trust debugPort alone.
- Verify PID, executable path, profileId/user-data-dir binding, and launch audit snapshot before CDP control.
- Fail closed on mismatch.

### Slice 4: Cookie Asset Verification

- Add read-only Cookie verification at startup.
- Add Cookie to backup/export/import/restore checks.
- Separate warnings from blockers using the Cookie policy above.

### Slice 5: Destructive Operation Preflight

- Add preflight for import, restore, reset, snapshot restore, and any profile-directory write/delete operation.
- Require explicit confirmation for confirmed destructive writes.
- Add tests ensuring automatic flows cannot delete or overwrite profile directories.

### Slice 6: AdsPower-Aligned Fingerprint Diff

- Extend fingerprint regression script to cover the stable fields listed above.
- Add Cookie persistence marker check.
- Keep parent-process differences as recorded environment metadata, not website-visible fingerprint failure.

### Slice 7: Release Acceptance Suite

- Run against Tauri release exe and sidecar.
- Start two temporary profiles.
- Verify profile isolation, Cookie persistence, CDP operations, screenshot, stop-one-keep-one, clean shutdown, and no residual processes.
- Verify duplicate user-data-dir and path mismatch are blocked.

## Completion Criteria

V1 is complete only when:

- All slices above have tests or scripted acceptance.
- `go test ./backend/...` passes.
- `npm run build` passes.
- `npm run tauri:build` passes.
- Win11 Tauri baseline enforcement passes.
- `git diff --check` passes.
- Release acceptance suite proves:
  - no profile mix
  - no user-data-dir drift
  - Cookie persistence survives restart
  - destructive operations require confirmation
  - CDP cannot attach to an unproven instance
  - fingerprint stable fields match expected baseline

