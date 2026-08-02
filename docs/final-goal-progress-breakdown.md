# Final Goal Progress Breakdown
Updated: 2026-06-22 (Asia/Shanghai)

## Current Split

- mainline delivery split: `100% / 0%`
- mainline quality gate color: `green`
- local self-use split: `100% / 0%`
- local self-use color: `green`
- only unverified item: CAPTCHA / SMS / Email 真实账号凭证 smoke
- local-only scope: release performance budget、外部分发 smoke、干净 Win11/第二机和跨机器 SessionBundle portability 已取消，只保留历史诊断材料

## Mainline 到达 100 / 0 的原因

从 `95% / 7%` 到 `100% / 0%` 通过 multi-agent worktree 并行执行闭环了最后 3 个 P0 项：

- `changeProxyIp` 从本地桩升级为真实 provider 级 HTTP 轮换引擎（POST/PUT/PATCH，重试/冷却/回滚，7 种错误分类）
- synchronizer 原生广播写入路径落地：物理 `SetWindowPos` 窗口排布，确定性排序
- recorder/templates native-first 降级闭环：desktop session 守卫，空状态模板选择修复，源消息精确化
- engineering hygiene 一并闭环：SQLite 路径 env var 降级、CI workflow、`.env` gitignore、`package-lock.json` 清理
- 所有代码经 4 个独立 subagent 审查，2 CRITICAL + 1 HIGH + 4 MEDIUM 问题在合并前修复

## 为什么本机自用现在是 100 / 0

当前目标已经明确收敛为本机自用，不再追外部分发、第二机、release performance budget、AdsPower 刷分或远程代理账号。已闭环的本机能力：

- 第一族控制 schema 已声明 `80` 个核心控制字段
- 当前运行时投影已扩到 `26` 个字段（`25` 个 control-supported + derived `platform`）
- cookie/localStorage/sessionStorage 重启持久化已落地
- `SessionBundle` profile-scoped export、import preflight、dry-run 和 confirmed local restore write path 已落地；最新本机 restore smoke 为 `local_restore_verified`，M8 本机 restore gate 为 `passed_local_restore_verified`；跨机器 profile portability smoke 已按本机自用范围取消
- Validation Board 已有 native DNS/transport reports、desktop WebView scoped probes、profile runtime probe contract、report history、profile-level export、P6 evidence metadata 和 P7 fingerprint observation audit
- 当前行为运行时只支持 `13` 个真实原语
- `450` 指纹信号 taxonomy seed 和 `450` 事件 taxonomy seed 已落地；strict observed coverage 为 `450 / 450`，本地 deterministic replay runtime `461 / 450` 且 `contractOnly=0`
- M10 headed stability/coherence、M15 browser process prewarm/CDP/RSS/cleanup、M15 pool/process integration、本机 TLS/transport 和 runtime adapter local self-use 均已通过
- CAPTCHA/SMS/Email 本地 readiness、dry-run、failure taxonomy、Settings operator surface 已落地；真实账号凭证 smoke 因缺少服务商账号未验

## 已闭环的 7% 是什么

这是最后的原生闭环切片，Round 19 完成交付：

1. provider 侧代理轮换写入 — **已完成**：真实 HTTP POST/PUT/PATCH 到 provider 端点
2. synchronizer 原生批量/广播写入 — **已完成**：物理 SetWindowPos + 确定性排序
3. recorder/templates 原生深度闭环 — **已完成**：desktop session 守卫 + 空状态修复

## 当前剩余是什么

只剩一类：CAPTCHA / SMS / Email 服务商真实账号凭证验证。需要用户提供真实账号、余额、API key、测试目标和允许消耗额度的确认。

## 汇报规则

默认使用本机自用规则：

- `主线交付: 100% / 0%`
- `本机自用: 100% / 0%`
- `未验: CAPTCHA / SMS / Email 真实账号凭证 smoke`

历史的 `77% / 23%` (historical-only) 审计重置仅作为上下文保留。

详细阶段板、评分卡和 AdsPower 基准摘要请参阅 `docs/19-phase-plan-and-scorecard.md`。
