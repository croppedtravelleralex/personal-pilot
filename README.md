# AutoOpenBrowser

高性能浏览器自动化系统，运行在 Ubuntu 上。

## 项目目标

构建一个主要供个人学习与研发使用、面向自动化任务执行的浏览器系统，当前采用：

- Rust
- SQLite
- REST API
- 内存任务队列
- fake runner

后续将接入 `lightpanda-io/browser` 作为真实浏览器执行引擎。

## 当前阶段

当前处于项目骨架与架构定义阶段。

## 关键文档

- `VISION.md` — 最终效果与最终功能定义
- `ROADMAP.md` — 过去 / 现在 / 未来的滚动路线图
- `STATUS.md` — 当前状态、风险、下一步
- `TODO.md` — 任务分层清单
- `EXECUTION_LOG.md` — 每轮执行记录
- `RUN_STATE.json` — 自动推进的轮次状态
- `AUTONOMY_PLAN.md` — 周期执行规则
- `CURRENT_DIRECTION.md` — 当前阶段方向说明
- `DESIGN_NETWORK_IDENTITY.md` — 指纹 / 代理池 / 任务网络策略设计
- `LONG_TERM_ROADMAP.md` — 中长期功能方向与演进顺序
- `GOLDEN_FEATURES.md` — 高价值金子功能建议与难度/成功率评估
- `EXECUTION_PROTOCOL.md` — 每5分钟/8小时周期执行协议
- `EXECUTION_STATE_MACHINE.md` — 自动执行状态机
- `EXECUTION_CHECKLIST.md` — 每轮执行检查清单
- `ROUND_RESULT.template.json` — 单轮结果模板
- `ROUND_SCHEDULER.md` — 轮次调度器设计

## 目录建议

- `src/` — Rust 主程序与模块
- `migrations/` — SQLite schema / 迁移
- `docs/` — 架构文档、接口说明
- `scripts/` — 开发辅助脚本
- `examples/` — 示例请求与样例任务

## 后续目标

1. 借鉴开源项目并形成参考清单
2. 完成最小可运行后端骨架
3. 打通任务创建 / 入队 / 执行 / 状态更新链路
4. 用 fake runner 跑通端到端流程
5. 接入真实浏览器引擎
6. 补齐观测、重试、资源隔离与稳定性能力

## 当前主任务

当前优先任务不是直接堆实现，而是：

- 借鉴开源项目
- 完善工程文档
- 再根据工程文档推进 app

详见 `CURRENT_TASK.md`。
