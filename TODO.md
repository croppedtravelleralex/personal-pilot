# TODO.md

## P0

- [x] 建立项目核心文档（VISION / ROADMAP / STATUS / TODO / EXECUTION_LOG / RUN_STATE）
- [x] 建立周期执行协议（EXECUTION_PROTOCOL）
- [x] 建立自动执行内核基础文件（STATE_MACHINE / CHECKLIST / ROUND_RESULT template）
- [x] 建立轮次调度器设计（ROUND_SCHEDULER）
- [ ] 初始化 Rust 工程（Cargo）
- [ ] 设计任务数据模型（Task / Run / Artifact / Log）
- [ ] 设计 SQLite schema
- [ ] 定义 REST API 最小接口
- [ ] 实现内存任务队列
- [ ] 实现 fake runner
- [ ] 打通创建任务 -> 入队 -> 执行 -> 状态更新 -> 查询结果

## P1

- [ ] 增加任务取消 / 超时 / 重试机制
- [ ] 增加结构化日志
- [ ] 增加执行历史与审计字段
- [ ] 设计 runner trait / adapter interface
- [ ] 为 `lightpanda-io/browser` 预留适配层
- [ ] 设计浏览器指纹能力边界
- [ ] 设计高级指纹下的性能预算与性能开销控制策略
- [ ] 设计持续抓取代理的工具（优先基于开源项目改造）
- [ ] 设计代理抓取后的清洗、去重、候选入池流程
- [ ] 设计代理池与代理轮换策略
- [ ] 设计代理池自生长机制
- [ ] 设计地区感知的代理匹配策略
- [ ] 设计“所有访问强制走代理池”的网络约束
- [ ] 设计可用代理比例 40%-60% 的动态控制策略
- [ ] 设计磁盘使用监控与落盘上限策略
- [ ] 设计 artifact / log 的保留、清理与归档策略
- [ ] 设计高并发下的性能优化与写放大控制策略

## P2

- [ ] 增加并发控制
- [ ] 增加资源限制
- [ ] 增加 API 鉴权
- [ ] 增加基础监控指标
- [ ] 增加集成测试
- [ ] 设计身份画像系统（Identity Profile）
- [ ] 设计指纹一致性校验器
- [ ] 设计代理质量评分系统
- [ ] 设计站点维度代理适配机制
- [ ] 设计行为层模拟机制
- [ ] 设计会话连续性机制
- [ ] 设计策略引擎
- [ ] 设计实验记录系统

## 待讨论

- [ ] 任务结果与 artifact 的落盘策略
- [ ] 截图 / HTML / console log 的存储方式
- [ ] 多租户/多用户隔离是否是近期目标
- [ ] 是否需要 webhook / callback 通知
- [ ] `GOLDEN_FEATURES.md` 中哪些能力应前置到中期优先级
