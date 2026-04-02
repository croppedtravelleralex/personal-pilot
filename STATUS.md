# STATUS.md

## 当前状态摘要

- **状态：** 已进入 **trust score 核心化 + verify 慢路径并入主排序 + 性能治理前置阶段**
- **日期：** 2026-04-02
- **当前焦点：** 把 **selection 中剩余的控制流语义** 和 **verify 慢路径底层风险信号** 继续统一进 **trust score / explainability 主链**，并开始为 provider/provider×region 风险聚合与 profiling 做前置收口。

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

## 当前状态

当前系统已经具备以下主线能力：

1. **执行与调度控制面**
   - DB-first queue
   - claim / reclaim / retry / cancel
   - 多 worker 并发执行
   - health / status / logs / runs 基础观测

2. **Fingerprint 控制面**
   - profile 创建 / 查询 / 绑定 / 校验
   - fake / lightpanda 统一 profile 视图
   - task/status 详情暴露 fingerprint resolution status

3. **Proxy pool / verify / trust score 主链**
   - proxy CRUD
   - provider / region / min_score / cooldown 过滤
   - sticky session 正式绑定表 `proxy_session_bindings`
   - smoke / verify / verify-batch / verify batch 查询
   - verify 结果、执行结果反哺 proxy score
   - provider / provider×region 风险快照
   - cached trust score 持久化、scan / repair / maintenance

4. **Explainability / 可观测性主链**
   - task / status / explain 接口统一暴露 `selection_reason_summary`
   - `selection_explain` 结构化输出
   - `winner_vs_runner_up_diff` 结构化输出
   - `candidate_rank_preview` 强类型化
   - `trust_score_components` 强类型化
   - `summary_artifacts` schema 标准化（source/category/severity）
   - run 级 `result_json` 持久化与 `run_id / attempt / timestamp` 溯源字段
   - `/proxies/:id/explain` 暴露 `trust_score_cached_at / explain_generated_at / explain_source`
   - explainability assembler 已从 handlers 中抽离到独立模块

5. **selection → trust score 核心化的当前成果**
   - auto 选择主链已经明确走 `trust score` 排序
   - explainability 已能输出 `trust_score_components`、候选预览与 winner-vs-runner-up 对比
   - `explicit / sticky / no-match` 已补结构化 explain 字段
   - `min_score` 已明确保持 hard gate，`soft_min_score` 已以 soft ranking penalty 形式进入 score 主链
   - provider/provider×region 风险已经进入 score 组件层表达

6. **verify 慢路径已开始真正并入排序主链**
   当前不仅 verify 接口能输出慢路径诊断，而且以下底层信号已经正式进入 trust score：
   - anonymity (`anonymity_bonus`)
   - probe latency (`latency_penalty`)
   - `exit_ip_not_public` (`exit_ip_not_public_penalty`)
   - `probe_error_category` 映射 (`probe_error_penalty`)
   - `geo mismatch` severity (`geo_mismatch_penalty`)
   - `region mismatch` severity (`region_mismatch_penalty`)

   这意味着 verify 慢路径已经从“接口诊断信息”升级为“selection 的真实排序输入”。

7. **测试与稳定性**
   - 单测 + 集成测试持续覆盖执行、代理、verify、trust score、explainability 主链
   - 当前测试状态：**41 unit + 84 integration 全绿**

## 当前风险

1. **selection 语义仍未完全统一。**
   当前 auto 主链已经走 trust score，但 explicit / sticky / cooldown / no-match fallback 仍保留一定控制流语义，后续若继续叠规则，维护成本仍可能升高。

2. **verify 风险原因已开始进入单代理 score，但 provider/provider×region 聚合还没正式吸收这些新信号。**
   这会导致单代理排序进步快于聚合风险层，后续可能需要再收一轮跨代理风险汇总策略。

3. **高并发下的 SQL / 写放大治理还没有正式做。**
   trust cache、verify 回写、status 聚合、selection explain 已经全部进入主链，后续要正式看查询成本、索引策略与写频率。

4. **profiling 现在已有最小观测埋点，但还缺真实样本。**
   当前已为 snapshot refresh / cached trust refresh / scoped refresh branch 增加 `AOB_PERF_PROBE=1` 观测埋点，但还没跑足够真实流量样本来判断热点分布。

5. **文档刚追回代码主线，仍需持续同步。**
   如果 `STATUS / TODO / PROGRESS / CURRENT_*` 不持续跟进，自动推进仍可能围绕旧阶段动作打转。

6. **Lightpanda 真实浏览器侧的更深 fingerprint 消费还没正式进入系统验证阶段。**
   当前 profile 注入主链是通的，但真实浏览器侧更深能力与性能影响仍待系统评估。

## 当前下一步

### P0
1. **继续推进 selection → trust score 核心化**，把剩余分散在 selection 中的控制流语义继续收进统一 score / explain 边界。
2. **开始评估 provider/provider×region 风险汇总是否吸收 verify 慢路径新信号**，避免单代理 score 与聚合风险层脱节。
3. **跑一轮真实场景下的 `AOB_PERF_PROBE=1` 样本观察**，量化 snapshot flip、范围刷新分布与耗时。
4. **继续清 explainability 主链里剩余 typed/JSON 边界与 summary 文案质量。**
5. **推进更真实的 verify 慢路径**，继续补匿名性 / 地区 / 出口真实性以外的可稳定质量信号。

### P1
6. 设计代理质量评分系统正式形态。
7. 设计 `SessionIdentity / ExecutionIdentity`，把 `proxy + fingerprint + region + risk_level` 收到统一表达。
8. 继续压 panic 风险点、锁竞争风险点与 flaky 测试。
9. 继续完善 API / 运维 / 能力说明文档。

## 本轮体检（2026-04-02）

- **找 bug：** 本轮真实暴露并修掉的核心 bug 不是业务逻辑错误，而是 explainability 组件标签映射未同步更新，导致新 score component 在 diff 中掉成 `unknown`；现已修复。
- **性能评分：** 当前阶段 **9.2/10**。优点是 trust score / explainability 主链已经开始真正消费 verify 慢路径信号，且 profiling 最小观测埋点已经落地；扣分点仍然是真实样本数据还未量化。
- **改进建议：** 下一步最值得做的是 **评估 provider/provider×region 风险汇总如何吸收 verify 慢路径新增信号**，避免单体排序与聚合风险策略分裂。

## Autopilot Sync

- 当前文档已对齐到 **2026-04-02 trust score 核心化 + verify 慢路径并入主排序 + 性能治理前置阶段**。
