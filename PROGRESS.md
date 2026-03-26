# PROGRESS.md

`lightpanda-automation` 项目进展记录。

目标：用一份简洁文档持续说明三件事：
- **已经实现了什么**
- **现在正在做什么**
- **后面将要实现什么**

---

## 1. 已经实现 / 已经落地

### 1.1 项目方向与北极星已定义
- 已明确项目目标：构建一个运行在 Ubuntu 上的高性能浏览器自动化系统。
- 已明确早期技术路线：`Rust + SQLite + REST API + 内存任务队列 + fake runner`。
- 已明确后续真实执行引擎方向：`lightpanda-io/browser`。

### 1.2 文档体系已建立
已建立并持续维护以下核心文档：
- `README.md`
- `STATUS.md`
- `TODO.md`
- `ROADMAP.md`
- `VISION.md`
- `CURRENT_TASK.md`
- `CURRENT_DIRECTION.md`
- `AUTONOMY_PLAN.md`
- `EXECUTION_PROTOCOL.md`
- `EXECUTION_STATE_MACHINE.md`
- `EXECUTION_CHECKLIST.md`
- `ROUND_SCHEDULER.md`
- `DESIGN_NETWORK_IDENTITY.md`
- `MODULE_SCOPE.md`
- `SCHEMA_SCOPE.md`
- `LONG_TERM_ROADMAP.md`
- `GOLDEN_FEATURES.md`
- `EXECUTION_LOG.md`
- `RUN_STATE.json`

### 1.3 自动推进框架已初步落地
- 已建立轮次执行机制（plan / build / verify / summarize）。
- 已建立执行日志记录机制。
- 已建立阶段汇总机制。
- 已建立运行状态文件 `RUN_STATE.json`。
- 已建立调度器设计与轮次状态字段。

### 1.4 Rust 工程骨架已初始化
已落地基础 Rust 工程文件与模块骨架：
- `Cargo.toml`
- `src/main.rs`
- `src/lib.rs`
- `src/api/`
- `src/db/`
- `src/domain/`
- `src/queue/`
- `src/runner/`
- `src/network_identity/`

### 1.5 SQLite schema 与核心数据模型草案已落地
已补齐首批数据库与领域层骨架：
- `src/db/schema.rs`
- `src/db/models.rs`
- `src/domain/task.rs`
- `src/domain/run.rs`
- `src/domain/state_machine.rs`

当前已定义最小核心对象：
- `tasks`
- `runs`
- `artifacts`
- `logs`
- `TaskStatus` 状态流转规则

### 1.6 最小 REST API 已落地
已新增最小 API 能力：
- `GET /health`
- `GET /status`
- `POST /tasks`
- `GET /tasks/:id`
- `POST /tasks/:id/retry`
- `POST /tasks/:id/cancel`
- `GET /tasks/:id/runs`
- `GET /tasks/:id/logs`

### 1.7 数据库初始化与目录自创建已落地
已支持：
- 启动时初始化 SQLite 连接池
- 启动时执行首批 schema SQL
- 启动前自动创建 SQLite 父目录
- 将数据库连接注入应用状态

### 1.8 内存任务队列已落地
已支持：
- 创建任务后自动入队
- 队列长度统计
- 队列内任务移除（支持 queued cancel）

### 1.9 fake runner 第一版已落地
已支持：
- 后台循环消费内存队列
- success / fail / timeout 三种模拟结果
- 任务状态回写：`queued -> running -> succeeded/failed/timeout`
- `started_at / finished_at / result_json / error_message` 回写

### 1.10 run history 已落地
已支持：
- `runs` 表写入最小执行历史
- `attempt` 按运行次数自动递增
- 执行结果关联 `run_id`

### 1.11 logs 已落地
已支持：
- 关键执行节点写入 `logs` 表
- 记录 `task_id / run_id / level / message / created_at`
- success / fail / timeout 分别写入不同级别日志

### 1.12 重试机制第一版已落地
已支持：
- 对 `failed / timeout` 任务执行重试
- 重试后重新置为 `queued`
- 重新入队并再次执行

### 1.13 取消机制第一版已落地
已支持：
- 对 `queued` 任务执行取消
- 从内存队列中移除任务
- 任务状态更新为 `cancelled`

### 1.14 健康与状态汇总能力已落地
已支持：
- `GET /health` 返回：
  - 队列长度
  - 任务状态统计
- `GET /status` 返回：
  - 队列长度
  - 任务状态统计
  - 最近 5 条任务摘要

### 1.15 执行明细查询能力已落地
已支持：
- `GET /tasks/:id/runs`
- `GET /tasks/:id/logs`
- 可直接查看指定任务的运行历史与执行日志

### 1.16 长期设计方向已明确
已明确以下关键方向，并沉淀到文档：
- 任务生命周期管理
- fake runner / real runner 统一抽象
- SQLite 持久化
- 最小 REST API
- 高级浏览器指纹能力
- 代理池
- 所有访问强制走代理池
- 代理地区匹配
- 可用代理比例维持在 40%-60%
- 代理池自生长
- 持续抓取代理工具
- 日志、artifact、验证与阶段汇总机制

---

## 2. 正在做 / 当前重点

### 2.1 当前所处阶段
当前处于：
- **最小可运行原型已跑通阶段**
- **控制面与观测面增强阶段**
- **为真实执行器接入做准备阶段**

### 2.2 当前正在推进的主题
当前重点不是重新补骨架，而是继续补齐这些增强项：
- API 鉴权
- 查询分页 / limit / 控量
- 更完整的任务控制（尤其是 running cancel）
- fake runner 到 real runner 的 adapter 预留
- 文档与代码能力持续对齐

---

## 3. 尚未完成但明确要做的功能

### 3.1 控制面增强
- API 鉴权
- running cancel 设计与实现
- 更完整的重试策略（如上限、退避、策略控制）

### 3.2 观测面增强
- `runs / logs` 分页与 limit 控制
- 更细粒度的统计查询
- 更丰富的 service status 输出

### 3.3 执行层增强
- runner trait / adapter interface
- real runner adapter
- 对 `lightpanda-io/browser` 的接入准备

### 3.4 网络与身份层
- 指纹模板 / 策略模型落地
- 代理抓取后的清洗、验证、候选入池联动
- 代理分配与轮换策略落地
- 代理失败剔除机制
- 地区代理基础存量维持机制

### 3.5 稳定性与工程化
- 更完整的错误分类
- smoke test / 集成测试
- artifact / log 的保留、清理与归档策略
- 高并发下的性能与写放大控制

---

## 4. 未来将要实现的功能

### 4.1 中期目标
- 将 fake runner 原型升级为更真实的执行框架
- 建立更完整的任务控制面和运维观测面
- 为真实浏览器执行器接入提供稳定边界

### 4.2 后期目标
- 接入真实浏览器执行引擎 `lightpanda-io/browser`
- 实现 fake runner → real runner 平滑切换
- 完善高并发下的性能控制
- 完善代理质量评分与调度
- 完善身份画像与指纹一致性能力
- 完善会话连续性与行为层模拟

### 4.3 长期演进方向
- 身份画像系统
- 指纹一致性校验器
- 代理质量评分系统
- 站点维度代理适配
- 行为层模拟
- 会话连续性系统
- 策略引擎
- 实验记录系统

---

## 5. 当前一句话总结

当前项目**已经完成最小可运行原型**：具备任务创建、排队、执行、状态流转、重试、取消、运行历史、执行日志和状态摘要能力；下一阶段重点是**安全性、可观测性增强以及真实执行器接入准备**。

---

## 6. 维护规则

后续每次推进时，优先同步更新：
- **已实现**：只有真正落地的功能才能写进来
- **正在做**：只写当前阶段真实推进重点
- **未来将实现**：只保留中长期确定方向，避免空泛堆砌
