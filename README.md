# AutoOpenBrowser

高性能浏览器自动化系统，运行在 Ubuntu 上。

## 项目摘要

当前项目已经完成 **最小可运行原型**：
- **任务创建 / 查询**
- **SQLite 持久化**
- **内存任务队列**
- **fake runner 执行**
- **success / fail / timeout 分支**
- **重试 / 取消（queued）**
- **run history / logs 记录与查询**
- **health / status 摘要输出**

更完整进展请看：
- `PROGRESS.md` — **已实现 / 正在做 / 未来将实现**
- `STATUS.md` — **当前状态摘要、风险、下一步**

## 项目目标

构建一个主要供个人学习与研发使用、面向自动化任务执行的浏览器系统，当前采用：

- Rust
- SQLite
- REST API
- 内存任务队列
- fake runner

后续将接入 `lightpanda-io/browser` 作为真实浏览器执行引擎。

## 当前阶段

当前已从“文档驱动 + 工程骨架阶段”推进到：

> **最小后端原型已跑通，正在向更完整的控制面、观测面和真实执行器演进。**

## 关键文档

- `PROGRESS.md` — 已实现 / 正在做 / 未来将实现的统一进展文档
- `STATUS.md` — 当前状态、风险、下一步
- `VISION.md` — 最终效果与最终功能定义
- `ROADMAP.md` — 过去 / 现在 / 未来的滚动路线图
- `TODO.md` — 任务分层清单
- `EXECUTION_LOG.md` — 每轮执行记录
- `RUN_STATE.json` — 自动推进的轮次状态
- `AUTONOMY_PLAN.md` — 周期执行规则
- `CURRENT_DIRECTION.md` — 当前阶段方向说明
- `DESIGN_NETWORK_IDENTITY.md` — 指纹 / 代理池 / 任务网络策略设计
- `LONG_TERM_ROADMAP.md` — 中长期功能方向与演进顺序
- `GOLDEN_FEATURES.md` — 高价值功能建议与难度/成功率评估
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

1. 增强 API 鉴权与控制面完整性
2. 增强 `runs / logs / status` 的可观测性与分页能力
3. 为真实浏览器执行器接入预留 runner adapter
4. 补齐更完整的失败恢复、取消控制与稳定性策略
5. 推进真实执行器 `lightpanda-io/browser` 集成

## 当前主任务

当前优先任务不是重新堆文档，而是：

- 在现有最小原型基础上继续增强控制面与观测面
- 为真实执行器接入做接口与架构预留
- 持续清理文档，确保描述和代码能力一致

详见 `CURRENT_TASK.md`。
