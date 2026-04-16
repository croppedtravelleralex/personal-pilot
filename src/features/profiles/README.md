# Profiles Feature

负责 `Profiles` 主工作台，不直接处理 native 细节。

建议内部继续拆成：

- `store.ts`
  - 过滤条件
  - 表格分页
  - 当前选中 profile
  - 抽屉开关
  - 批量选中状态
- `hooks.ts`
  - 页面 view model
  - 防抖搜索
  - stale-result 保护
- `model.ts`
  - profile row
  - filter option
  - drawer tab
- `selectors.ts`
  - 当前筛选后的表格数据
  - 当前批量动作可用状态

二级子模块边界：

- Toolbar
- Filter Rail
- Profiles Table
- Selection And Batch Actions
- Details Drawer
- Create/Edit Wizard

预期依赖的桌面契约：

- `listProfilePage`
- `readProfileDetail`
- `createProfile`
- `updateProfile`
- `startProfiles`
- `stopProfiles`
- `openProfiles`
- `checkProfileProxies`
- `syncProfiles`
