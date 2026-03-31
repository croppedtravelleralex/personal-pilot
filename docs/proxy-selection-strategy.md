# Proxy Selection Strategy

## 文档目的

这份文档用来说明：

- 系统当前是怎么选代理的
- 为什么不再只是看 `score`
- 巡检结果是怎么反哺选择逻辑的
- 当前阶段哪些能力已经落地，哪些还没做

---

## 一句话结论

当前系统的代理选择逻辑，已经从：

**“谁分数高就先选谁”**

推进成：

**“优先选择更新鲜、更可靠、地理更匹配、最近没翻车的代理；分数只是最后一层参考。”**

---

## 当前选择逻辑（V1）

### 第 1 层：硬条件过滤
先过滤掉根本不应该进入候选池的代理：

- 代理状态必须是 `active`
- 代理不能处于 `cooldown`
- 如果任务指定了 `provider`，必须匹配
- 如果任务指定了 `region`，必须匹配
- 如果任务指定了 `min_score`，必须满足最低分数

这一步的目标很简单：

**先把明显不能用的代理排除掉。**

---

### 第 2 层：强优先信号
在可用代理里，系统会优先看这些更强的可靠性信号：

- `last_verify_status = ok`
- `last_verify_geo_match_ok = true`
- `last_smoke_upstream_ok = true`

这意味着：

- 已经验证通过的代理，优先级更高
- 地理匹配正确的代理，优先级更高
- 至少 smoke upstream 正常的代理，优先级更高

这一步的目标是：

**让“验证过、验证对、链路通”的代理优先被选中。**

---

### 第 3 层：风险惩罚
即使某个代理分数高，只要它风险更大，也会被往后排：

- `last_verify_status = failed` → 明显后排
- `last_verify_at IS NULL` → 后排
- `last_verify_at` 过旧 → 后排

当前 stale 判定口径：

- 以 **3600 秒** 作为当前实现里的基础过旧阈值

这一步的目标是：

**避免“高分但不新鲜 / 高分但刚翻车”的代理抢走优先级。**

---

### 第 4 层：资源均衡与兜底排序
在前面都差不多的情况下，系统最后才看：

- `score DESC`
- `last_used_at ASC`
- `created_at ASC`

这表示：

- `score` 仍然重要，但已经不是第一优先级
- 最近没被用过的代理，会更容易被轮到
- 更早进入系统的代理，在其他条件相近时更容易被先尝试

这一步的目标是：

**在“可靠性优先”的前提下，再做分数与资源均衡。**

---

## 当前系统已经验证过的核心口径

当前测试已经覆盖并锁住以下关键选择规则：

1. **fresh verified 优先于 stale high score**
2. **verify ok 优先于 recent verify failed**
3. **geo match verified 优先于 smoke-only**
4. **fresh verified 优先于 missing verify**

这意味着当前 selection V1 不是拍脑袋规则，而是已经有回归保护。

---

## 为什么这一步重要

如果巡检结果只是“能看见”，价值只做了一半。

真正值钱的是：

- 巡检发现哪个代理更可靠
- 系统在真正选代理时就更偏向它
- 巡检发现哪个代理最近失败了
- 系统就自动把它往后排

也就是说，当前已经开始形成这条闭环：

**巡检 → 判断 → 选择**

这也是代理系统从“观测功能”走向“调度能力”的关键一步。

---

## 当前还没做的部分

虽然闭环已经成立，但当前还不是最终形态。

后续还可以继续推进：

- 把 stale 阈值做成更明确的策略参数，而不是写死口径
- 把 selection 规则从 SQL 排序进一步抽成更清楚的策略层
- 把 provider 级调度均衡进一步做强
- 把更多巡检历史结果纳入长期权重
- 让批量巡检结果更直接影响后续调度周期

---

## 当前阶段定位

当前可以把这套能力定位为：

**Proxy Selection Strategy V1**

特点是：

- 已经不只看 `score`
- 已经开始显式吃巡检结果
- 已经具备最小的可靠性优先逻辑
- 已经有关键回归保护

这说明系统已经开始从：

**“有代理池”**

走向：

**“会挑更靠谱代理的代理系统”**


## 策略层抽象进展

当前代码里已经开始把代理选择规则提炼成独立概念：

- `ProxySelectionTier`
- `ProxySelectionRule`
- `current_proxy_selection_rules()`
- `proxy_selection_order_sql()`

这意味着 selection 逻辑不再只是“埋在查询里的隐式规则”，而是开始有了可单独描述、可继续扩展的策略层骨架。


### 当前已抽出的策略层骨架

当前策略层已经不只是暴露排序 SQL，还开始暴露基础筛选口径：

- `proxy_selection_base_where_sql()`
- `proxy_selection_order_sql()`

这表示 selection 的“硬过滤”和“排序优先级”已经开始从 engine 中拆分出来。


## 当前阶段小结

当前可以把这部分能力定位为：

**Proxy Selection Strategy Layer V1**

当前已经收拢到策略模块中的内容包括：

- 规则分层概念（tier）
- 规则清单（rule）
- 基础筛选口径（base where）
- 排序优先级口径（order sql）
- sticky / resolved_sticky / unresolved 解析状态口径
- `resolved_proxy` 的结果表达

这说明 selection 已经开始从“engine 里的一组实现细节”，走向“可维护、可测试、可继续扩展的独立策略层”。


## 长期历史权重（当前已落地）

当前 selection 已开始纳入最小可行的长期历史权重口径：

- 当 `failure_count >= success_count + 3` 时，代理会被更明显地后排
- 当 `failure_count > success_count` 时，代理会被轻度后排
- 其他情况暂不额外惩罚

这意味着系统不再只看“最近一次看起来好不好”，也开始看“这条代理长期是不是经常翻车”。


## Provider 长期稳定性（当前已落地）

当前 selection 已开始纳入 provider 级长期稳定性口径：

- 如果某个 provider 下面全部代理累计后满足 `SUM(failure_count) >= SUM(success_count) + 5`
- 则该 provider 会整体被后排

这意味着系统不只会判断“单条代理长期稳不稳”，也开始判断“这家 provider 整体长期靠不靠谱”。


## 时间衰减型近期失败惩罚（当前已落地）

当前 selection 已开始纳入最小可行的“时间衰减型近期失败惩罚”口径：

- 若 `last_verify_status = failed` 且最近 **1800 秒**内失败 → 更重后排
- 若 `last_verify_status = failed` 且最近 **7200 秒**内失败 → 较轻后排
- 更早的失败暂不吃这层近期惩罚

这意味着系统不再只看“累计失败过多少次”，也开始看“失败是不是刚发生、风险是不是还很新鲜”。


## Provider / Region 近期失败聚簇惩罚（当前已落地）

当前 selection 已开始纳入 provider × region 维度的近期失败聚簇口径：

- 若同一个 `provider + region` 组合下
- 最近 **3600 秒** 内
- 有 **2 条及以上代理** 出现 `last_verify_status = failed`
- 则该 `provider + region` 组合会被整体后排

这意味着系统不只会看单条代理是否失败、provider 是否长期不稳，也开始识别“某个 provider 在某个 region 最近是不是正在局部翻车”。


## 参数化起点

当前策略层已经开始提供一组默认调优参数：

- `stale_after_seconds`
- `recent_failure_heavy_window_seconds`
- `recent_failure_light_window_seconds`
- `provider_failure_margin`
- `provider_region_failure_cluster_window_seconds`
- `provider_region_failure_cluster_count`

当前这些参数还主要作为**默认策略口径**存在，但这一步意味着策略层已经开始从“纯硬编码规则”往“可调参数规则”迈出第一步。


## 参数化第一版（当前状态）

当前策略层已经完成参数化的第一步：

- 有明确的默认调优参数结构 `ProxySelectionTuning`
- `proxy_selection_order_sql()` 已转为模板化口径
- 执行链已经开始通过 tuning 参数生成实际选择规则

当前这一步的意义不在于已经完全开放配置，而在于：

**策略阈值已经开始从“散落的硬编码常量”变成“有明确参数面的规则系统”。**


## 环境变量覆盖（当前已支持）

当前默认 tuning 已支持通过环境变量做覆盖：

- `AOB_PROXY_STALE_AFTER_SECONDS`
- `AOB_PROXY_RECENT_FAILURE_HEAVY_WINDOW_SECONDS`
- `AOB_PROXY_RECENT_FAILURE_LIGHT_WINDOW_SECONDS`
- `AOB_PROXY_PROVIDER_FAILURE_MARGIN`
- `AOB_PROXY_PROVIDER_REGION_CLUSTER_WINDOW_SECONDS`
- `AOB_PROXY_PROVIDER_REGION_CLUSTER_COUNT`
- `AOB_PROXY_RAW_SCORE_WEIGHT_TENTHS`
- `AOB_PROXY_VERIFY_OK_BONUS`
- `AOB_PROXY_VERIFY_GEO_MATCH_BONUS`
- `AOB_PROXY_SMOKE_UPSTREAM_OK_BONUS`
- `AOB_PROXY_VERIFY_FAILED_HEAVY_PENALTY`
- `AOB_PROXY_VERIFY_FAILED_LIGHT_PENALTY`
- `AOB_PROXY_VERIFY_FAILED_BASE_PENALTY`
- `AOB_PROXY_MISSING_VERIFY_PENALTY`
- `AOB_PROXY_STALE_VERIFY_PENALTY`

这意味着策略层已经不只是在代码里持有默认参数，也开始具备最小可行的“可切换 / 可注入”入口。


## Trust Score 起点

当前策略层已经开始具备统一 trust score 的最小起点：

- `proxy_trust_score_sql_with_tuning()`

当前它还没有完全接管 selection 排序，但已经把多条正负信号收敛成单一分值表达的第一版骨架，为后续往统一 trust/risk score 模型推进做准备。
