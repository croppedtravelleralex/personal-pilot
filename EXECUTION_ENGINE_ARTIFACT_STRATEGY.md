# EXECUTION_ENGINE_ARTIFACT_STRATEGY

## 目标

为 `PersonaPilot` 明确一条低风险、可演进的 **执行引擎边界 + artifact 策略**，避免 runner / artifact / 持久化职责继续散落，影响后续 trust score、verify 和长期运行治理。

## 当前判断

项目已经具备：
- fake runner
- lightpanda runner
- 任务 / 运行 / artifact / log 的基础表达
- verify / smoke / batch verify / 巡检链路

但当前还缺：
- 执行引擎边界的统一口径
- artifact 分层与落盘策略
- 长期运行下 artifact / log / screenshot / html 的保留、清理、归档原则
- 对 verify 结果、runner 输出、调试产物的统一抽象

## 建议边界

### 1. Engine 层只负责“执行”

`runner engine` 的职责应收敛为：
- 启动执行
- 返回运行结果
- 返回结构化原始产物引用
- 不直接负责长期存储策略

换句话说：
> engine 产出 artifact reference，不决定最终 retention policy。

### 2. Artifact 层负责“结果分层”

建议至少分成：
- **result artifact**：业务核心结果（页面结果、verify 结果、提取数据）
- **debug artifact**：截图、html、console、network dump
- **transient artifact**：临时调试产物，可被短期清理
- **summary artifact**：给 status / explainability / report 用的轻摘要

### 3. Retention 层负责“保留策略”

建议后续单独抽出 retention policy：
- smoke 成功：只保留 summary
- verify 失败：保留 debug + summary
- batch verify：按采样比例保留 debug，完整保留 summary
- 巡检：优先保留 summary，按异常升级保留 debug

## 当前低风险落地方向

这阶段**不建议**直接大改 runner 主链，先做：
1. 明确 engine / artifact / retention 三层边界
2. 统一命名与分类口径
3. 给后续 STATUS / explainability / metrics 留出稳定字段
4. 避免后续 trust score / verify 深化时，artifact 继续成为散乱附属物

## 后续动作建议

### P0
- 定义 artifact taxonomy（result / debug / transient / summary）
- 给 runner 输出补统一 artifact envelope
- 定义最小 retention rule 文档

### P1
- 给 verify / batch verify / 巡检统一 summary artifact
- 设计 screenshot/html/console 的按异常升级保留策略
- 给 status / explainability 暴露 artifact 统计指标

### P2
- 设计归档与清理任务
- 设计磁盘预算与自动清理上限
- 设计异常 run 的长期保留策略

## 当前结论

当前最稳的推进方式不是直接改 engine 主代码，而是：
> **先把执行引擎边界与 artifact 策略文档化，再用这份文档约束下一轮实现。**
