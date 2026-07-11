# 53 行为生物特征（L5）执行桥接工程指导与验收

Updated: 2026-07-08 (Asia/Shanghai)

## 0. 定位

- **目标**：消除「人化能力只在计划层、主执行路径却用固定模式」的分裂，使鼠标/键盘/滚动的**真实执行**具备可对抗行为生物识别（mouse/keystroke dynamics ML）的分布特征，且**per-profile 可复现、跨 profile 有差异**。
- **在优化轨道中的位置**：`docs/49` 修已有 hook 真实性、`docs/50` 战略维度、`docs/51` 指纹面广度、`docs/52` 设备人格相干性、**本文（53）= 行为执行层（L5）落地**。
- **强关联**：行为 seed 必须与 `docs/52` 的 `HumanizeSeed`/Persona 同源（同一用户的手法一致）；与 `docs/50` Input Plane（S0 OS / S1 CDP）协同。
- **诚实边界**：不承诺「行为不可区分」；目标是让行为落在**真实人群分布内**且不出现跨账号收敛的机器指纹。无证据不写 observed。

## 1. 证据基线：计划层 vs 执行层分裂

本轮核实（文件:行）：

| 编号 | 能力 | 计划层（已实现） | 执行层（实际调用） | 问题 |
| --- | --- | --- | --- | --- |
| L5-E1 | 鼠标轨迹 | `humanize/trajectory.go:47-79` `BuildFittsTrajectory`（Fitts+easeInOutCubic+pink noise） | `cdp_executor.go:861-907` `mouseMove` 用固定 `DefaultMouseProfile`（cp 0.3/0.7、`Sin(πt)`） | 计划未接线；执行是**固定 Bezier** |
| L5-E2 | 点击相位 | `BuildFourPhaseClickPlan`（press/overshoot 等） | `cdp_executor.go:740-773` `executeClick` 仅 hover→click 两段 | 无 overshoot/修正 |
| L5-E3 | 打字动力学 | `humanize/typing.go:64-103` inter-key 间隔 + 误触 | `cdp_executor.go:702-737` keyDown→char→keyUp **dwell≈0** | 无按键保持时间、无 bigram |
| L5-E4 | 滚动物理 | `scroll.go:33-97` 四段 overshoot；`136-191` 惯性/回读（仅测试） | `cdp_executor.go:356-390` 单次 `window.scrollBy(smooth)` | 惯性/回读未接线；用 JS 非 wheel 事件 |
| L5-E5 | 生物噪声 | `bio_noise.go:17-40` `NaturalDelay`；`OperationGap` 无调用方 | 仅 `primitive_executor` idle/pause 用 | 覆盖窄、未人格化 |
| L5-E6 | seed 复现 | — | `typing.go:59` 用全局 `rand.Float64()`，非 per-profile seed | 跨会话不可控/可收敛 |
| L5-E7 | OS 输入 | — | `wininput/mouse_windows.go:63-102` 另一套 Bezier，与 humanize 无关 | 两套轨迹源，OS 面未统一 |
| L5-E8 | `FingerSpeedRatio` | `typing.go:237-249` 定义 | 未被 `BuildTypingPlan` 使用 | 死代码 |

**结论**：主执行路径的鼠标（固定控制点 0.3/0.7 + SpeedMean=400 + `dist/15` 步数）、打字（线性 WPM、零 dwell）、滚动（单次 smooth）都是**统计上可收敛的固定模式**——这是行为 ML 最容易抓的跨账号共性。

## 2. 统一原则（先读）

1. **单一 seed 源**：轨迹/打字/滚动/噪声全部由 profile `HumanizeSeed`（+ Persona，见 `docs/52`）确定性派生；禁止全局 `rand` 无种子。
2. **分布而非定值**：速度、曲率、dwell、flight、暂停都从**人群分布**采样（seed 决定该 profile 落点），而非全局常量。
3. **计划层为唯一真相**：执行层必须调用 `humanize/*` 的 Plan，删除/降级执行层内联的固定 Bezier。
4. **OS/CDP 同源**：`wininput` 与 CDP 执行共用同一 trajectory/typing 计划源。
5. **一致性优先**：宁可平凡但自洽，不制造「完美但机械」的轨迹。

## 3. 任务分解

### P0

#### Task L5-1 — 鼠标执行接线 Fitts + 四段点击（L5-E1/E2）
- **技术指导**：
  1. `cdp_executor.go` 的 `mouseMove`/`executeClick` 改为消费 `BuildFittsTrajectory` 与 `BuildFourPhaseClickPlan` 输出的路点序列（含 overshoot→修正回拉），删除内联固定 cp 0.3/0.7。
  2. `MouseProfile` 从 Persona 派生（快/慢手、曲率、抖动幅度），而非全局 `DefaultMouseProfile`。
  3. 保留 `docs/49` D1 的 CDP 往返压缩（move 系列流水线、失败重试、OS fallback）。
- **AC**：
  - AC1：grep 断言 `executeClick`/`mouseMove` 调用 `BuildFittsTrajectory`/`BuildFourPhaseClickPlan`；执行层无固定 `0.3`/`0.7` 控制点常量。
  - AC2：单测：同 seed 轨迹可复现；不同 seed 曲率/速度分布不同；含 overshoot 段。
  - AC3：`go test ./backend/internal/behavior/... -count=1` passed。

#### Task L5-2 — 打字动力学（dwell/flight/bigram，per-seed）（L5-E3/E6/E8）
- **技术指导**：
  1. keyDown 与 keyUp 之间加 **dwell**（按键保持时间，seed+按键派生分布）；相邻键 **flight time** 按 bigram/手指切换分布；启用 `FingerSpeedRatio`。
  2. `BuildTypingPlan` 改用 per-profile seed（去 `typing.go:59` 全局 rand）。
  3. 保留误触+回删；密码域慢 40%（已有）。
- **AC**：
  - AC1：CDP smoke：keyDown→keyUp 间隔 >0 且随键变化；相邻键间隔非常数。
  - AC2：单测：同 seed 打同一串可复现；不同 seed 分布不同；`FingerSpeedRatio` 被使用（无死代码）。
  - AC3：`go test ./backend/internal/behavior/... -count=1` passed。

### P1

#### Task L5-3 — 滚动惯性/回读接线 + wheel 事件（L5-E4）
- **技术指导**：
  1. `primitive_executor` 的 `scroll_progressive` 与 `ExecuteHumanizedScroll` 路由到 `BuildScrollPlan`/`BuildInertialScrollPlan`（四段 overshoot→pause→return→microadjust + `MaybeAddReread`）。
  2. 评估用 CDP `Input.dispatchMouseEvent{type:mouseWheel}` 替代 JS `window.scrollBy`（wheel 事件更接近真实输入；JS scroll 无 isTrusted wheel）。
- **AC**：
  - AC1：grep 断言 `scroll_progressive` 经 `BuildScrollPlan`；含惯性与偶发回读。
  - AC2：滚动产生 wheel 事件（或明确记录 JS 回退边界）；XHS live feed 滚动回归通过。

#### Task L5-4 — OS 输入平面统一同源（L5-E7）
- **技术指导**：`wininput/mouse_windows.go` 与键盘 OS 路径改为消费同一 `humanize` 计划源（同 seed），S0 与 S1 手法一致；避免「同一用户 OS 点击与 CDP 点击轨迹风格不同」。
- **AC**：单测/对照：同 profile 的 OS 与 CDP 轨迹来自同一计划源（同 seed 参数）；`go test` passed。

#### Task L5-5 — 生物噪声人格化 + 操作间隔接线（L5-E5）
- **技术指导**：`NaturalDelay`/`OperationGap` 参数从 Persona 派生（不同用户不同节奏）；把 `OperationGap`（当前无调用方）接入 primitive 间与任务间隔。
- **AC**：`OperationGap` 有生产调用；操作间隔随 profile 不同；跨 session 稳定。

### P2

#### Task L5-6 — 行为生物特征门禁
- **技术指导**：新增 `scripts/behavior_biometrics_gate.ps1`：对多个 profile 采样 dwell/flight 分布、轨迹曲率、滚动模式，验证：(a) 落在真实人群分布区间；(b) 跨 profile 有差异（无收敛）；(c) 同 profile 跨 session 稳定。
- **AC**：门禁输出分布统计；跨 profile 差异达阈值、同 profile 稳定；不达标 `blocked`+`failureReason`。

## 4. 总验收门禁（本文任务）

```powershell
go test ./backend/internal/behavior/... ./backend/internal/behavior/humanize/... -count=1
.\scripts\behavior_biometrics_gate.ps1
.\scripts\platform_99_gate.ps1 -Track all
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda    # feed 滚动/输入回归不得下降
```

**通过判据**：执行层调用计划层（无内联固定 Bezier/零 dwell）；同 seed 可复现、跨 profile 有差异；OS 与 CDP 同源；XHS live 12/12 不回归；行为门禁分布达标。任一未达标 `blocked`+`failureReason`，禁止改写 accepted。

## 5. 与 49–52 的执行顺序

| 波次 | 本文任务 | 依赖 |
| --- | --- | --- |
| Wave 1 | L5-1、L5-2 | `docs/49` D1 鼠标往返压缩；`docs/52` DP1 Persona seed |
| Wave 2 | L5-3、L5-4 | `docs/50` Input Plane |
| Wave 3 | L5-5、L5-6 | 独立 |

## 6. 风险与回滚

| 风险 | 缓解 |
| --- | --- |
| 接线后轨迹变慢影响任务吞吐 | 速度分布含快手档；CDP 往返压缩（docs/49 D1）并行 |
| wheel 事件兼容性差 | 保留 JS scroll 回退并记边界 |
| 过度拟人反成新指纹 | 分布采样 + 跨 profile 差异门禁，避免统一参数 |
| 回滚 | 每 Task 独立提交；`~/.cursor/anti-lazy/scripts/rollback.py` |

## 7. 变更同步

每 Task 完成后更新 `docs/02-current-state.md`（behavior 执行层证据）与 `docs/04-improvement-backlog.md`；`docs/00-master-todo.md` 行为相关项回指本文 Task。
