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

2. **provider/provider×region 聚合虽已开始吸收 verify 新信号，但聚合范围与 refresh 代价还需继续收敛。**
   当前 provider risk snapshot 已吸收 `exit_ip_not_public` 群体命中，provider×region risk snapshot 已吸收 `region_mismatch` 群体命中，但后续仍需继续验证聚合收益与范围刷新成本的平衡。

3. **高并发下的 SQL / 写放大治理还没有正式做。**
   trust cache、verify 回写、status 聚合、selection explain 已经全部进入主链，且 profiling 样本显示范围刷新分支占比不低，后续要正式看查询成本、索引策略与写频率。

4. **profiling 已有第一批真实样本，当前热点仍偏写侧范围刷新。**
   当前已为 snapshot refresh / cached trust refresh / scoped refresh branch、`/status` 与 `/proxies/:id/explain` 增加 `AOB_PERF_PROBE=1` 观测埋点，并拿到第一批样本：范围刷新分支命中占比约 `57.1%`，其中 `provider_scope_flip` 是当前主导项；读侧 `/status` 约 `1ms`、`/proxies/:id/explain` 在 `candidate_count=1~3` 时约 `3~6ms`，当前仍明显轻于写侧范围刷新。

5. **文档刚追回代码主线，仍需持续同步。**
   如果 `STATUS / TODO / PROGRESS / CURRENT_*` 不持续跟进，自动推进仍可能围绕旧阶段动作打转。

6. **Lightpanda 真实浏览器侧的更深 fingerprint 消费还没正式进入系统验证阶段。**
   当前 profile 注入主链是通的，但真实浏览器侧更深能力与性能影响仍待系统评估。

## 当前下一步

### P0
1. **继续推进 selection → trust score 核心化**，把剩余分散在 selection 中的控制流语义继续收进统一 score / explain 边界。
2. **继续扩大真实任务流样本，验证 `provider_scope_flip / provider_region_scope_flip / proxy_only_no_flip` 的命中比例是否稳定。**
3. **推进绝对指纹优先第一批真实实现**，当前已继续完成性能/并发预算可观测性补齐：status 已暴露 fingerprint medium/heavy 并发上限，selection explain 已带出预算字段，便于后续调参与回归验证。
4. **继续清 explainability 主链里剩余 typed/JSON 边界与 summary 文案质量。**
5. **推进更真实的 verify 慢路径**，继续补匿名性 / 地区 / 出口真实性以外的可稳定质量信号。

### P1
6. 设计代理质量评分系统正式形态。
7. 设计 `SessionIdentity / ExecutionIdentity`，把 `proxy + fingerprint + region + risk_level` 收到统一表达。
8. 继续压 panic 风险点、锁竞争风险点与 flaky 测试。
9. 继续完善 API / 运维 / 能力说明文档。

## 本轮体检（2026-04-02）

- **找 bug：** 本轮没有新增业务逻辑 bug；profiling 样本反而确认了两个真实热点事实：`provider_scope_flip` 已在 verify/open_page/batch verify 真执行链中真实命中，且范围刷新分支在当前样本中占比约 `57.1%`。
- **性能评分：** 当前阶段 **9.4/10**。优点是 trust score / explainability 主链已经开始真正消费 verify 慢路径信号，profiling 最小观测埋点已经落地且已有第一批真实样本；扣分点主要转移到读取侧观测尚未补齐。
- **改进建议：** 下一步最值得做的是 **把 fingerprint consistency / budget 决策进一步接入 task result / explain summary，并补更强的并发预算行为回归测试**。

## Autopilot Sync

- 当前文档已对齐到 **2026-04-02 trust score 核心化 + verify 慢路径并入主排序 + 性能治理前置阶段**。


- **阶段冻结边界：** providerRegion 继续冻结；selection redesign 继续冻结；广义 trust 语义扩张继续冻结。
