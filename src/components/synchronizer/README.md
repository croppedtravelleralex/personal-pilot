# Synchronizer Components

这里放 `Synchronizer` 专用展示组件。

建议拆成：

- `WindowMatrix.tsx`
- `WindowCard.tsx`
- `LayoutToolbar.tsx`
- `MainWindowBadge.tsx`
- `SynchronizerActionFeed.tsx`

规则：

- 组件只负责展示和交互发射
- 原生窗口动作仍只经 `desktop.ts`
- 状态由 `src/features/synchronizer/*` 提供
