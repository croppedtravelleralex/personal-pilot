# RUNNING_CANCEL_PLAN.md

`running cancel` 第一版设计预留。

目的：在不把现有 runner 架构写脏的前提下，为“取消正在执行中的任务”留出正确的演进路径。

---

## 1. 当前现状

当前项目只支持：

- `queued` 状态任务取消
- 从内存队列中移除任务
- 回写任务为 `cancelled`

当前**不支持**：

- 取消 `running` 状态任务
- 杀掉正在运行的外部执行器进程
- 通知 runner 停止当前执行

这在 fake runner 阶段还能接受，但 `LightpandaRunner` 已接入最小真实执行后，`running cancel` 已经进入必须正视的范围。

---

## 2. 为什么不能直接硬做

如果现在直接在 API 层“收到取消请求就把数据库状态改成 cancelled”，会有几个问题：

1. **数据库状态和真实执行状态会脱节**
2. 外部进程可能还在跑
3. runner 可能继续回写 succeeded / failed / timeout
4. 会出现“看起来已取消，实际上还在执行”的假取消

所以不能只改数据库，不处理执行链路。

---

## 3. 第一版设计目标

`running cancel` V1 不追求完美，只要求：

1. 有明确的取消请求入口
2. 有 runner 侧的取消能力抽象
3. 至少为外部进程型 runner 留出进程终止路径
4. 保证取消状态和最终回写语义不明显冲突

---

## 4. 建议演进方式

### Phase A：设计预留（当前阶段）
- 在 runner capability 中明确 `supports_cancel_running`
- 为 runner 扩展 cancel 接口（即使默认未实现）
- 在任务执行上下文中引入“可取消句柄”的概念
- 不直接在 API 层伪造取消成功

### Phase B：最小可用第一版
- 对外部进程型 runner（如 `LightpandaRunner`）保存子进程句柄/可取消标识
- 收到 cancel 请求时，尝试终止子进程
- 若终止成功，任务状态进入 `cancelled`
- 若终止失败，返回明确错误，而不是伪成功

### Phase C：统一 runner 能力
- fake / lightpanda / 未来 runner 共用 cancel 抽象
- 明确取消后的回写优先级和状态机规则

---

## 5. 当前最合理的 V1 边界

我建议 `running cancel` 第一版只支持：

- `LightpandaRunner` 这种外部进程型 runner
- 只处理“当前任务正在执行一个 fetch 子进程”的场景

先不处理：

- 多阶段脚本执行
- 深层子进程树
- 分布式取消
- cancel 后 artifact 清理

---

## 6. 接口建议

### runner trait 方向
建议后续扩展类似能力：

- `supports_cancel_running`
- `cancel(task_id)` 或 `cancel(run_id)`
- `RunnerCancelResult` 统一表达取消请求是否被 runner 接受

### 执行上下文方向
建议引入：

- `running task id -> cancel handle` 的注册表
- 外部进程句柄或取消 token

---

## 7. 当前结论

当前不应该直接硬做 `running cancel` 代码闭环。

正确顺序是：

1. 先做设计预留
2. 再确定 `LightpandaRunner` 的取消句柄如何保存
3. 再落最小外部进程取消实现

---

## 8. 一句话结论

> `running cancel` 不能只改数据库；必须把“执行中的真实 runner”纳入取消链路。

