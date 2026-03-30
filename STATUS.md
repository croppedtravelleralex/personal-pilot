# STATUS.md

## 当前状态摘要

- **状态：** 已从文档驱动/工程骨架阶段推进到 **最小可运行原型阶段**
- **日期：** 2026-03-30
- **当前焦点：** 在已跑通的最小后端原型之上，继续把 fingerprint profile 从控制面推进到真实执行器消费边界，同时完善并发控制、观测面与 `LightpandaRunner` 的最小真实执行链路。

## 本文件用途

`STATUS.md` 只保留：
- **当前状态**
- **当前风险**
- **当前下一步**

更完整的进展说明请看：
- `PROGRESS.md` — **已实现 / 正在做 / 未来将实现**
- `ROADMAP.md` — **过去 / 现在 / 未来的滚动路线图**
- `EXECUTION_LOG.md` — **每轮执行记录**
- `RUN_STATE.json` — **轮次与调度状态**

## 当前风险

- **fingerprint profile 已打通到 runner 注入入口第一版**，当前 fake/lightpanda 都能拿到统一 profile 视图；但真实执行器如何把 profile 转成 Lightpanda 命令参数 / 环境变量 / 浏览器上下文，仍未落地。

- **API Key 鉴权已具备可选能力**，但默认未开启，当前仍更适合本地开发和原型验证，不适合裸暴露
- **running cancel 已完成第一轮一致性收口**，当前 queued/running cancel 都会写日志，running cancel 也会同步回写最近 run 为 `cancelled`；但仍需继续验证真实进程终止后的边界行为
- **`status / runs / logs` 已增加 `limit + offset` 第二版分页控制**，当前已可做基础翻页；后续如数据量继续增大，仍可能需要 cursor 等更强策略
- **`LightpandaRunner` 已接入最小真实执行第一版**，但当前仍偏 V1 形态，结果结构、错误语义与稳定性还需要继续打磨
- **runner 通用执行层刚完成第一轮抽离**，仍需继续检查职责边界与接口稳定性
- **当前已可正常执行 `cargo test`**，但宿主机缺少 `rustfmt`，暂时无法用 `cargo fmt` 完成统一格式化收口。

## 当前下一步

1. **继续做 DB-first claim / reclaim 的并发竞争收口**
2. **继续细化 lease TTL / reclaim 策略与 worker 退避**
3. **继续补 cancel / retry / claim / reclaim / heartbeat / fingerprint 的集成测试覆盖**
4. **让内存队列进一步降级为唤醒/提示层，而不再参与执行真相判断**
5. **继续清理 worker loop、claim SQL 与状态机边界**
6. **继续把并发运行态观测补到 API / metrics 层，而不只停留在启动日志**
7. **保持文档与代码能力同步更新**

- **fingerprint profile 控制面 + 注入入口第一版已落地**，当前已覆盖创建/查询/校验/绑定/注入/异常降级，并已通过 `cargo test` 验证。

- **并发控制第一版骨架已落地**，当前已支持通过 `AUTO_OPEN_BROWSER_RUNNER_CONCURRENCY` 启动多 worker 共享队列；但最小一致性保护与并发测试仍需继续补齐。

- **最小一致性保护第一轮已落地**，当前已对 retry 重复入队、cancel 后 run 终态覆盖、以及 `status/logs` 并发排序抖动做了最小收口；但更完整的事务化/claim 机制仍未实现。

## 本轮体检（2026-03-30）

- **找 bug：** 本轮已修复一个真实链路 bug：创建任务时 `fingerprint_profile_version` 曾因 SQL 占位错误未正确落库，现已修复并补测试锁定。
- **性能评分：** 当前阶段 **8.2/10**。优点是任务主链、runner 注入、异常降级和测试回归都已成形；扣分点是 Lightpanda 仍未真实消费 profile，claim/reclaim 参数化与高并发写放大控制也还没落地。
- **改进建议：** 下一步优先把 profile 注入从“runner 已拿到”推进到“真实执行器已消费”，同时补 profile 命中/降级日志与 metrics，避免运行时黑盒。

- **DB-first claim / reclaim 参数化第一版已落地**，当前 heartbeat 间隔与 claim 重试次数都已可配置，`/status.worker` 也能直接看到当前运行参数；但 claim 原子性、退避策略和更高并发下的一致性收口仍需继续加强。
