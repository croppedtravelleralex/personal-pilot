# 当前 23% 收口计划（原“最后18%交付计划”）
Updated: 2026-04-16 (Asia/Shanghai)

## 2026-04-16 Re-Verify Delta

- Ship hardening 增补一条明确 blocker：Rust 测试门当前不绿
- 在 `B-008 / B-010 / B-009` 之外，当前还需要补一条实际执行项：
  - 修复 `humanize::*` 随机失败
  - 收敛 full `cargo test` 下的 `database is locked`

## 状态说明

这份文件保留原路径，方便历史链接继续可用；但旧的 `82% / 18%` 已经作废。
当前有效口径改为：

- whole-app progress: `77%`
- remaining delivery slice: `23%`

如果这里和 `docs/02-current-state.md`、`docs/17-full-app-audit-progress-reset.md` 冲突，以后两者为准。

## 目的

把当前“能用的本地桌面 Beta”收口成“可交付的完整 App”。
这 `23%` 不是新功能扩张，而是把主线做真、做稳、做可验收。

## 当前证据

- `src/features/synchronizer/store.ts`、`src/features/profiles/adapters.ts`、`src/features/proxies/adapters.ts`、`src/features/recorder/store.ts` 仍保留 `fallback / mock / staged` 路径
- `pnpm build` 仍保留 Vite chunk size warning
- `changeProxyIp`、`Synchronizer`、`Recorder / Templates` 仍能继续向 `provider / native / batch` 方向加深

## 完成定义

1. 所有主页面都能以真实数据或明确降级态工作，且降级态有显式标记。
2. `launch -> detail -> retry/cancel/manual gate` 稳定跑通。
3. `changeProxyIp` 具备真实 provider 换 IP 语义，而不是本地占位闭环。
4. `Synchronizer` 的 `main / layout / focus / broadcast` 具备原生闭环。
5. release 构建、测试、Win11 验收全部通过。
6. 默认用户路径不再依赖未说明的 mock / seed 数据。

## 推荐推进顺序

### 1. Proxy Identity & IP Rotation

目标：把 `changeProxyIp` 从本地追踪闭环推进到 provider-grade 换 IP 与 sticky residency 语义。

验收：

- change IP 的状态可追踪、可复验、可解释
- 批量检查、健康评分、冷却窗口和失败反馈保持一致
- release 默认路径不再把“本地占位换 IP”当成真实闭环

### 2. Synchronizer Native Batch / Broadcast

目标：把 `Synchronizer` 的 `staged broadcast / settings` 继续下沉到 native batch / broadcast 能力。

验收：

- `readSnapshot / setMain / layout / focus / broadcast` 形成统一 native 主路径
- `staged` 仅保留为显式过渡态，而不是默认执行态
- UI 对 `native / staged / fallback` 的标识一致

### 3. Recorder / Templates Native-First Closure

目标：把 `Recorder / Templates` 的 release 默认路径改成 native-first，并收掉 seed / fallback 对真实交付度的污染。

验收：

- release 默认路径不再把 seed / adapter fallback 当主数据源
- recorder 真采集、模板真读写和运行链路保持一致
- 页面状态能准确反映真实来源

### 4. Tasks Route / Surface Unification

目标：决定 `Tasks` 是独立入口还是并入 `Automation`，并按结论真正收口主壳主路由。

验收：

- 不再存在“代码存在但主入口不可达”的半落地状态
- 文档、导航、路由、工作台口径一致

### 5. Release Hardening & Ship

目标：把功能收口成可交付版本。

验收：

- `pnpm typecheck`
- `pnpm build`
- `cargo test --lib -- --test-threads=1`
- `cargo build --release`
- `scripts/windows_local_verify.ps1`
- Win11 baseline enforcement

## 当前切片映射

| 顺序 | 任务 | 对应 backlog |
| --- | --- | --- |
| 1 | Proxy Rotation | `B-004` |
| 2 | Synchronizer Native Closure | `B-005` |
| 3 | Recorder / Templates De-Fallback | `B-011` |
| 4 | Tasks Route / Surface | `B-012` |
| 5 | Ship Hardening | `B-008` / `B-010` / `B-009` |
