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

1. **补一轮能力清单文档**
2. **再做一次轻量性能复盘**
3. **继续设计更真实的匿名性/地区校验链**
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

- **DB-first claim / reclaim 并发收口第二版已落地**，当前 claim 已切到单条原子抢占链，worker 空闲时也会指数退避，减少无任务空转与并发抢占抖动；但更深层的 retry / reclaim / cancel 三方竞争、claim 原子性极限验证和退避策略精细化仍需继续补齐。

- **DB-first claim / reclaim 并发收口第三版已落地**，当前 reclaim 不会再误回收没有 `runner_id` 的半脏 running 任务，`running` 状态下的 retry 也已收口为明确的 `409 CONFLICT`；调度内核的一致性比前几轮明显更稳。

- **代理池 V1 骨架已落地**，当前已具备 `proxies` 管理接口、任务级 `network_policy_json` 持久化，以及 runner 执行前的最小代理解析能力；但健康回写、冷却、粘性会话与更细粒度选择策略仍未完成。

- **代理健康回写第一版已落地**，当前执行链会按成功/失败/超时更新代理的成功计数、失败计数、最近使用时间、最近检查时间与冷却截止时间；但冷却时长、评分衰减、provider/sticky 选择仍较粗糙。

- **代理选择策略第一版增强已落地**，当前代理解析已支持 `provider` 过滤、`cooldown_until` 过滤和最小版 `sticky_session` 复用；但 sticky 目前仍是基于历史任务结果 JSON 回溯，后续应考虑独立映射结构与更细评分衰减。

- **代理观测面增强与 smoke test 第一版已落地**，当前已可从任务详情和 `/status` 直接看到代理命中信息与解析状态，并能通过 `POST /proxies/:id/smoke` 对单个代理做最小 TCP 连通性探测；但还没有更高级的 HTTP 层验证、匿名性校验与批量巡检机制。

- **sticky/provider 正式映射结构第一版已落地**，当前 `sticky_session` 已通过 `proxy_session_bindings` 表维护绑定，不再依赖历史任务结果 JSON 回溯；后续重点应转向 `/status` 聚合成本、warning 清理以及更真实的代理协议验证。

- **HTTP/代理协议层 smoke test 第一版已落地**，当前 smoke test 已不再只验证 TCP 端口可达，而会尝试发送 HTTP CONNECT 并判断代理响应是否像样；但仍未覆盖 HTTPS 上游连通性、匿名性/IP 地区校验与批量巡检。

- **lease TTL / reclaim / worker backoff 再收口已落地**，当前 stale running 回收已经进一步 DB-first 化，worker 空闲退避也加入了轻量 jitter 与 error backoff 上限；调度内核的竞争窗口与同步抖动都比前一轮更稳。

- **环境变量与状态暴露文档已收口**，当前 worker backoff / heartbeat / reclaim / claim retry 相关环境变量，以及代理验证与 smoke 结果字段的暴露口径已经整理清楚；后续更适合转向能力清单与更真实的匿名性/地区校验链。

- **能力清单文档已补齐**，当前系统已明确区分：调度控制面、fingerprint、proxy pool、sticky binding、proxy verification、状态观测，以及仍属于 V1/临时方案的部分。

- **更真实的匿名性/地区校验链设计已起草**，已明确下一步应引入外部 probe endpoint、出口国家/地区回显、geo match 判定，以及独立于 smoke 的 `verify` 慢路径。

- **当前阶段总结文档已收口**，`STAGE_SUMMARY_2026-03-31.md` 已整理出本阶段能力、风险、性能判断、代表性提交与下一阶段主线。

- **API / 运维文档第一版已补齐**，当前 endpoint surface、smoke 返回字段、proxy 持久化验证信号、runner 调参项与当前运维建议已集中整理进 `docs/api-ops.md`。

- **batch verify / 定期巡检方案已起草**，建议下一步不要做同步大循环，而是把单代理 `verify` 复用为 `verify_proxy` 任务，再由 `POST /proxies/verify-batch` 负责批量投递。

- **verify / selection / batch verify 说明文档已补齐**，当前 smoke 与 verify 分工、proxy 选择优先级、sticky 行为和 batch verify 设计口径已集中整理进 `docs/proxy-verification-reference.md`。


- **巡检 V1 已进入成型状态**：`verify_proxy` task kind、`POST /proxies/verify-batch`、按 stale/timeout/recent-use/failed-only/provider-cap 的筛选策略、provider 级批次 summary、`batch_id`、`verify_batches` 落库，以及 `GET /proxies/verify-batch` / `GET /proxies/verify-batch/:id` 批次查询都已打通；批次详情还能回看 `queued/running/succeeded/failed` 计数与派生状态。

- **巡检结果已开始真正反哺代理选择**：selection 当前会明确优先 fresh verified / geo-match verified 的代理，并对 recent verify failed、missing verify、stale verify 做后排处理；对应回归已覆盖 fresh-vs-stale、ok-vs-failed、geo-match-vs-smoke-only、verified-vs-missing-verify 四类核心口径。

- **代理选择策略层抽象已起步**：selection 规则已经开始从 engine 中拆出，当前策略模块已暴露 `ProxySelectionTier`、`ProxySelectionRule`、`current_proxy_selection_rules()`、`proxy_selection_base_where_sql()`、`proxy_selection_order_sql()`，engine 已开始直接复用这层规则来源。

- **代理选择策略层第一版已初步成型**：策略模块当前已承载 selection 的 tier/rule 定义、base where、order sql、resolved/unresolved/resolved_sticky 解析状态口径，以及 resolved_proxy JSON 结果表达；engine 已开始直接复用这些能力。

- **代理选择已开始纳入长期历史权重**：selection 当前除了吃即时 verify / geo / stale / missing 信号，也开始按 `success_count` / `failure_count` 的长期对比，对长期失败偏多的代理做额外后排处理。

- **代理选择已开始纳入 provider 长期稳定性**：selection 当前除了吃代理个体的长期成功/失败历史，也开始按 provider 维度聚合成功/失败记录，对长期整体不稳的 provider 做额外后排处理。

- **代理选择已开始纳入时间衰减型近期失败惩罚**：selection 当前会根据失败发生的时间远近，对更近期的失败给出更重惩罚，对更早的失败给出更轻或不额外惩罚。

- **代理选择已开始纳入 provider × region 维度的近期失败聚簇惩罚**：selection 当前会识别同一 provider 在同一 region 内的近期集中失败现象，并对该局部组合做额外后排处理。

- **代理选择策略层参数化第一步已打通**：selection 当前已经具备 `ProxySelectionTuning` 默认参数结构，规则模板会通过 tuning 参数生成实际执行口径，策略层开始真正进入“参数驱动规则”的阶段。

- **代理选择 tuning 已具备可切换/可注入入口**：selection 当前默认参数除了代码内默认值外，也已支持通过环境变量覆盖加载，执行链开始具备最小可行的策略配置切换入口。

- **代理选择已具备 trust score 起点**：策略层当前已能把多条正负信号收敛为统一 trust score SQL 表达，为后续把 selection 从多段规则叠加推进到更统一的 trust/risk score 模型打下基础。

- **稳定性清扫已开始处理 panic 风险点**：当前已先对 memory queue 与 lightpanda runner 中的 poisoned mutex 锁处理做加固，不再在这些点直接 `expect(...)` panic，而是改为尽量恢复内部状态继续工作。

- **trust score 接主链已补最小回归验证**：当前已新增直接排序层面的回归，用来验证在 trust score 规则下，更健康的代理能够压过 raw score 更高但状态更差的代理。

- **trust score 核心化继续推进**：当前主排序已不再只是简单按 raw score 兜底，而是开始把 `score` 更明确地纳入 trust score 主排序表达，进一步收敛 selection 语义。

- **第二轮稳定性清扫继续推进**：当前已进一步加固 lightpanda runner 中的 env lock poisoned 处理，避免该类锁异常直接触发 `expect(...)` panic。
