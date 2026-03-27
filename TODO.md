# TODO.md

## P0

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

## P1

- [x] 增加任务取消机制
- [x] 增加任务重试机制
- [x] 增加任务超时模拟分支
- [x] 增加结构化日志
- [x] 增加执行历史与审计字段
- [x] 设计 runner trait / adapter interface
- [x] 为 `lightpanda-io/browser` 预留适配层
- [x] 抽离 runner 通用执行层（第一轮）
- [x] 让 RunnerTask 接入真实任务输入（第一版）
- [x] 接入 `LightpandaRunner` 最小真实执行第一版（`LIGHTPANDA_BIN` + `fetch`）
- [x] 收紧 `LightpandaRunner` 结果结构与错误语义
- [x] 增加 `status / runs / logs` 查询控制与分页（第一版）
- [x] 增加 `status / runs / logs` 的 `offset` 分页（第二版）
- [x] 设计 `running cancel` 的正确演进边界
- [x] 增加 runner cancel 抽象层（第一版）
- [x] 为 `LightpandaRunner` 增加取消句柄注册表（第一版）
- [x] 让 `LightpandaRunner` 尝试终止运行中外部进程（第一版）
- [x] 让 AppState 持有当前 runner 句柄
- [x] 打通 API 层 `running cancel` 第一版接线
- [x] 增加 running cancel 状态竞争保护（第一版）
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
- [x] 增加数据库目录自动创建
- [ ] 设计高并发下的性能优化与写放大控制策略

## P2

- [ ] 增加并发控制
- [ ] 增加资源限制
- [x] 增加 API 鉴权
- [x] 增加运行历史与日志查询接口
- [x] 清理 README / STATUS / PROGRESS 中过时静态状态
- [x] 增加基础监控指标
- [ ] 增加集成测试
- [x] 增加最小 smoke test 脚本
- [x] 增加 lightpanda 专项验证脚本入口
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
