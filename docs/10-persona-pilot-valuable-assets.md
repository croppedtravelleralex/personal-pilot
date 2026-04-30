# persona-pilot 高价值资产合入记录

日期：2026-04-30

本轮只吸收 `D:\SelfMadeTool\persona-pilot` 中低风险、高回报的资产，不做整仓合并，不迁移 package/toolchain，不替换 Go sidecar，不启用内嵌浏览器原型。

## 已合入的值钱部分

1. 工作台 UI 基础组件
   - `src/components/workbench/MetricStrip.tsx`
   - `src/components/workbench/PageHero.tsx`
   - `src/components/workbench/SectionTabs.tsx`
   - `src/components/workbench/WorkbenchActionStrip.tsx`
   - `src/components/workbench/WorkbenchSplit.tsx`

2. 通用交互组件
   - `src/components/TruthBoundaryBanner.tsx`
   - `src/components/InlineContentPreview.tsx`
   - `src/components/SearchInput.tsx`
   - `src/components/VirtualList.tsx`

3. feature-store 模板能力
   - `src/store/createStore.ts`
   - 提供 `createStore()`、`useStoreSelector()`、`createRequestGate()`。
   - 目标是支持 selector 读取、请求防陈旧结果、AbortController 取消旧请求。

4. features 目录规则
   - `src/features/README.md` 已扩展为业务域组织策略。
   - 新功能优先采用 `model/adapters/store/selectors/hooks` 切片。

5. Windows 调试入口
   - `scripts/windows/windows_ui_debug_entry.ps1`
   - 适配本项目 npm、Vite 5218 端口和 Tauri dev 链路。

## 本轮刻意不合入

- 不合并 `persona-pilot` 的 `package.json`、lockfile、pnpm、React 19、TypeScript 6、Vite 8。
- 不合并 Rust 后端原型和 `src-tauri` 命令层。
- 不合并内嵌浏览器 M1 原型；当前主线仍是外部真实 Chromium 窗口。
- 不合并 `data/`、`dist/`、`target/`、`node_modules/`、`.omx/`、`.codex_tmp/`。
- 不合并 `research/external/*` 作为运行时代码。

## 使用规则

- 新页面先使用 `PageHero`、`MetricStrip`、`SectionTabs` 建立统一工作台骨架。
- 有“本地数据不等于云端真实状态”或“能力边界”时使用 `TruthBoundaryBanner`。
- 日志、JSON、长文本预览使用 `InlineContentPreview`，避免无上限内联渲染。
- 列表超过 200 条时使用 `VirtualList` 或现有 `Table` 虚拟滚动能力。
- 搜索输入优先使用 `SearchInput` 或 `useDebouncedValue`，默认 300ms debounce。
- 异步搜索/筛选/刷新使用 `createRequestGate()`，旧请求返回时不得覆盖新结果。

## 后续推荐试点

首个试点建议选择 `Profiles` 或 `Proxies`，将页面逻辑拆成：

```text
src/features/<domain>/model.ts
src/features/<domain>/adapters.ts
src/features/<domain>/store.ts
src/features/<domain>/selectors.ts
src/features/<domain>/hooks.ts
```

完成一个 domain 后再决定是否推广到 `Tasks`、`Templates`、`Synchronizer`。
