# STATUS.md

## 当前状态摘要

- **状态：** 已从文档驱动/工程骨架阶段推进到 **最小可运行原型阶段**
- **日期：** 2026-03-27
- **当前焦点：** 在已跑通的最小后端原型之上，继续清理 runner 抽象边界、增强控制面/观测面，并打磨 `LightpandaRunner` 的最小真实执行链路

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

- **API Key 鉴权已具备可选能力**，但默认未开启，当前仍更适合本地开发和原型验证，不适合裸暴露
- **running cancel 已完成第一轮一致性收口**，当前 queued/running cancel 都会写日志，running cancel 也会同步回写最近 run 为 `cancelled`；但仍需继续验证真实进程终止后的边界行为
- **`status / runs / logs` 已增加 `limit + offset` 第二版分页控制**，当前已可做基础翻页；后续如数据量继续增大，仍可能需要 cursor 等更强策略
- **`LightpandaRunner` 已接入最小真实执行第一版**，但当前仍偏 V1 形态，结果结构、错误语义与稳定性还需要继续打磨
- **runner 通用执行层刚完成第一轮抽离**，仍需继续检查职责边界与接口稳定性
- **宿主机当前未发现可用 `cargo` / `rustc`**，Rust 编译与测试验证暂时受阻；需先恢复工具链，才能完成真实 `cargo test` / `cargo check`

## 当前下一步

1. **完成 `LightpandaRunner` 最小真实执行第一版后的 bug / 结构校准**
2. **打磨本地二进制执行链路（stdout/stderr/timeout/exit code）**
3. **补 `LightpandaRunner` 最小验证覆盖（非法输入 / 缺失二进制 / 非 0 退出 / timeout）**
4. **继续验证 `limit + offset` 分页控制第二版是否满足当前查询需求**
5. **继续验证并打磨 `running cancel` 第一版状态回写与边界行为**
6. **恢复 Rust 工具链，完成真实 `cargo test` / `cargo check`**
7. **保持文档与代码能力同步更新**

- **集成测试骨架第一版已落地**，当前先覆盖 fake runner 成功闭环与 retry 基本状态流转；真实可执行性仍待宿主机 Rust 工具链恢复后跑通 `cargo test` 验证。

- **并发控制第一版骨架已落地**，当前已支持通过 `AUTO_OPEN_BROWSER_RUNNER_CONCURRENCY` 启动多 worker 共享队列；但最小一致性保护与并发测试仍需继续补齐。

- **最小一致性保护第一轮已落地**，当前已对 retry 重复入队、cancel 后 run 终态覆盖、以及 `status/logs` 并发排序抖动做了最小收口；但更完整的事务化/claim 机制仍未实现。
