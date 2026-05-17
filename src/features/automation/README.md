# Automation Feature

负责 `Automation` 页面，不包含录制器底层状态机本体。

建议内部继续拆成：

- `store.ts`
  - runs 视图状态
  - template 选中态
  - launch panel 状态
- `hooks.ts`
  - runs board view model
  - template board view model
  - launcher actions
- `model.ts`
  - run summary
  - run detail
  - launch request

二级子模块边界：

- Runs Board
- Templates Board
- Run Launcher
- Run Detail
- Local API Surface

预期依赖的桌面契约：

- `listRunPage`
- `listTemplatePage`
- `launchTemplateRun`
- `readRunDetail`
