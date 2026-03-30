# PROGRESS.md

`lightpanda-automation` 项目进展记录。

目标：用一份简洁文档持续说明三件事：
- **已经实现了什么**
- **现在正在做什么**
- **后面将要实现什么**

---

## 0. 更新记录（按功能逐条记录）

> 规则：**每实现一个功能，就新增一条记录**；每条记录都带 **年月日时分秒** 时间戳，格式统一为：`YYYY.M.DD-HH:MM:SS`。

- **2026.3.26-12:18:00** 实现了 **`PROGRESS.md` 进展文档初版**，用于统一记录已实现 / 正在做 / 未来将实现的功能。
- **2026.3.26-12:27:00** 实现了 **SQLite schema 草案与核心数据模型骨架**，落地 `tasks / runs / artifacts / logs` 表结构与 `TaskStatus` 状态流转规则。
- **2026.3.26-12:31:00** 实现了 **最小 REST API 骨架**，落地 `GET /health`、`POST /tasks`、`GET /tasks/:id`。
- **2026.3.26-12:32:00** 实现了 **数据库初始化入口**，支持启动时初始化 SQLite 连接池并执行 schema SQL。
- **2026.3.26-12:39:00** 实现了 **`POST /tasks` 写入 SQLite**，创建任务不再是假响应，而是会真实入库。
- **2026.3.26-12:41:00** 实现了 **`GET /tasks/:id` 从 SQLite 查询**，打通“创建任务 -> 查询任务”的最小闭环。
- **2026.3.26-12:43:00** 实现了 **内存任务队列**，创建任务后会自动入队，并在健康检查中暴露队列长度。
- **2026.3.26-12:44:00** 实现了 **fake runner 第一版**，支持后台消费队列并回写任务状态。
- **2026.3.26-12:48:00** 实现了 **`runs` 执行历史写入**，任务执行会生成最小 run history 记录。
- **2026.3.26-12:50:00** 实现了 **fake runner 的失败 / 超时分支**，支持 success / fail / timeout 三种模拟结果。
- **2026.3.26-12:51:00** 实现了 **SQLite 数据目录自动创建**，启动时会自动创建数据库父目录，提升启动稳健性。
- **2026.3.26-12:53:00** 实现了 **`logs` 执行日志写入**，关键执行节点会写入 `logs` 表。
- **2026.3.26-12:58:00** 实现了 **稳定唯一 ID 生成方式**，`task / run / log` 全部改为 UUID 风格 ID。
- **2026.3.26-13:01:00** 实现了 **重试机制第一版**，支持对 `failed / timeout` 任务重试并重新入队。
- **2026.3.26-13:41:00** 实现了 **取消机制第一版**，支持取消 `queued` 状态任务并从队列中移除。
- **2026.3.26-13:44:00** 实现了 **health / status 服务摘要输出增强**，支持返回队列长度、任务状态统计与最近任务摘要。
- **2026.3.26-13:51:00** 实现了 **`runs / logs` 查询接口**，支持查看指定任务的运行历史和执行日志。
- **2026.3.26-14:03:00** 完成了 **README / STATUS / PROGRESS 文档清理**，让文档描述与当前代码能力重新对齐。
- **2026.3.26-15:09:13** 实现了 **API Key 鉴权（可选）**，支持 `x-api-key` / `Authorization: Bearer` 头校验。
- **2026.3.26-16:51:52** 实现了 **runner trait / adapter interface 第一版**，将 fake runner 纳入统一 `TaskRunner` 抽象，并将启动入口改为通过统一 runner loop 启动。
- **2026.3.26-17:27:00** 实现了 **Lightpanda runner 占位适配层与 runner kind 切换入口**，新增 `LightpandaRunner` 占位实现，并支持通过 `AUTO_OPEN_BROWSER_RUNNER` 在 `fake / lightpanda` 间切换。
- **2026.3.27-11:15:00** 完成了 **标准项目文档入口层补齐**，新增 `AI.md` / `PLAN.md` / `FEATURES.md`，并在 `README.md` 增加标准接手入口，统一项目接手路径。
- **2026.3.27-11:26:00** 完成了 **runner 通用执行层第一轮抽离**，新增 `src/runner/engine.rs`，将任务消费、run/log 写入与状态回写从 `fake.rs` 抽离，为后续 `lightpanda` 接入清理职责边界。
- **2026.3.27-11:29:00** 完成了 **RunnerTask 真实输入接线第一版**，通用执行层开始从 `tasks.input_json` 读取任务输入并传给 runner，同时为 `timeout_seconds` 增加透传位。
- **2026.3.27-11:33:00** 完成了 **`LightpandaRunner` V1 接入边界定义**，新增 `LIGHTPANDA_V1_PLAN.md`，明确第一版先以最小真实页面访问为目标，不提前堆叠脚本、截图、代理、指纹等高级能力。
- **2026.3.27-11:36:00** 完成了 **`LightpandaRunner` 参数校验与错误语义第一版**，开始校验 `url` 输入、区分 `invalid_input / not_implemented` 错误语义，为后续最小真实执行接入铺路。
- **2026.3.27-11:47:00** 完成了 **`LightpandaRunner` 最小真实执行第一版**，新增 `LIGHTPANDA_BIN` 配置读取，并通过本地 `lightpanda fetch <url>` 执行真实页面访问，开始回收 `stdout / stderr / exit_code / timeout` 到结果链路。
- **2026.3.27-11:53:00** 完成了 **`LightpandaRunner` 结果结构与错误语义收紧**，统一输出 `status / error_kind / exit_code / stdout_preview / stderr_preview` 字段，并为截断输出增加明确的 `...[truncated]` 标记。
- **2026.3.27-12:05:00** 完成了 **查询控制与分页第一版**，为 `status / runs / logs` 接口增加 `limit` 参数与上限约束，避免结果集无限增长。
- **2026.3.27-12:10:00** 完成了 **`running cancel` 设计预留**，新增 `RUNNING_CANCEL_PLAN.md`，明确取消正在执行任务不能只改数据库状态，而必须把真实 runner 和外部进程纳入取消链路。
- **2026.3.27-12:15:00** 完成了 **runner cancel 抽象层第一版**，为 `TaskRunner` 增加 `cancel_running` 默认接口与 `RunnerCancelResult` 类型，为后续 `LightpandaRunner` 最小可用取消实现打底。
- **2026.3.27-12:18:00** 完成了 **`LightpandaRunner` 取消句柄注册表第一版**，为运行中的任务登记子进程 pid，并让 `cancel_running(task_id)` 能识别当前是否存在可取消的外部进程。
- **2026.3.27-12:22:00** 完成了 **`LightpandaRunner` 最小进程终止链路第一版**，`cancel_running(task_id)` 已可尝试对登记中的外部进程发送 `SIGTERM`，把 running cancel 从“识别可取消对象”推进到“尝试实际终止进程”。
- **2026.3.27-12:26:00** 完成了 **AppState 持有当前 runner 句柄的结构调整**，为 API 层把 running cancel 请求真正转发到 runner cancel 链路打通前置条件。
- **2026.3.27-12:29:00** 完成了 **API 层 running cancel 第一版接线**，`POST /tasks/:id/cancel` 在任务处于 `running` 状态时，已会调用 `state.runner.cancel_running(task_id)`，并在成功后回写 `cancelled` 状态。
- **2026.3.27-12:32:00** 完成了 **running cancel 状态竞争保护第一版**，通用执行层在 runner 执行结束后会先检查任务当前状态，若任务已被标记为 `cancelled`，则不再用 succeeded/failed/timeout 回写覆盖取消结果。

---

- **2026.3.28-01:18:00** 完成了 **`LightpandaRunner` 稳定性收口第一轮**，补齐缺失二进制/非法输入/非 0 退出/timeout 的错误分类与最小测试覆盖，并在 timeout 分支增加真实子进程终止处理。

- **2026.3.28-01:24:00** 完成了 **running cancel 一致性收口第一轮**，让 queued/running cancel 都写入日志，并在 running cancel 时同步回写最近一条 run 为 `cancelled`，降低 task/run/log 状态漂移风险。

- **2026.3.28-01:28:00** 完成了 **最小 smoke test 脚本第一版**，新增 `scripts/smoke_test.sh` 用于串行验证 `health -> create task -> poll task -> runs/logs/status` 主链路，便于后续做最小回归检查。

- **2026.3.28-01:35:00** 完成了 **lightpanda 专项验证脚本入口第一版**，新增 `scripts/lightpanda_verify.sh` 作为真实执行器边界验证入口，并记录当前宿主机 Rust 工具链缺失导致 `cargo test` 暂时受阻。

- **2026.3.28-01:34:00** 完成了 **cancel 边界日志补强**，runner 在任务已被标记为 `cancelled` 后若继续结束，会补写一条跳过终态覆盖的 warn 日志，帮助排查取消后的竞态行为。

- **2026.3.28-01:33:00** 完成了 **查询分页控制第二版**，为 `status / runs / logs` 接口补齐 `offset` 参数，支持基础翻页查询，降低结果集持续增大时的读取压力。

- **2026.3.28-09:45:00** 完成了 **集成测试骨架第一版落地**，新增 `tests/integration_api.rs` 与测试友好构建入口，先覆盖 fake runner 成功链路与 retry 基本状态流转，为后续并发改造提供回归底座。

- **2026.3.28-10:15:00** 完成了 **并发控制第一版骨架落地**，将 runner 启动方式从单 worker loop 升级为可配置的多 worker 模型，新增 `AUTO_OPEN_BROWSER_RUNNER_CONCURRENCY` 并在主程序与测试入口显式传入并发度。

- **2026.3.28-10:20:00** 完成了 **最小一致性保护第一轮**，为 retry 增加条件更新与队列防重入队，并在 cancel 后保护 latest run 不再被 worker 终态覆盖；同时补了 `status/logs` 的稳定排序键。

- **2026.3.28-11:34:00** 完成了 **执行引擎与状态命名收口第一轮**，修复 `src/runner/engine.rs` 被错误内容污染的问题，恢复 `run_one_task_with_runner(...)` 主执行链路；同时开始统一任务超时状态命名，由 `timeout` 向 `timed_out` 收口，并为 worker 增加最小状态检查，降低并发下的脏执行风险。

- **2026.3.28-12:01:00** 完成了 **worker error 可见化第一版**，修复多 worker loop 对 `engine::run_one_task_with_runner(...)` 错误静默吞掉的问题；当前若执行引擎报错，会输出包含 `worker_id / runner / error` 的运行时错误日志，降低排障黑盒程度。

- **2026.3.28-12:06:00** 完成了 **状态常量统一入口第一版**，为 task/run 状态新增统一常量与 `as_str()` 映射，并将 `runner/engine.rs` 与 `api/handlers.rs` 开始接入共享状态入口，降低状态字符串散落带来的命名漂移风险。

- **2026.3.28-12:08:00** 完成了 **engine 状态常量混用修复**，将 `src/runner/engine.rs` 中任务状态更新到 `running` 的绑定从 run 常量改回 task 常量，避免 task/run 状态未来分叉时埋下语义错误。

- **2026.3.28-12:11:00** 完成了 **cancel / latest run 状态口径修复第一轮**，将 `api/handlers.rs` 中 running cancel 回写最近一条 run 的状态绑定从 task 常量改为 run 常量，进一步收紧 task/run 状态边界。

- **2026.3.28-12:22:00** 完成了 **Lightpanda timeout 状态口径收口第一轮**，将 `src/runner/lightpanda.rs` 的内部结果状态从 `timeout` 统一到 `timed_out`，同时保留 `error_kind=timeout` 作为错误分类字段，区分“状态口径”和“错误类型”两层语义。

- **2026.3.28-13:35:00** 完成了 **handlers/tests 状态字面量清理第一轮**，将 `retry` 返回值与集成测试中的常见任务状态断言接入共享 task 状态常量，降低测试与接口对状态命名漂移的敏感度。

- **2026.3.28-13:58:00** 完成了 **统计查询与执行条件状态常量接线第一轮**，将 `api/handlers.rs` 中任务状态统计查询和 `runner/engine.rs` 中 `queued -> running` 的关键 SQL 条件开始接入共享 task 状态常量，继续减少主链路里的状态字面量。

- **2026.3.28-14:07:00** 完成了 **retry 幂等性修复第一轮**，将 `retry_task` 的允许状态条件接入共享 task 状态常量，并将重复入队场景从冲突调整为幂等成功返回，减少任务已进入 `queued` 但接口仍报冲突的语义噪音。

- **2026.3.28-14:09:00** 完成了 **retry 队列/数据库漂移收口第一轮**，将 `retry_task` 从“先改 DB 再入队”调整为“先尝试入队、再条件更新 DB、失败时回滚队列”，并补上 queued/race 场景的幂等处理，降低任务被标记为 `queued` 但实际未进入内存队列的风险。

- **2026.3.28-14:15:00** 完成了 **running cancel 目标 run 选择收口第一轮**，将 `cancel_task` 从按最新 attempt 选取 run，调整为优先选取当前 `running` 状态的 run 再回写 `cancelled`，降低 cancel 误伤非当前执行 run 的竞态风险。

- **2026.3.28-14:23:00** 完成了 **worker finish/run 覆盖竞态收口第一轮**，将 `runner/engine.rs` 中 run 终态回写改为仅允许从 `running` 状态收尾；若 `cancel_task` 已先将 run 标记为 `cancelled`，worker 将跳过覆盖并记录 warning log，降低取消后 run 终态被执行结果反向覆盖的风险。

- **2026.3.28-18:51:00** 完成了 **worker finish/task 覆盖竞态收口第一轮**，将 `runner/engine.rs` 中 task 终态回写改为仅允许从 `running` 状态收尾；若任务已被其他路径改出 `running`（例如取消竞态），worker 将跳过覆盖并记录 warning log，进一步降低 task 终态被晚到执行结果反向覆盖的风险。

- **2026.3.28-19:29:00** 完成了 **fake/lightpanda 结果结构对齐第一轮**，为 `FakeRunner` 补齐与 `LightpandaRunner` 更接近的最小结果字段集合，统一输出 `runner / action / ok / status / error_kind / task_id / attempt / kind / payload / message`，降低不同 runner 返回结构分叉带来的接口与测试复杂度。

- **2026.3.28-19:31:00** 完成了 **runner 结果字段超集对齐第一轮**，进一步将 `FakeRunner` 与 `LightpandaRunner` 的 `result_json` 收敛到接近同一字段超集：`task_id / attempt / kind / payload / url / timeout_seconds / bin / exit_code / stdout_preview / stderr_preview` 等字段在两类 runner 中都具备可消费位置（缺失场景以 `null` 表达），继续降低上层消费分支复杂度。

- **2026.3.28-19:33:00** 完成了 **runner 结果结构测试锁定第一轮**，为 `FakeRunner` 新增最小结果字段断言，并在 `LightpandaRunner` 现有测试中补齐 `task_id / attempt / kind / payload` 等共享字段校验，降低 runner 结果结构再次漂移的风险。

- **2026.3.28-19:51:00** 完成了 **工具链验证收口与 timed_out retry 测试补强**，在恢复 Rust 工具链并跑通 `cargo check` / `cargo test` 后，补充 `timed_out -> retry` 集成测试，并通过 crate 级属性清理当前 `AutoOpenBrowser` 的非 snake_case 命名 warning，继续提高可验证性与测试覆盖。

- **2026.3.28-19:56:00** 完成了 **db init warning 清理第一轮**，移除 `src/db/init.rs` 中已无实际用途的 `ConnectOptions` 导入，并再次通过测试验证当前代码处于可编译、可测试状态。

- **2026.3.29-21:58:00** 完成了 **并发运行态可观测性补强第一轮**，为 `GET /status` 增加 `worker_count / queue_mode / reclaim_after_seconds` 摘要，让多 worker 与 reclaim 配置从“仅启动日志可见”推进到“API 面可见”，为后续 durable queue 与性能优化提供控制面观测基础。
- **2026.3.29-22:03:00** 完成了 **浏览器指纹能力边界设计第一版**，新增 `FINGERPRINT_BOUNDARY.md`，将指纹能力拆成声明层 / 注入层 / 一致性层 / 拟真层，明确当前阶段先做可配置、可持久化、可绑定、可验证的 profile 策略层，不提前投入重反检测拟真。
- **2026.3.29-22:08:00** 完成了 **fingerprint profile schema 与任务绑定字段第一版**，新增 `fingerprint_profiles` 表，并为 `tasks` 增加 `fingerprint_profile_id / fingerprint_profile_version` 字段；`CreateTaskRequest / TaskResponse / GET /status / GET /tasks/:id` 已开始回显 profile 绑定信息，为后续真实 profile 注入与一致性校验铺路。
- **2026.3.29-22:12:00** 完成了 **fingerprint profile 最小管理接口第一版**，新增 `POST /fingerprint-profiles`、`GET /fingerprint-profiles`、`GET /fingerprint-profiles/:id`，并让创建任务时可解析 active profile 的当前版本并写入任务绑定结果，为后续 profile 注入和版本审计提供基础。
- **2026.3.29-22:16:00** 完成了 **fingerprint profile 一致性校验器第一版**，新增 `src/network_identity/validator.rs`，对 `timezone / locale / accept_language / platform / viewport / screen / hardware_concurrency / device_memory_gb` 做最小静态一致性检查，并将 `validation_ok / validation_issues` 接入 fingerprint profile 的创建与查询返回。
- **2026.3.30-21:59:00** 完成了 **runner fingerprint profile 注入入口第一版**，`runner/engine.rs` 在 claim 任务时会联表读取 active fingerprint profile，并将 `id / version / profile_json` 注入 `RunnerTask`，让 fake/lightpanda runner 都能拿到统一的 profile 视图。
- **2026.3.30-22:00:00** 完成了 **fingerprint 绑定链路 bug 修复第一版**，修正创建任务时 `fingerprint_profile_version` 写库占位错误，避免 profile 版本“算出来但没落库”的假闭环。
- **2026.3.30-22:03:00** 完成了 **fingerprint 注入与异常场景集成测试补强**，新增 profile 注入成功、缺失 profile、inactive profile、stale version 四类集成测试；当前策略为：inactive profile 在创建阶段直接拒绝，缺失或版本不匹配的历史绑定在 runner 执行阶段按“无可用 profile”降级处理。
- **2026.3.30-23:00:00** 完成了 **DB-first claim / reclaim 参数化第一版**，新增 `AUTO_OPEN_BROWSER_RUNNER_HEARTBEAT_SECONDS` 与 `AUTO_OPEN_BROWSER_RUNNER_CLAIM_RETRY_LIMIT` 环境变量，claim 重试次数与 heartbeat 间隔不再写死；`/status.worker` 现在会返回 `reclaim_after_seconds / heartbeat_interval_seconds / claim_retry_limit` 这组运行参数。
- **2026.3.30-23:09:00** 完成了 **DB-first claim / reclaim 并发收口第二版**，将 `claim_next_task()` 从“先查 candidate 再 update”推进为单条 `CTE + UPDATE ... RETURNING` 的原子抢占链，并为 worker 增加 idle exponential backoff；`/status.worker` 现在额外暴露 `idle_backoff_min_ms / idle_backoff_max_ms`。

## 1. 已经实现 / 已经落地

### 1.1 项目方向与北极星已定义
- 已明确项目目标：构建一个运行在 Ubuntu 上的高性能浏览器自动化系统。
- 已明确早期技术路线：`Rust + SQLite + REST API + 内存任务队列 + fake runner`。
- 已明确后续真实执行引擎方向：`lightpanda-io/browser`。

### 1.2 文档体系已建立
已建立并持续维护以下核心文档：
- `AI.md`
- `PLAN.md`
- `FEATURES.md`
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
- API Key 鉴权（可选，环境变量控制）

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

### 1.9 fake runner 与执行引擎主链路已落地
已支持：
- 后台循环消费内存队列
- success / fail / timeout 三种模拟结果
- 任务状态回写主链路已恢复：`queued -> running -> succeeded/failed/timed_out`
- `started_at / finished_at / result_json / error_message` 回写
- 已实现 `TaskRunner` 统一抽象，fake runner 已转为该抽象下的一个实现
- `src/runner/engine.rs` 已完成第一轮修复，不再是错误混入的文档内容
- worker 在消费任务后会先检查当前任务状态，降低取消/重试并发下的脏执行风险

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

### 1.16 指纹 profile 控制面与执行链第一版已落地
已支持：
- `fingerprint_profiles` 的创建 / 查询 / 校验
- 任务与 `fingerprint_profile_id / version` 的绑定
- runner claim 阶段联表读取 active profile
- fake/lightpanda runner 结果中回显 `fingerprint_profile`
- inactive profile 在创建阶段直接拒绝
- 缺失 profile / stale version 在执行阶段安全降级为不注入

### 1.17 长期设计方向已明确
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
- runner trait / adapter interface 稳定化
- Lightpanda runner 从占位适配层向真实接入推进
- 查询分页 / limit / 控量
- 更完整的任务控制（尤其是 running cancel）
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

当前项目**已经完成最小可运行原型，并进入 `LightpandaRunner` 最小真实执行 V1 阶段**：具备任务创建、排队、执行、状态流转、重试、取消、运行历史、执行日志和状态摘要能力，同时已支持通过本地 `lightpanda fetch <url>` 执行真实页面访问；下一阶段重点是**打磨真实执行链路稳定性、增强查询控制与继续完善控制面**。

---

## 6. 维护规则

后续每次推进时，优先同步更新：
- **已实现**：只有真正落地的功能才能写进来
- **正在做**：只写当前阶段真实推进重点
- **未来将实现**：只保留中长期确定方向，避免空泛堆砌
- **更新记录**：每实现一个功能，都要在文档开头新增一条带 **年月日时分秒** 的记录

- **2026.3.29-00:06:00** 完成了 **queue claim / durable queue 下一步方案文档**，新增 `QUEUE_CLAIM_PLAN.md`，明确将任务执行权收回 SQLite 原子 claim、让内存队列降级为唤醒提示层，并给出下一阶段的最小实现顺序。

- **2026.3.29-00:12:00** 完成了 **DB-first claim 第一版落地**，worker 执行入口从“先 pop 内存队列再补查数据库”推进到“直接从 SQLite 原子 claim `queued` 任务并创建对应 run”，同时补了一条回归测试，验证即便内存队列条目缺失，DB 中的 queued 任务仍能被执行。

- **2026.3.29-01:13:00** 完成了 **runner_id / stale-running reclaim 最小实现**，为 tasks 增加 `runner_id` 持有者字段，让 DB-first claim 在抢占时写入执行者标识；同时新增 stale-running reclaim 逻辑，可将超时未收尾的 `running` 任务回收为 `queued` 并将悬挂 run 标记为失败，还补了一条集成测试覆盖该回收链路。

- **2026.3.29-08:45:00** 完成了 **heartbeat_at / lease-style reclaim 最小实现**，为 running 任务增加 `heartbeat_at` 字段与执行期心跳刷新逻辑，让 reclaim 优先基于 `heartbeat_at` 而不是仅靠 `started_at` 粗判；同时补了一条集成测试，验证带新鲜 heartbeat 的 running 任务不会被误回收。

- **2026.3.29-09:18:00** 完成了 **DB-first claim 后的队列语义收口第一轮**，修复 `health/status` 仍读取内存队列长度导致 `queue_len` 失真的问题，并补齐 queued cancel 在内存队列条目缺失时仍应按 DB 状态成功取消的回归测试，降低内存队列从真相源降级后的残余语义冲突。

- **2026.3.29-09:59:00** 完成了 **内存队列降级收口第一轮**，移除 `create / retry / reclaim / cancel` 对内存队列的真实语义依赖，让内存队列不再参与任务状态判断与执行正确性，仅作为兼容层保留；同时清理了新增测试中的局部 warning。
