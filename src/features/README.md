# features 目录策略

`features/` 只按业务域组织，不按 UI 类型组织。新功能优先放在这里，再由页面或组件读取。

推荐结构：

```text
src/features/<domain>/
  model.ts       # 业务类型、状态枚举、请求/响应类型
  adapters.ts    # desktop/service 返回值到前端模型的转换
  store.ts       # 业务状态，优先 selector 读取
  selectors.ts   # 派生数据，避免重复状态
  hooks.ts       # 页面/组件使用的入口
  README.md      # 当前 domain 的边界和验收规则
```

页面调用链保持：

```text
pages/components -> features/hooks/store -> services/desktop.ts -> tauri
```

硬规则：

- 页面和组件不直接调用 Tauri `invoke`，统一走 `src/services/desktop.ts`。
- 搜索、筛选、排序等异步交互必须做防陈旧结果保护，可用 `src/store/createStore.ts` 里的 `createRequestGate()`。
- 搜索默认使用 250-400ms debounce；本项目默认 300ms。
- 超过 200 条可见记录的列表或表格必须使用虚拟滚动、分页或增量加载。
- store 不保存大体量原始数据全集，优先保存当前页、索引、筛选条件和派生 selector。
- 新 domain 不要从 `pages/`、`components/` 反向 import。
