# Features Map

`src/features/` 只放业务域状态、hooks、selectors、view model 和模块内编排。

统一规则：

- 页面只负责渲染和交互编排
- native 调用只允许经 `src/services/desktop.ts`
- 每个 feature 目录优先自行维护：
  - `store.ts`
  - `hooks.ts`
  - `types.ts` 或 `model.ts`
  - `selectors.ts`

当前目录分为两类：

- 已接线模块：
  - `status`
  - `runtime`
  - `tasks`
  - `logs`
  - `settings`
- 已预留的新工作台模块：
  - `profiles`
  - `proxies`
  - `automation`
  - `recorder`
  - `templates`
  - `synchronizer`
