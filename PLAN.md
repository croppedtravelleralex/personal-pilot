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
3. 完成 runner 通用执行层抽离后的结构收口，确保 fake/lightpanda/engine 职责清晰
4. 推进 fingerprint profile 从 runner 注入入口走向真实执行器消费边界
5. 继续验证并打磨 `LightpandaRunner` 本地二进制执行链路（stdout/stderr/timeout/exit code）
6. 推进 `lightpanda` runner 适配层从最小真实执行走向更稳定可用
7. 增强 `runs / logs / status` 查询控制与分页
8. 继续推进 `running cancel` 从设计预留走向最小可用第一版

### P1：控制面与观测面增强
1. 明确当前取消、分页、状态控制等缺口的落地顺序
2. 增强 `runs / logs / status` 的查询控制、limit、分页
3. 为 running cancel 做设计预留或第一版实现

### P2：中期能力铺垫
1. 补齐浏览器指纹能力边界设计，并收口为可配置/可注入/可验证的第一版策略层
2. 代理池 / 代理抓取 / 清洗 / 轮换 / 自生长策略设计
3. 磁盘使用控制、artifact/log 保留与归档策略
4. 高并发下性能优化与写放大控制策略

---

## 3.1 Fingerprint -> Lightpanda 当前落地方向

当前建议按以下顺序推进：
1. 先在 `LightpandaRunner` 内新增 profile 消费边界函数，把 `RunnerFingerprintProfile` 映射为统一运行时配置对象
2. 第一版优先映射低风险静态字段：`accept_language / timezone / locale / viewport / screen / platform / hardware_concurrency / device_memory_gb`
3. 注入方式优先级：**环境变量 / CLI 参数 / 预留 browser context 配置**，避免一开始就侵入复杂浏览器补丁层
4. 若执行器暂不支持某字段，必须显式记录“已收到但未消费”，不要静默吞掉
5. 继续保持 profile 注入和真实执行解耦，让 fake runner / lightpanda runner 共享同一输入模型

## 4. 当前已知阻塞 / 风险

- `LightpandaRunner` 虽已进入最小真实执行阶段，但当前仍偏 V1，缺少充分验证与稳定性保护
- `LightpandaRunner` 仍未真正消费 fingerprint profile，只是拿到了统一注入对象
- 查询侧能力虽已有 limit / offset，但后续仍可能需要更强的 cursor / metrics 方案
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


## 额外进展（2026-03-30）

- claim / reclaim 参数化第一版已落地：
  - `AUTO_OPEN_BROWSER_RUNNER_RECLAIM_SECONDS`
  - `AUTO_OPEN_BROWSER_RUNNER_HEARTBEAT_SECONDS`
  - `AUTO_OPEN_BROWSER_RUNNER_CLAIM_RETRY_LIMIT`
- 当前更适合继续推进的方向是：claim SQL 原子化增强、worker 退避、以及 reclaim / retry / cancel 并发竞争收口。

- claim / reclaim 并发收口第二版已落地：
  - `claim_next_task()` 使用单条 `CTE + UPDATE ... RETURNING` 原子抢占
  - worker 空闲与错误场景采用 idle exponential backoff
  - `/status.worker` 可见 heartbeat / claim retry / idle backoff 参数

- 并发收口第三版已落地：
  - reclaim 增加 `runner_id IS NOT NULL` 安全条件
  - `running` 状态下 retry 明确返回 `409 CONFLICT`
  - 新增对应回归测试，当前调度边界更清晰

- 代理池 V1 骨架已落地：
  - `proxies` 表
  - `/proxies` 创建/列表/详情接口
  - `CreateTaskRequest.network_policy_json`
  - runner 执行前最小代理解析（`proxy_id` / `region + min_score`）
  - fake/lightpanda 结果回显 `proxy`，Lightpanda 注入 `LIGHTPANDA_PROXY_*` 环境变量

- 代理健康回写第一版已落地：
  - success -> `success_count + 1`，刷新 `last_used_at / last_checked_at / updated_at`
  - failed -> `failure_count + 1`，写入短 `cooldown_until`
  - timed_out -> `failure_count + 1`，写入更长 `cooldown_until`

- 代理选择策略第一版增强已落地：
  - `provider` 过滤
  - `cooldown_until` 过滤
  - 最小版 `sticky_session` 复用
  - fallback 顺序：sticky > `provider/region/min_score` > `score DESC + last_used_at ASC + created_at ASC`

- 代理观测面增强 + smoke test 第一版已落地：
  - `/tasks/:id` 与 `/status.latest_tasks` 暴露 `proxy_id / proxy_provider / proxy_region / proxy_resolution_status`
  - `/status` 新增 `proxy_metrics` 聚合
  - `POST /proxies/:id/smoke` 做最小 TCP smoke test，并回写 `last_checked_at / failure_count / cooldown_until`

- sticky/provider 正式映射结构第一版已落地：
  - 新增 `proxy_session_bindings` 表
  - 执行完成后 upsert `sticky_session -> proxy_id` 绑定
  - 执行前优先按 binding 命中，并校验 `expires_at / cooldown / provider / region / min_score`
  - sticky 不再走历史任务 `result_json` 回溯
