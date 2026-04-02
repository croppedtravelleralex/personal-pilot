# TODO.md

## P0

- [x] 文档化执行引擎边界与 artifact 策略第一版
- [ ] 按 `AUTONOMY_PLAN.md` 持续执行：每轮 3–5 建议、默认执行前两项、周期性进入查 bug / 修 bug 环
- [x] 建立项目核心文档（VISION / ROADMAP / STATUS / TODO / EXECUTION_LOG / RUN_STATE）
- [x] 建立周期执行协议（EXECUTION_PROTOCOL）
- [x] 建立自动执行内核基础文件（STATE_MACHINE / CHECKLIST / ROUND_RESULT template）
- [x] 建立轮次调度器设计（ROUND_SCHEDULER）
- [x] 初始化 Rust 工程（Cargo）
- [x] 设计任务数据模型（Task / Run / Artifact / Log）
- [x] 设计 SQLite schema
- [x] 定义 REST API 最小接口
- [x] 实现内存任务队列
- [x] 实现 fake runner
- [x] 打通创建任务 -> 入队 -> 执行 -> 状态更新 -> 查询结果
- [x] 打通创建任务 -> 查询任务 的最小闭环
- [x] 增加数据库初始化入口
- [x] 增加数据库目录自动创建
- [x] 设计下一步 queue claim / durable queue 方案（DB-first claim 第一版）
- [x] 落地 runner_id / stale-running reclaim 最小实现
- [x] 落地 heartbeat_at / lease-style reclaim 判定最小实现
- [x] 修复 DB-first claim 后 queue_len / queued cancel 的内存队列漂移问题
- [x] 将内存队列降级为兼容层，不再参与真实调度语义
- [x] 增加并发运行态可观测性第一版（status 暴露 worker / queue mode / reclaim）
- [x] 增加 API 鉴权
- [x] 增加运行历史与日志查询接口
- [x] 增加基础监控指标
- [x] 增加集成测试
- [x] 增加集成测试骨架第一版（fake runner + retry）
- [x] 增加最小 smoke test 脚本
- [x] 增加 lightpanda 专项验证脚本入口
- [x] 增加并发控制第一版骨架（多 worker + 并发度配置）
- [x] 增加最小一致性保护第一轮（retry 防重 + cancel 保护 run）
- [x] 加真实代理连通性/烟雾测试能力
- [x] 收口环境变量与状态暴露文档
- [x] 设计浏览器指纹能力边界
- [x] 设计指纹 profile schema 与任务绑定字段第一版
- [x] 设计指纹 profile 一致性校验器第一版
- [x] 增加 fingerprint profile 最小管理接口第一版
- [x] 为 runner 增加 fingerprint profile 注入入口第一版
- [x] 增加代理池基础能力（创建 / 查询 / 筛选 / 任务绑定）
- [x] 增加代理健康状态回写功能
- [x] 增加 sticky session 正式绑定表与复用链路
- [x] 增加 HTTP 代理协议层 smoke test
- [x] 增加 verify_proxy task kind
- [x] 增加 `POST /proxies/verify-batch`
- [x] 增加 verify batch 查询接口
- [x] 增加代理选择策略层第一版
- [x] 增加 provider / region / 历史成功失败 / 近期失败衰减 / provider×region 风险等选择信号
- [x] 增加 `ProxySelectionTuning` 默认结构与环境变量注入入口
- [x] 增加 trust score 起点与主链接入
- [x] 增加 explainability traceability 元数据（`run_id / attempt / timestamp / explain_source / explain_generated_at`）
- [x] 修复 `get_task_runs` 误复用 task 结果的问题，改为读取 run 自身结果
- [x] 标准化 `summary_artifacts` schema（source / category / severity / trace metadata）
- [x] 强类型化 `candidate_rank_preview`
- [x] 抽离 explainability assembler 到独立模块
- [x] 强类型化 `trust_score_components`
- [x] 给 `src/api/explainability.rs` 补独立 unit tests
- [x] 给 `src/runner/engine.rs` 的 explainability 辅助逻辑补独立 unit tests
- [x] 做一轮 explainability 主链剩余 loose JSON 普查与收口计划
- [ ] 继续推进 trust score 核心化，减少分散排序项依赖
- [x] 为 explicit / sticky / no-match 增加结构化 explain 字段
- [x] 固化 eligibility gate vs ranking score 的正式边界
- [x] 将 `soft_min_score` 作为 soft ranking penalty 并入 trust score
- [x] 将 verify 慢路径底层风险信号并入 trust score（anonymity / latency / exit-ip / probe-error / geo-mismatch / region-mismatch）
- [x] 评估 provider/provider×region 风险汇总是否吸收 verify 慢路径新信号
- [ ] 继续收窄 trust cache / risk snapshot refresh 范围
- [x] 为 selection / trust refresh / snapshot flip 增加最小 perf probe 埋点
- [x] 跑一轮 selection / trust cache / verify 回写 profiling 样本记录
- [x] 给 `/status` 与 `/proxies/:id/explain` 增加最小读取侧观测
- [ ] 继续扩大真实任务流样本，验证 `provider_scope_flip / provider_region_scope_flip / proxy_only_no_flip` 命中比例是否稳定
- [x] 推进 provider 级 refresh 范围收窄方案设计（优先研究 `provider risk version / dirty 标记 + 懒刷新`，且第一阶段只落 provider risk，不与 provider_region 一起上）
- [x] 继续验证 provider risk version / seen 第一版在真实链路中的收益
- [ ] 再补少量 providerScope 真实链路样本作为阶段收口
- [x] 当前阶段决定继续延后 provider_region 扩面
- [x] 评估 selection 是否需要继续消费 provider-risk version 语义
- [x] 评估 explain 是否需要继续消费 provider-risk version 语义
- [x] 若需要新增 version 语义消费者，优先设计 explain 可见性字段
- [x] 决定 explain 可见性字段进入下一轮最小实现
- [ ] 补 explain 可见性字段的最小文案与使用边界
- [ ] 定义 provider_region 进入实现阶段的条件
- [ ] 继续清 explainability summary / artifact 文案质量
- [ ] 推进更真实的 verify 慢路径（匿名性 / 地区 / 出口真实性之外的稳定质量信号）
- [x] 设计高并发下的性能优化与写放大控制策略
- [x] 收口 verify / runner 后的 scoped trust refresh，减少重复 refresh 写放大
- [x] 抽取 trust cache `UPDATE proxies SET cached_trust_score = ...` 公共 SQL 模板
- [ ] 设计高级指纹下的性能预算与性能开销控制策略
- [ ] 设计磁盘使用监控与落盘上限策略
- [ ] 记录 selection / status / trust cache / verify 回写链的 explain-level profiling 结果
- [ ] 设计 artifact / log 的保留、清理与归档策略

## P1

- [ ] 设计身份画像系统（Identity Profile）
- [ ] 设计 SessionIdentity / ExecutionIdentity，把 `proxy + fingerprint + region + risk_level` 收到统一表达
- [ ] 设计站点维度代理适配机制
- [ ] 设计行为层模拟机制
- [ ] 设计会话连续性机制
- [ ] 设计策略引擎正式形态
- [ ] 设计实验记录系统
- [ ] 增加 selection / verify / batch verify 的 metrics 与 explainability 深化输出
- [ ] 压测 proxy selection 查询、status 聚合 SQL 与 verify 批次链路
- [ ] 继续清理 panic 风险点、锁竞争风险点与 flaky 测试
- [ ] 继续完善 API / 运维 / 能力说明文档
- [ ] 设计持续抓取代理的工具（优先基于开源项目改造）
- [ ] 设计代理抓取后的清洗、去重、候选入池流程
- [ ] 设计代理池自生长机制
- [ ] 设计地区感知的代理匹配策略
- [ ] 设计“所有访问强制走代理池”的网络约束
- [ ] 设计可用代理比例 40%-60% 的动态控制策略

## P2

- [ ] 增加更正式的并发控制与资源限制
- [ ] 评估 LightpandaRunner 对更真实 fingerprint 消费的落地边界
- [ ] 设计多租户/多用户隔离是否需要前置
- [ ] 设计 webhook / callback 通知是否纳入近期目标
- [ ] 评估 `GOLDEN_FEATURES.md` 中哪些能力应前置到中期优先级

## 待讨论

- [ ] 任务结果与 artifact 的落盘策略
- [ ] 截图 / HTML / console log 的存储方式
- [ ] 真实浏览器执行结果与 proxy quality 信号如何更紧耦合
- [ ] trust score 与未来 risk score / policy engine 的边界
- [ ] Identity Profile 与 fingerprint profile 的职责切分
