# Synchronizer Feature

负责本地窗口同步控制台，不直接触碰页面外的系统调用。

建议内部继续拆成：

- `store.ts`
  - 当前窗口矩阵
  - 主窗口
  - 布局模式
  - 操作状态
- `hooks.ts`
  - refresh windows
  - set main window
  - apply layout
  - focus window
- `model.ts`
  - window snapshot
  - layout mode
  - action result

二级子模块边界：

- Window Matrix
- Main Window
- Layout Controls
- Focus Actions
- Action Feedback

预期依赖的桌面契约：

- `listSyncWindows`
- `setMainSyncWindow`
- `applyWindowLayout`
- `focusSyncWindow`
