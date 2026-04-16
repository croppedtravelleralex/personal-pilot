# Synchronizer Feature

负责同步控制台的矩阵视图、操作编排和反馈状态。页面侧不直接调用原生能力，统一通过 `services/desktop.ts`。

## Current capability boundary

- `listSyncWindows` / `readSynchronizerSnapshot` / `readSyncLayoutState`
  - 读取 native synchronizer snapshot 和布局状态。
- `setMainSyncWindow`
  - 写入 synchronizer 内部主窗口锚点状态，不包含物理窗口重排。
- `applyWindowLayout`
  - 写入 synchronizer 内部布局状态，不包含物理桌面窗口重排。
- `focusSyncWindow`
  - 走 native Win32 焦点控制。
- `broadcastSyncAction`
  - 能力门控执行：有契约时记录 broadcast intent 与布局标志并回写快照；无契约时保留 prepared/fallback。
  - 当前不包含物理多窗口事件分发。

## Module split

- `store.ts`: snapshot/state orchestration, capability transitions, action feedback
- `hooks.ts`: page-friendly action wiring
- `model.ts`: types, defaults, templates, capability copy
