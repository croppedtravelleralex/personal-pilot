# PLAN.md

`lightpanda-automation` / `AutoOpenBrowser` 项目统一计划书。

---

## 1. 当前总目标

把当前已经打通的最小后端原型，继续推进为一个：

- 具备稳定任务生命周期管理能力
- 具备更完整控制面与观测面
- 可从 fake runner 平滑演进到 real runner
- 可逐步接入 `lightpanda-io/browser` 的浏览器自动化系统

---

## 2. 当前阶段

当前阶段不是从零开始搭骨架，而是：

> 在已有最小闭环基础上，继续增强 runner 抽象、控制面、观测面，并为真实执行器接入做准备。

---

## 3. 当前优先级

### P0：主线推进
1. 核对当前代码与文档是否一致，避免状态漂移
2. 固化标准接手入口（`AI.md` / `PLAN.md` / `FEATURES.md`）与旧文档映射
3. 稳定 runner trait / adapter interface
4. 推进 `lightpanda` runner 适配层从占位走向真实可接入

### P1：控制面与观测面增强
1. 明确当前取消、分页、状态控制等缺口的落地顺序
2. 增强 `runs / logs / status` 的查询控制、limit、分页
3. 为 running cancel 做设计预留或第一版实现

### P2：中期能力铺垫
1. 指纹能力边界设计
2. 代理池 / 代理抓取 / 清洗 / 轮换 / 自生长策略设计
3. 磁盘使用控制、artifact/log 保留与归档策略
4. 高并发下性能优化与写放大控制策略

---

## 4. 当前已知阻塞 / 风险

- Lightpanda runner 目前仍偏占位，尚未形成真实执行闭环
- running cancel 仍未真正支持
- 查询侧能力后续需要 limit / 分页控制
- 部分历史文档保留了旧阶段表述，存在认知分散风险
- 当前工作树已有未提交改动，接手时需小心不要覆盖进行中的实现

---

## 5. 当前执行原则

1. 一次只聚焦一个主任务
2. 文档描述必须与代码能力对齐
3. 所有新实现都要能说明：它如何服务 fake → real runner 演进主线
4. 若文档过多，优先统一入口，不盲目删除历史文档

---

## 6. 建议的接手动作顺序

1. 读取 `STATUS.md` 与 `PROGRESS.md`，确认项目真实状态
2. 检查 `git status`，确认当前改动范围
3. 读取 `src/main.rs` 与 `src/runner/`，确认当前主线是否正在转向 lightpanda runner
4. 再决定当前轮的唯一主任务

---

## 7. 本计划书与旧文档关系

- `TODO.md`：保留为细粒度待办池
- `ROADMAP.md`：保留为滚动路线图
- `CURRENT_TASK.md` / `CURRENT_DIRECTION.md`：保留为阶段性方向文件
- `PLAN.md`：只做统一收口与当前优先级定义

