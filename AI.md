# AI.md

`lightpanda-automation` / `AutoOpenBrowser` 项目 AI 入口文档。

目的：让人或 AI 在最短时间内理解这个项目是什么、现在到哪了、应该先看什么、如何安全接手继续推进。

---

## 1. 项目是什么

这是一个运行在 Ubuntu 上的高性能浏览器自动化系统原型，当前阶段重点是：

- 用 Rust 构建后端主程序
- 用 SQLite 做任务/运行/日志持久化
- 通过 REST API 暴露任务创建、查询、取消、重试、运行历史、日志等能力
- 先用 fake runner 跑通闭环
- 为后续接入 `lightpanda-io/browser` 真实执行器预留统一 runner 抽象

一句话：

> 这是一个从“可运行任务原型”向“真实浏览器自动化系统”演进的工程，不是单次脚本。

---

## 2. 当前阶段

当前已处于：

> 最小可运行后端原型已打通，正在向更完整的控制面、观测面和真实执行器适配层演进。

当前已知现状：

- 最小任务闭环已建立
- 已有 SQLite 持久化
- 已有内存任务队列
- 已有 fake runner
- 已有 runs / logs 查询接口
- 已有可选 API Key 鉴权
- 已有 Lightpanda runner 占位适配层与 runner kind 切换入口

接手时不要把项目误判为“只有文档没有代码”。这个项目已经进入了真实工程推进阶段。

---

## 3. 项目根目录关键文件

### 标准入口文档
- `AI.md`：本文件，项目接手入口
- `PLAN.md`：统一计划书，收口当前计划、优先级、阻塞与下一步
- `FEATURES.md`：最终目标功能清单

### 现有核心文档
- `README.md`：项目摘要与传统入口说明
- `STATUS.md`：当前状态、风险、下一步
- `PROGRESS.md`：已经落地的功能进展
- `TODO.md`：任务清单
- `ROADMAP.md`：滚动路线图
- `VISION.md`：项目最终效果与核心能力定义
- `LONG_TERM_ROADMAP.md`：中长期能力演进方向
- `GOLDEN_FEATURES.md`：高价值增强能力清单
- `CURRENT_TASK.md`：当前任务定义
- `CURRENT_DIRECTION.md`：当前阶段方向约束

### 运行/自动推进相关
- `EXECUTION_LOG.md`
- `RUN_STATE.json`
- `AUTONOMY_PLAN.md`
- `EXECUTION_PROTOCOL.md`
- `EXECUTION_STATE_MACHINE.md`
- `EXECUTION_CHECKLIST.md`
- `ROUND_SCHEDULER.md`
- `round-results/`
- `summaries/`

---

## 4. 目录结构速览

```text
src/
  main.rs
  lib.rs
  runner/
  ...

scripts/
  run_round.py
  scheduler_daemon.py

round-results/
summaries/
```

重点：
- `src/` 是主代码区
- `src/runner/` 是当前最关键演进点之一
- 各类 `EXECUTION_*`、`RUN_STATE.json`、`round-results/` 属于自动推进体系

---

## 5. 接手顺序（默认）

以后任何人或 AI 接手本项目，默认按这个顺序：

1. 先看 `AI.md`
2. 再看 `PLAN.md`
3. 再看 `FEATURES.md`
4. 再看 `STATUS.md` / `PROGRESS.md`
5. 最后进入 `src/` 看代码与当前改动

不要一上来就在大量历史文档里乱翻，否则很容易被旧阶段信息带偏。

---

## 6. 接手时必须确认的事

- 当前工作树是否有未提交改动
- 文档描述是否仍与代码能力一致
- 当前“主任务”是否只有一个
- 当前是否处于 fake runner 增强阶段，还是 lightpanda runner 接入阶段
- 是否需要同步更新 `AI.md` / `PLAN.md` / `FEATURES.md`

---

## 7. 文档映射关系

本项目在补充标准文档前，原有文档职责大致如下：

- `README.md` ≈ 旧版入口说明
- `TODO.md` + `ROADMAP.md` + `CURRENT_TASK.md` ≈ 旧版计划系统
- `VISION.md` + `ROADMAP.md` + `GOLDEN_FEATURES.md` ≈ 旧版最终功能定义

因此新增 `AI.md` / `PLAN.md` / `FEATURES.md` 时，原则是：

> 做统一收口，不推翻已有有效文档。

---

## 8. 接手原则

- 先校准文档，再推进代码
- 先确认当前真实状态，再决定下一步
- 不把历史规划误当成已实现能力
- 不把单轮临时任务误当成长期方向
- 每次代码改动后同步检查标准文档是否需要更新

