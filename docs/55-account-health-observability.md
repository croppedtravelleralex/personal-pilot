# 55 账号健康与过检率可观测工程指导与验收

Updated: 2026-07-08 (Asia/Shanghai)

## 0. 定位

- **目标**：把「单次快照评分」升级为**按天/按账号/按站点的纵向指标**（过检率、挑战率、封号率、任务成功率），并把 warmup/动作预算做成**运行的闭环引擎**，让 `docs/50` 的「成本不对称」有真实数据支撑，让策略调整可被验证。
- **在优化轨道中的位置**：`docs/49–53` 提升单次隐身质量、`docs/54` 保证多开可靠、**本文（55）= 长期效果的度量与账号运营闭环**（L6 反馈的数据层）。
- **诚实边界**：指标只反映**已观测**的挑战/成功，不外推「平台永不封」；无真实运营数据的指标标 `insufficient_data`，不伪造。

## 1. 证据基线：可观测现状

本轮核实（文件:行）：

| 编号 | 主题 | 现状 | 证据 | 问题 |
| --- | --- | --- | --- | --- |
| AH-E1 | 账号健康 | 时点合成分（detection+identity+trajectory 三点平均） | `app_account_health.go:29-99` | 单次快照，非纵向 |
| AH-E2 | trajectory 输入 | 仅 createdAt/lastStart 两元数据点 | `session/trajectory_validator.go:73-87` | 不是真实行为时序 |
| AH-E3 | outcome 聚合 | 最近 **50** 条 successRate + bySite 计数 | `app_account_outcome.go:118-151` | 无按日分桶/趋势/挑战率 |
| AH-E4 | 挑战记录 | `asymmetric_challenges` 点状 INSERT | `app_stealth_engine.go:404-420` | 无 rate 统计 API |
| AH-E5 | 挑战反馈 | 48h 内≥3 次 → cooldown（阈值） | `asymmetric/cost.go:229-253` | 阈值规则，非时序分析 |
| AH-E6 | 事件日志 | 按时间/namespace 过滤，无 rollup | `app_eventlog.go:25-63` | 无 GROUP BY day / rate |
| AH-E7 | warmup 引擎 | `PlanDailySession`（new/risk/steady）存在 | `lifecycle/engine.go:45-54` | 逻辑在，未闭环 |
| AH-E8 | 每日周期 | `runDailyLifecycleCycle` **死代码** | `app_lifecycle.go`（全库无调用方） | warmup 未真正运行 |
| AH-E9 | cadence 配置 | `platform-packs/xhs/cadence.yaml` | 全库**无代码引用** | 配置从未被加载 |
| AH-E10 | 探测器 | 单次布尔探针→分数，无历史表 | `detection/auto_score.go:25-50` | 无 detector pass rate 趋势 |

**结论**：有点状记录（outcome、challenge、event）与单次评分，但**缺纵向指标层**（按天/按账号过检率、封号率、成功率趋势），且 **warmup/动作预算引擎未运行**（死代码 + 未加载的 yaml）。

## 2. 统一原则（先读）

1. **纵向优先**：核心交付是**时间序列指标**，不是又一个单次分。
2. **只报已观测**：指标基于真实记录；样本不足标 `insufficient_data`，不外推。
3. **闭环可验证**：挑战率上升 → 策略调整（pause/切 API/降频）→ 指标回落，全链可追踪（接 `docs/50` E 成本不对称）。
4. **复用现有表**：优先在 `workbench_detection_results`/`asymmetric_challenges`/`event_log` 上做 rollup，减少 schema 膨胀。
5. **账号即人格连续体**：warmup/预算与 `docs/52` Persona、`docs/50` B「同一用户」一致，不制造「新号却像老号」的矛盾。

## 3. 任务分解

### P0

#### Task AH1 — 纵向指标 rollup 层（AH-E3/E4/E6/E10）
- **技术指导**：
  1. 定义指标字典：`challenge_rate_7d`、`ban_rate_30d`、`task_success_rate_by_day`、`detector_pass_rate_7d`（按 profile / site / day）。
  2. 新增日粒度 rollup（新表 `account_health_daily` 或对现有表按日聚合的查询层）；样本不足返回 `insufficient_data`。
  3. 暴露 API：`AccountHealthTrend(profileId, metric, window)`；Dashboard 展示趋势。
- **AC**：
  - AC1：API 返回按日时间序列；样本不足标注而非 0/100。
  - AC2：单测：注入若干 challenge/outcome → rate 计算正确、分桶正确。
  - AC3：`go test ./backend/... -count=1` passed。

#### Task AH2 — warmup 引擎接线（激活死代码）（AH-E7/E8/E9）
- **技术指导**：
  1. `runDailyLifecycleCycle` 接入 scheduler（人类时间窗内触发），成为真实运行的每日 warmup。
  2. 加载 `platform-packs/*/cadence.yaml`（sessionMinutes/scrollPauseMs/maxActionsPerSession），驱动动作预算。
  3. `PlanDailySession` 的 new/risk/steady 决策消费 AH1 的 risk 指标。
- **AC**：
  - AC1：grep 断言 `runDailyLifecycleCycle` 有生产调用方；`cadence.yaml` 被真实加载解析。
  - AC2：单测：new_account 档动作数受 `maxActionsPerSession` 限制；risk 高时降频。
  - AC3：`go test ./backend/... -count=1` passed。

### P1

#### Task AH3 — 挑战↔成功↔动作关联分析（AH-E5）
- **技术指导**：把 `asymmetric_challenges` 与 `account_outcome`、动作日志按 profile/site/time 关联，识别「哪些操作/节奏/出口导致挑战率上升」；输出 per-profile 归因。
- **AC**：报告能给出挑战率与具体维度（站点/时段/出口/动作类型）的相关性；单测覆盖聚合逻辑。

#### Task AH4 — 成本不对称闭环接线（接 docs/50 E）
- **技术指导**：AH1 的 `challenge_rate` 作为 `docs/50` E `interceptCostIndex` 的输入；挑战率超阈自动触发 `docs/50` D3 降级（pause/切 API/换出口），并记录调整后指标变化。
- **AC**：挑战率超阈 → 自动降级事件 → 后续窗口挑战率对照可查（闭环证据）。

### P2

#### Task AH5 — 可观测门禁 + Dashboard
- **技术指导**：新增 `scripts/account_health_observability_gate.ps1`：校验 rollup 指标可算、warmup 引擎在跑、闭环事件链完整；Dashboard 增加账号健康趋势视图。
- **AC**：门禁通过；Dashboard 显示按日趋势与 `insufficient_data` 标注。

## 4. 总验收门禁（本文任务）

```powershell
go test ./backend/... -count=1
.\scripts\account_health_observability_gate.ps1
.\scripts\platform_99_gate.ps1 -Track all
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda    # 回归不得下降
```

**通过判据**：纵向指标可算且样本不足有标注；warmup 引擎有生产调用、cadence.yaml 被加载；挑战率闭环可追踪；无伪造 accepted。任一未达标 `blocked`+`failureReason`。

## 5. 与其它文档的关系

- 数据喂给 `docs/50` E（成本不对称度量）与 D3（挑战反馈闭环）。
- warmup/预算与 `docs/52` Persona 老化（DP4）、`docs/50` B3 seed 生命周期锁一致，不制造账号年龄矛盾。
- 复用 `docs/43-capability-scenario-test-suite.md` 的 T3/T5 长稳口径。

## 6. 风险与回滚

| 风险 | 缓解 |
| --- | --- |
| 指标外推成「过检率保证」 | 强制 `insufficient_data` 标注；文档写明只反映已观测 |
| rollup 表膨胀/写放大 | 优先查询层聚合；必要时日表 + 定期归档（参考 `docs/sql-write-amplification-audit.md`） |
| warmup 接线影响现有调度 | 灰度开关；先 dry-run 记录计划一轮 |
| 闭环误触降级 | 阈值可配 + 需连续窗口确认再降级 |
| 回滚 | 每 Task 独立提交；`~/.cursor/anti-lazy/scripts/rollback.py` |

## 7. 变更同步

每 Task 完成后更新 `docs/02-current-state.md`（可观测指标证据）与 `docs/04-improvement-backlog.md`；`docs/00-master-todo.md` 账号/规则相关项回指本文 Task。
