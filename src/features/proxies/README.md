# Proxies Feature

负责本地 `Proxy Center`，不直接操作 UI 之外的系统能力。

建议内部继续拆成：

- `store.ts`
  - 列表分页
  - 过滤条件
  - 当前选中代理
  - 批量检测状态
- `hooks.ts`
  - 代理列表 view model
  - 批量检测动作
- `model.ts`
  - proxy row
  - proxy health
  - usage mapping

二级子模块边界：

- Proxy Table
- Batch Check
- Usage Mapping
- Change-IP Actions
- Filtering

预期依赖的桌面契约：

- `listProxyPage`
- `checkProxyBatch`
- `readProxyUsage`
- `changeProxyIp`
