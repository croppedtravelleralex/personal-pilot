# Automation Components

这里放 `Automation / Recorder / Templates` 相关展示组件。

建议拆成：

- `RunsBoard.tsx`
- `TemplatesBoard.tsx`
- `RunLauncher.tsx`
- `RunDetailPanel.tsx`
- `RecorderTimeline.tsx`
- `TemplateVariableEditor.tsx`

规则：

- 组件只负责展示和交互发射
- 录制器状态机不写在组件里
- 状态分别由 `src/features/automation/*`、`src/features/recorder/*`、`src/features/templates/*` 提供
