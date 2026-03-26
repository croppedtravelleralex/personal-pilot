# PROGRESS.md

`lightpanda-automation` 项目进展记录。

目标：用一份简洁文档持续说明三件事：
- 已经实现了什么
- 现在正在做什么
- 后面将要实现什么

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

### 1.5 长期设计方向已明确
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
- 文档驱动阶段
- 工程骨架阶段
- 周期执行协议已落地后的继续推进阶段

### 2.2 当前正在推进的主题
当前重点不是堆业务功能，而是继续补齐以下基础：
- 自动执行内核
- 轮次调度器
- mini-cycle 试运行
- SQLite schema 细化
- fingerprint / proxy / validation / allocation 相关数据模型

### 2.3 当前轮次状态（按现有运行状态）
- 当前轮次：`Round 78`
- 当前状态：`plan completed`
- 下一轮类型：`build`
- 当前下一步重点：细化 SQLite schema 草案，并继续为自动推进内核提供更具体的落地方向

---

## 3. 尚未完成但明确要做的功能

### 3.1 核心后端闭环
- 任务创建
- 任务入队
- 任务执行
- 状态更新
- 结果查询
- 取消 / 重试

### 3.2 数据层
- Task / Run / Artifact / Log 数据模型
- SQLite schema 真正落地
- 执行记录表设计
- 状态流转规则落地

### 3.3 API 层
- 健康检查接口
- 创建任务接口
- 查询任务状态接口
- 查询结果接口
- 查询执行历史接口
- 取消 / 重试接口

### 3.4 执行层
- fake runner 真正实现
- runner trait / interface 抽象
- real runner adapter
- 对 `lightpanda-io/browser` 的接入准备

### 3.5 网络与身份层
- 指纹模板 / 策略模型落地
- 代理抓取后的清洗、验证、候选入池联动
- 代理分配与轮换策略落地
- 代理失败剔除机制
- 地区代理基础存量维持机制

### 3.6 稳定性与观测
- 结构化日志
- 错误分类
- smoke test
- artifact / log 管理策略
- 磁盘占用与清理策略

---

## 4. 未来将要实现的功能

### 4.1 中期目标
- 跑通最小可运行闭环
- 用 fake runner 完成端到端验证
- 建立最小可验证 API 服务
- 建立最小任务系统

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

当前项目**已经把方向、文档、执行协议、调度框架和 Rust 骨架搭起来了**，但**真正的业务闭环能力（数据库、API、任务执行、fake runner、真实引擎接入）还在建设中**。

---

## 6. 维护规则

后续每次推进时，优先同步更新：
- `已实现`：只有真正落地的功能才能写进来
- `正在做`：只写当前阶段真实推进重点
- `未来将实现`：只保留中长期确定方向，避免空泛堆砌
