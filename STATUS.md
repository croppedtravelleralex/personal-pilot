# STATUS.md

## 当前状态摘要

- **状态：** 已从“最小可运行原型”继续推进到 **代理质量闭环 + 策略收敛阶段**
- **日期：** 2026-03-31
- **当前焦点：** 继续把 **proxy selection** 从多段规则叠加收敛到 **trust score 核心表达**，同时补强 **verify 慢路径**、**高并发性能治理** 和 **策略可观测性**。

## 本文件用途

`STATUS.md` 只保留：
- **当前状态**
- **当前风险**
- **当前下一步**
- **本轮体检**

更完整的进展说明请看：
- `PROGRESS.md` — 已实现能力时间线
- `ROADMAP.md` — 过去 / 现在 / 未来路线图
- `EXECUTION_LOG.md` — 每轮执行记录
- `RUN_STATE.json` — 调度状态
- `STAGE_SUMMARY_2026-03-31.md` — 阶段总结

## 当前状态

当前系统已经具备以下主线能力：

1. **执行与调度控制面**
   - DB-first queue
   - claim / reclaim / retry / cancel
   - 多 worker 并发执行
   - 基础观测与状态暴露

2. **Fingerprint 控制面**
   - profile 创建 / 查询 / 绑定 / 校验
   - runner 注入入口第一版
   - fake / lightpanda 统一 profile 视图

3. **Proxy pool 与 selection**
   - proxy CRUD
   - provider / region / min_score / cooldown 过滤
   - sticky session 正式绑定表 `proxy_session_bindings`
   - 代理选择策略层第一版
   - `ProxySelectionTuning` 参数注入入口
   - trust score 起点与主链接入

4. **Proxy verification / 巡检 V1**
   - smoke: TCP connect + HTTP CONNECT 响应判断
   - verify: 出口 IP / 国家 / 地区 / geo match / anonymity 信号
   - `verify_proxy` task kind
   - `POST /proxies/verify-batch`
   - verify batch 查询与批次回看
   - 巡检结果开始反哺 selection

## 当前风险

1. **trust score 还没有完全成为唯一主语义。**
   当前已经接入主排序，但仍保留 `score DESC` 作为次级兜底；部分语义仍分散在 score、规则和兜底排序之间，后续还要继续收敛。

2. **verify 慢路径还偏轻。**
   当前 verify 已经有 geo / anonymity / upstream 信号，但本质仍是轻量探测，距离“更真实的出口真实性与匿名性校验链”还有距离。

3. **高并发下的写放大与聚合成本还没正式治理。**
   当前 selection、verify batch、status 聚合、代理健康回写都已进入真实链路，后续要重点评估 SQL 压力、写频率与索引策略。

4. **策略可解释性还不够强。**
   当前已经能调 tuning，但还缺少更直接的 explainability / metrics，让外部快速知道“为什么这次选了这个代理”。

5. **Lightpanda 对更真实 fingerprint 的消费边界还未真正落地。**
   当前 profile 注入入口已具备，但真实浏览器侧还未进入更深一层的消费与性能验证。

## 当前执行工作流

- 默认每轮先读取 `VISION.md`、`CURRENT_DIRECTION.md`、`TODO.md`，必要时补读 `STATUS.md` / `PROGRESS.md`。
- 默认先给出 **3–5 个下一阶段最合适做的事情**，按优先级排序。
- 默认执行前 **2 个**，执行后再次发散并重新排序。
- 周期性进入 **查 bug / 修 bug** 双步骤环，修复后 commit，并在条件允许时 push。
- 异步/队列场景默认使用短轮询 + 小延迟，优先避免 flaky。

## 当前下一步

### P0
1. **继续推进 trust score 核心化**，减少分散排序项与兜底逻辑依赖。
2. **推进 verify 慢路径**，补更真实的匿名性 / 地区 / 出口真实性校验链。
3. **做一轮高并发性能治理**，重点检查 selection SQL、status 聚合、verify batch 与代理回写的写放大。
4. **补策略 explainability / metrics**，让 selection 决策不再是黑盒。

### P1
5. 设计代理质量评分系统正式形态。
6. 设计 `SessionIdentity / ExecutionIdentity`，把 `proxy + fingerprint + region + risk_level` 收到统一表达。
7. 继续压 panic 风险点、锁竞争风险点与 flaky 测试。
8. 继续完善 API / 运维 / 能力说明文档。

## 本轮体检（2026-03-31）

- **找 bug：** 本轮最明显的问题不是代码主链缺失，而是**文档阶段错位**。`CURRENT_*` 和旧 `STATUS.md` 仍停留在早期原型视角，已经不能准确代表当前项目阶段；现已同步修正。
- **性能评分：** 当前阶段 **8.6/10**。优点是执行链、代理验证链、策略层、批量巡检和测试覆盖都已成型；扣分点是 verify 慢路径仍偏轻、策略 explainability 仍弱、高并发写放大治理未正式落地。
- **改进建议：** 下一步最值得做的不是继续堆新功能，而是优先完成 **trust score 再收敛 + verify 慢路径深化 + 写放大治理 + explainability 补齐**。

## Autopilot Sync

- 独立 autopilot 最近一次文档同步已完成。
## 执行引擎 / Artifact 策略

- 已新增 `EXECUTION_ENGINE_ARTIFACT_STRATEGY.md`，先以文档方式收敛执行引擎边界与 artifact 分层/保留策略。

