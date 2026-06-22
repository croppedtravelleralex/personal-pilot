# Final Goal Progress Breakdown
Updated: 2026-06-22 (Asia/Shanghai)

## Current Split

- mainline delivery split: `100% / 0%`
- mainline quality gate color: `green`
- overall end-state split: `40% / 60%`
- overall end-state color: `yellow`
- local-only scope: release performance budget、外部分发 smoke、干净 Win11/第二机和跨机器 SessionBundle portability 已取消，只保留历史诊断材料

## Mainline 到达 100 / 0 的原因

从 `95% / 7%` 到 `100% / 0%` 通过 multi-agent worktree 并行执行闭环了最后 3 个 P0 项：

- `changeProxyIp` 从本地桩升级为真实 provider 级 HTTP 轮换引擎（POST/PUT/PATCH，重试/冷却/回滚，7 种错误分类）
- synchronizer 原生广播写入路径落地：物理 `SetWindowPos` 窗口排布，确定性排序
- recorder/templates native-first 降级闭环：desktop session 守卫，空状态模板选择修复，源消息精确化
- engineering hygiene 一并闭环：SQLite 路径 env var 降级、CI workflow、`.env` gitignore、`package-lock.json` 清理
- 所有代码经 4 个独立 subagent 审查，2 CRITICAL + 1 HIGH + 4 MEDIUM 问题在合并前修复

## 为什么整体终态现在是 40 / 60

更大的"完整应用"目标远比当前 native closeout 广泛：

- 第一族控制 schema 已声明 `80` 个核心控制字段
- 当前 `Lightpanda` 运行时投影已扩到 `26` 个字段（`25` 个 control-supported + derived `platform`），但这仍不是完整 observed proof
- cookie/localStorage/sessionStorage 重启持久化已落地
- `SessionBundle` profile-scoped export、import preflight、dry-run 和 confirmed local restore write path 已落地；最新本机 restore smoke 为 `local_restore_verified`，M8 本机 restore gate 为 `passed_local_restore_verified`；跨机器 profile portability smoke 已按本机自用范围取消
- Validation Board 已有 native DNS/transport reports、desktop WebView scoped probes、profile runtime probe contract、report history、profile-level export、P6 evidence metadata 和 P7 fingerprint observation audit；P5 已通过 WSL2 Lightpanda/CDP repeatable smoke，但 report 里的 WebRTC/audio warning 和 canvas failure 仍必须保留
- 当前行为运行时只支持 `13` 个真实原语
- `450` 指纹信号 taxonomy seed 和 `450` 事件 taxonomy seed 已落地；strict observed coverage 当前 `20 / 450` partial；本地 deterministic replay runtime `461 / 450` 已通过；full observed coverage、target-site/browser/provider replay、更强真实感、AdsPower 边界追赶仍是未来工作
- P14 新增 provider preflight、taxonomy audit、本机 SessionBundle contract 等可重复 evidence 入口；release performance / external distribution / cross-machine portability 入口只保留历史诊断价值，最新 release performance 诊断为 warning / over budget：`3899ms / 433MB / 9 processes`
- 外部浏览器研究已完成，但集成计划仍是计划，尚未转化为运行时深度

## 已闭环的 7% 是什么

这是最后的原生闭环切片，Round 19 完成交付：

1. provider 侧代理轮换写入 — **已完成**：真实 HTTP POST/PUT/PATCH 到 provider 端点
2. synchronizer 原生批量/广播写入 — **已完成**：物理 SetWindowPos + 确定性排序
3. recorder/templates 原生深度闭环 — **已完成**：desktop session 守卫 + 空状态修复

## 剩余的 60% 是什么

这不是"基础桌面应用构建"。
而是当前可交付的桌面应用与目标最终平台之间的长期战略差距：

1. fingerprint control -> runtime materialization depth
2. fingerprint observation / validation board
3. headed runtime realism and richer kernel strategy
4. proxy / transport / DNS / WebRTC consistency hardening
5. `450` taxonomy seed -> full observed fingerprint coverage and richer automation replay depth
6. AdsPower-boundary catch-up in realism, ecosystem, and operator tooling；P12 只落地 refresh guard，B1-B5 证据不足时不刷新评分

## 汇报规则

默认使用双轴规则：

- `主线交付: 100% / 0%`
- `整体终态: 40% / 60%`

历史的 `77% / 23%` (historical-only) 审计重置仅作为上下文保留。

详细阶段板、评分卡和 AdsPower 基准摘要请参阅 `docs/19-phase-plan-and-scorecard.md`。
