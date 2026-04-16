# 当前主线 7% 收口计划（原“最后18%交付计划”）
Updated: 2026-04-17 (Asia/Shanghai)

## 2026-04-17 Reality Delta

- `changeProxyIp` 已不再是旧的本地 `queued` 假成功路径，而是 provider refresh-backed write
- `Synchronizer` 已不再停留在 live read / focus；`setMain / layout / broadcast` typed desktop contract 已落地
- 当前主线残余不再是“有没有 A1/A2 链路”，而是 provider hardening、physical execution 深度、口径和验收收口

## 状态说明

这份文件保留原路径，方便历史链接继续可用；但旧的 `82% / 18%` 已经作废。
当前有效口径改为：

- mainline delivery: `95% / 7% / green`
- overall end-state: `30% / 70% / yellow`

如果这里和 `docs/02-current-state.md`、`docs/17-full-app-audit-progress-reset.md` 冲突，以后两者为准。

## 目的

把当前“已经可用的本地桌面 App”继续收口成“主线真实闭环、口径统一、验收可复现”的可交付版本。
这 `7%` 是风险加权收口，不是简单算术余量。

## 当前证据

- `changeProxyIp` 已进入真实 provider refresh 路径，但 success-path proof、config carrier 和同步阻塞策略还未最终收口
- `Synchronizer` 已具备 `readSnapshot / setMain / layout / focus / broadcast` 的统一 desktop contract 主路径，但 `layout / broadcast` 仍不是物理执行闭环
- `Recorder / Templates` 仍能继续向 native-first capture / replay 深化

## 完成定义

1. 所有主页面都能以真实数据或明确降级态工作，且降级态有显式标记。
2. `launch -> detail -> retry/cancel/manual gate` 稳定跑通。
3. `changeProxyIp` 具备真实 provider-side write 语义，并明确区分“写入已受理”与“exit-IP 已观测变更”。
4. `Synchronizer` 的 `main / layout / focus / broadcast` 形成统一 desktop contract 主路径，并明确区分 state/intention write 与 physical execution。
5. release 构建、测试、Win11 验收全部通过。
6. 默认用户路径不再依赖未说明的 mock / seed 数据。

## 推荐推进顺序

### 1. Proxy Identity & IP Rotation

目标：把 `changeProxyIp` 从“真实 provider write 已落地”推进到“可证明、可维护、可扩展”的 provider-grade rotation contract。

验收：

- change IP 的状态可追踪、可复验、可解释
- 批量检查、健康评分、冷却窗口和失败反馈保持一致
- success/failure/rollback 的 operator 口径稳定且可复验
- release 默认路径不再把“写入已受理”误报成“出口已完成漂移”

### 2. Synchronizer Native Batch / Broadcast

目标：把 `Synchronizer` 从“typed state/intention write 已落地”推进到“physical layout / broadcast execution 更深一层闭环”。

验收：

- `readSnapshot / setMain / layout / focus / broadcast` 形成统一 desktop contract 主路径
- `prepared` 仅保留为显式准备态，而不是被误写成已执行 fallback
- UI 对 `native / prepared / fallback` 的标识一致

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
