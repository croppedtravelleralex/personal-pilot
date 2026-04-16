# Profiles Components

这里放 `Profiles` 工作台专用展示组件。

建议拆成：

- `ProfilesToolbar.tsx`
- `ProfilesFilterRail.tsx`
- `ProfilesTable.tsx`
- `ProfilesBatchBar.tsx`
- `ProfileDrawer.tsx`
- `ProfileWizard.tsx`

规则：

- 组件只负责展示和交互发射
- 不直接调用 `desktop.ts`
- 状态由 `src/features/profiles/*` 提供
