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
- **API Key 鉴权（可选）**

更完整进展请看：
- `PROGRESS.md` — **已实现 / 正在做 / 未来将实现**
- `STATUS.md` — **当前状态摘要、风险、下一步**

## API 鉴权

设置环境变量 `AUTO_OPEN_BROWSER_API_KEY` 后，所有接口需要携带以下任一方式：
- `x-api-key: <key>`
- `Authorization: Bearer <key>`

未设置该环境变量时，接口将不做鉴权限制。

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

## 标准接手入口

以后接手本项目，默认先看这 3 份标准文档：

- `AI.md` — 项目入口、接手顺序、关键文档映射
- `PLAN.md` — 当前计划、优先级、风险、下一步
- `FEATURES.md` — 项目最终目标功能总表

再按需下钻：`STATUS.md` / `PROGRESS.md` / `TODO.md` / `ROADMAP.md` / `VISION.md`。

---

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

## Lightpanda V1 运行说明

当前 `LightpandaRunner` 第一版通过本地二进制方式接入：

- 环境变量：`LIGHTPANDA_BIN`
- 默认命令：`lightpanda fetch <url>`
- 当前输出：回收 `stdout / stderr / exit_code / timeout` 到结果链路

如果宿主机未安装 `lightpanda`，请先安装 nightly binary，或将二进制路径写入 `LIGHTPANDA_BIN`。

## Smoke Test

已新增最小冒烟脚本：`scripts/smoke_test.sh`

默认用途：
- 检查 `health`
- 创建一个最小任务
- 轮询任务直到结束
- 拉取 `runs / logs / status`

示例：

```bash
AUTO_OPEN_BROWSER_BASE_URL=http://127.0.0.1:3000 \
AUTO_OPEN_BROWSER_RUNNER=fake \
scripts/smoke_test.sh
```

如启用了 API Key，可同时传入：

```bash
AUTO_OPEN_BROWSER_API_KEY=your-key scripts/smoke_test.sh
```

## 当前主任务

当前优先任务不是重新堆文档，而是：

- 在现有最小原型基础上继续增强控制面与观测面
- 为真实执行器接入做接口与架构预留
- 持续清理文档，确保描述和代码能力一致

详见 `CURRENT_TASK.md`。
