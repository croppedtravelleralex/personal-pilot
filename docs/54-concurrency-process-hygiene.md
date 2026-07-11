# 54 并发规模与进程卫生工程指导与验收

Updated: 2026-07-08 (Asia/Shanghai)

## 0. 定位

- **目标**：让 Personal Pilot 在**同时运行多个 profile**时稳定、无资源泄漏、无端口竞争、崩溃可自愈——这是与 AdsPower/BitBrowser「多开」体验对齐的运维地基。
- **在优化轨道中的位置**：`docs/49–53` 关注单实例的指纹/行为质量；**本文（54）= 规模与进程生命周期的可靠性**，是多开落地的前提。
- **诚实边界**：本机自用范围，不追云端/团队编排；目标是「本机 N 开稳定 + 崩溃自愈 + 资源可控」。无证据不写 observed。

## 1. 证据基线：并发与清理现状

本轮核实（文件:行）：

| 编号 | 主题 | 现状 | 证据 | 问题 |
| --- | --- | --- | --- | --- |
| CP-E1 | 浏览器 pool | `internal/pool/engine.go` 有 Slot/MaxSlots/Budget/Cleanup 设计 | `pool/engine.go:1-49`；backend 主程序**零引用** | pool 未接入生产，启动是 one-shot |
| CP-E2 | debug 端口分配 | `nextAvailablePort` = Listen:0→Close→sleep→复验，**无进程级预留** | `app_utils.go:34-53`、`app_instance.go:256` | 并发启动存在 TOCTOU 冲突风险 |
| CP-E3 | 端口预留正例 | sing-box 用 `reservePortNumber`（持有 release） | `port_reserve.go:17-38`、`singbox.go:122` | 浏览器端口未复用该机制 |
| CP-E4 | sing-box 桥池化 | refcount + 45s idleTTL + 15s cleanup + 复用 | `singbox.go:22-24,238-311,341-349,587-619` | **已良好**（保留） |
| CP-E5 | 退出清理 | 并行 kill browser + StopAll xray/singbox/clash + `killResidualRuntimeProcesses` | `app_shutdown.go:17-85` | 仅**应用退出**时清理 |
| CP-E6 | 崩溃检测 | 异常退出 → 释放桥接；5min 内 3+ 次 → `risk:browser:crash-loop` | `app_instance.go:720-768` | 有检测，无运行期孤儿 reconcile |
| CP-E7 | detached 窗口 | launcher 退出但 CDP 仍活 → 轮询端口最终 markStopped | `app_instance.go:776-816` | 半死状态无超时强制回收 |
| CP-E8 | 孤儿清理事件 | `system:recovery:orphan-cleanup` schema + emitter 存在 | `events/registry.go:1343` | backend **无 emit 调用**（事件死） |
| CP-E9 | 资源预算 | `pool` 有 `WorkingSetMB`/`BudgetStatus` 字段 | 未与 App 集成；`GetDashboardStats` 仅计数 | 无「最大并发/总 RSS/总子进程」策略 |
| CP-E10 | 并发上限 | `MaxProfileLimit` 限**配置档案数** | `browser/profile.go:306` | 非**同时运行**上限 |

**结论**：sing-box 桥池化良好，但**浏览器实例无池化、端口无预留、无运行期资源预算、孤儿/半死清理不完整**——N 开时可能端口冲突、资源耗尽或残留僵尸进程。

## 2. 统一原则（先读）

1. **预留优于探测**：端口、slot、桥接采用「acquire→hold→release」而非「探测即用」。
2. **运行期自愈**：不只在退出清理；运行期周期性 reconcile 孤儿/半死资源。
3. **背压优于崩溃**：超预算时**拒绝启动并给明确原因**，不硬撑到 OOM。
4. **复用已有正例**：端口用 `reservePortNumber`；桥接沿用 `singbox` refcount 模型。
5. **事件必须落地**：定义了的 `orphan-cleanup`/`crash-loop` 必须真实 emit 且可观测。

## 3. 任务分解

### P0

#### Task CP1 — Debug 端口全局预留（消除 TOCTOU）（CP-E2/E3）
- **技术指导**：把浏览器 debug 端口分配从 `nextAvailablePort` 改为 `reservePortNumber` 式：分配后**持有**到 Chrome 真正绑定成功再 release；维护进程内 `reservedPorts` 集合避免并发重复分配。
- **AC**：
  - AC1：单测/smoke：并发启动 N（≥10）个 profile 无端口冲突、无 `bind` 失败。
  - AC2：分配的端口在 Chrome 就绪前不被二次分配（并发单测断言）。
  - AC3：`go test ./backend/... -count=1` passed。

#### Task CP2 — 运行期资源预算与背压（CP-E9/E10）
- **技术指导**：
  1. 定义 `RuntimeBudget{ maxConcurrentInstances, maxTotalRSSMB, maxSubprocesses }`（可配置）。
  2. 实例启动前校验预算；超限**拒绝并返回明确错误**（不静默）。
  3. `GetDashboardStats` 扩展：当前并发数、总 RSS、子进程数、预算余量。
- **AC**：
  - AC1：单测：超 `maxConcurrentInstances` 时启动被拒且错误含预算维度。
  - AC2：Dashboard/stats 返回资源用量与余量。
  - AC3：`go test ./backend/... -count=1` passed。

### P1

#### Task CP3 — 运行期孤儿/僵尸 reconcile 循环（CP-E5/E6/E8）
- **技术指导**：
  1. 新增周期性 reconcile（类比 `singbox` cleanupLoop）：扫描已 markStopped 但仍存活的 chrome/sing-box 子进程、无主 CDP 连接的端口，安全清理。
  2. 清理时真实 `EmitSystemRecoveryOrphanCleanup`（激活 CP-E8 死事件）。
  3. 与 `app_shutdown` 退出清理共用底层 kill 逻辑。
- **AC**：
  - AC1：smoke：kill launcher 制造孤儿 → reconcile 清理 + 发出 `system:recovery:orphan-cleanup` 事件（事件日志可查）。
  - AC2：无误杀正常运行实例（单测）。

#### Task CP4 — Detached/半死状态超时强制回收（CP-E7）
- **技术指导**：`waitDetachedBrowser` 增加**上限超时**：半死端口超时后强制 markStopped + 释放桥接 + 回收端口，不无限占用。
- **AC**：单测：半死端口在超时后被强制回收，桥接 refcount 归零。

#### Task CP5 — 浏览器 pool 接入评估（CP-E1）
- **技术指导**：评估把 `internal/pool` 接入启动路径做**预热池**（降低冷启动，对标商业多开秒开）；或明确记录「本机自用 one-shot 足够」的边界决策，避免 `pool` 长期悬空。
- **AC**：要么 pool 接入且 prewarm 生效（冷启动下降，记录 before/after）；要么 ADR 明确不接入并把 `pool` 标注为实验/移除，二选一有结论。

### P2

#### Task CP6 — 并发 smoke 门禁强化
- **技术指导**：扩展现有 `scripts/concurrency_smoke.ps1`：N 开并行启动/停止、端口无冲突、无残留进程、RSS 在预算内、崩溃自愈。
- **AC**：`concurrency_smoke.ps1` 覆盖 N≥10；结束后无孤儿 chrome/sing-box；预算不超。

## 4. 总验收门禁（本文任务）

```powershell
go test ./backend/... -count=1
.\scripts\concurrency_smoke.ps1                    # N 开并发 + 清理 + 预算
.\scripts\platform_99_gate.ps1 -Track all
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda    # 回归不得下降
```

**通过判据**：N≥10 并发无端口冲突、无残留进程；超预算被明确拒绝；孤儿被运行期 reconcile 清理并发事件；半死端口超时回收；XHS live 12/12 不回归。任一未达标 `blocked`+`failureReason`。

## 5. 与其它文档的关系

- sing-box 桥池化模型（CP-E4）是本文端口/资源预留的参考正例，勿破坏。
- 与 `docs/50` C2（remote-debugging-pipe）协同：若改 pipe 传输，端口预留逻辑相应简化。
- 与 `docs/29-proxy-supply-chain.md` 的桥接生命周期一致。

## 6. 风险与回滚

| 风险 | 缓解 |
| --- | --- |
| 端口预留引入死锁/泄漏 | release 用 defer + 超时；复用 `port_reserve.go` 成熟逻辑 |
| reconcile 误杀正常进程 | 严格匹配 profileId/PID/端口三元组；先 dry-run 日志一轮 |
| 资源预算阈值不当误拒 | 阈值可配置 + 观察一轮再收紧 |
| pool 接入引入复杂度 | 先 ADR 决策，不确定则保持 one-shot |
| 回滚 | 每 Task 独立提交；`~/.cursor/anti-lazy/scripts/rollback.py` |

## 7. 变更同步

每 Task 完成后更新 `docs/02-current-state.md`（并发/清理证据）与 `docs/04-improvement-backlog.md`；若做 pool 决策，写入 `docs/36-browser-pool-management.md` 或标注其状态。
