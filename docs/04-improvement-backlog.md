# 04 改进 Backlog

## P0

### B-014 收口 continuity control-plane compile gap
- 当前状态：
  - 已完成本地主线收口
  - `cargo check -q` 通过
  - `cargo test --no-run -q --lib` 通过
  - `cargo test --no-run -q` 通过
  - `cargo build --release -q` 通过
  - `cargo test --test integration_continuity_control_plane -- --nocapture` 通过（`8/8`）
- 本轮完成：
  - continuity schema / DTO / handlers / runner 最小兼容面已回到主线
  - task terminal result 现在会稳定写入 `continuity_check_result`
  - legacy slash-escaped JSON template fields 现在会兼容解析，不再把 `continuity_checks_json` 静默降成 `[]`
- 结论：
  - `B-014` 不再阻塞本地工程推进
  - continuity 相关后续工作转入 `B-010` 兼容债和 `B-011/B-012` 平台扩展
- 验收：
  - 已完成

### B-001 远端验证 release/profile 脚本语法与行为
- 范围：
  - `scripts/preflight_release_env.sh`
  - `scripts/release_baseline_verify.sh`
  - `scripts/release_fast_verify.sh`
- 当前结果：
  - `public-smoke` 已通过
  - `prod-live` 已通过
- 后续：
  - `gateway-upstream` 还需要补独立稳定验收

### B-002 用远端实跑确认 verify batch 的 mode 过滤
- 验证 `prod_live` 下 demo/public source 不会被 verify batch 选入
- 验证 `demo_public` 下 demo source 仍可走烟雾链

### B-003 prod-live acceptance gate 已完成第一阶段
- 已完成：
  - `prod-live` preflight pass
  - `prod-live` release_fast_verify pass
  - `mode == prod_live`
  - active pool observed
  - continuity observed
- 后续继续：
  - 补标准 30 分钟长跑，不只保留 180 秒快速验收
  - 把 prod-live 的固定 summary artifact 纳入 release 阅读路径

### B-009 代理质量 runtime 继续提分
- 本轮已完成第一阶段实装：
  - 新增 `scripts/prod_proxy_pool_hygiene.py`
  - 远端已删除低价值 `candidate/candidate_rejected` `16457` 条
  - 已 quarantine `github_monosans_http`
  - 已同步 prod source metadata `4` 条
  - 已重启当前运行中的 debug binary 让内存池吃到新 DB
- 当前 prod-live 实时结果已提升到：
  - `effective_active_ratio_percent ≈ 30.56`
  - `promotion_rate ≈ 67.74%`
  - `reject_rate ≈ 32.26%`
- 当前剩余提分方向：
  - 降低 `github_speedx_http` 单源集中度
  - 把 `expected_geo_quality == unknown` 的 source 继续分层，不再只按“能用”评分
  - 补一轮新的 30 分钟 prod-live 长跑，把本轮池治理结果固化进长期报告
- 本轮还补了两层错误模式门禁，后续不再把 demo config 误带入 prod-live：
  - `proxy_real_longrun_driver.py` 拒绝 `prod_live + repo demo config`
  - `proxy_mainline_verify.sh` 拒绝 `prod_live + PROXY_VERIFY_REAL_ALLOW_DEMO=1`
- 本轮第二阶段已继续完成：
  - 新增 `scripts/prod_proxy_geo_enrich.py`
  - active region 已从 `0` 提升到 `13`
  - `proxy_real_longrun_driver.py` 已支持 `--auto-browser-regions-from-db`
  - longrun report 已能输出：
    - `browser_requested_regions`
    - `browser_hot_regions_observed`
    - `geo_enrich_active_regions_after_last`
  - `prod_proxy_pool_hygiene.py` 已避免把已落地的 `host_geo_inferred` 再覆盖回 `unknown`
- 当前剩余缺口已进一步缩到：
  - `/status` 的 latest `hot_regions` 仍受 queued/running 时间窗限制，最新快照不一定稳定显示；如需 UI 常亮，需要后续再补 recent-demand 视窗或独立 demand 缓存
  - source concentration 仍偏高，主活跃池还是 `github_speedx_http`
  - 还没把多 source / 多 region 的均衡度直接写进 release gate

### B-013 continuity metrics / observability ????
- ????
  - `/status.identity_session_metrics` ?????? `result_json json_extract` ??
  - task terminal write path ?? continuity ?????
  - task / run / status ?? `continuity_timing`?restore / persist_db / snapshot / total_overhead?
- ?????
  - ??? task ?????????? session ??? task ???????? rollup/cache
  - dashboard ?? continuity overhead ??????

## 2026-04-15 执行板补充

- 下一阶段执行顺序固定为：
  1. `B-003 stable_v1 30min x2 acceptance`
  2. `B-001 gateway-upstream independent acceptance`
  3. behavior active-path promotion
  4. `B-012` 第二平台 continuity 模板
- 本阶段不建议同时扩 UI 产品化、多平台大面积铺开、以及额外 explainability 重写。

## P1

### B-004 补 integration / unit tests
- status mode + ratio 字段
- verify batch 的 source eligibility
- legacy alias `device_memory`
- `prod_live` fingerprint profile 必填约束

### B-005 summary artifact 第一阶段已落地
- 已完成固定 artifact 输出：
  - `source quality summary`
  - `session continuity summary`
- 后续继续：
  - release 报告直接挂出 artifact 路径
  - dashboard / longrun / release 三处统一引用同一组摘要

### B-006 release 失败细分继续扩展
- 当前已引入主 reason code 方向，后续可继续细分：
  - `gateway_no_token_failed`
  - `browser_contract_failed`
  - `prod_live_driver_failed`

### B-007 public-smoke release chain is now green
- 已完成：public-smoke preflight pass
- 已完成：public-smoke release_fast_verify pass
- 后续：继续保持与 `prod-live` 的口径拆分，不再让 public/demo 结果给 production-live 背书

### B-008 fix remaining remote dirty-branch compile drift
- handlers/core.rs: DTO / tuple row drift
- runner/engine.rs: RunnerSessionContext 新字段未补齐
- db/schema.rs: `ALL_SCHEMA_SQL` 长度与实际项数漂移


### B-010 continuity control-plane warning / compatibility debt
- ????
  - ?? `ResolvedNetworkPolicyModel.region_anchor`
  - ?? `ResolvedPersonaBundle.persona_status`
  - ?? persona lookup ????? `network_policy_region_anchor`
  - ?? `HarvestSourceRow` ??? harvest ?????? metadata ??
  - ???? harvest source ?? SQL ??????
- ????
  - ?? `cargo test --tests` ??
  - ?? `cargo build --release` ??
- ???
  - mode-aware legacy source ???????????
  - `prod_live` ? legacy / demo source ?????????????????????

### B-011 ??? sample-ready ??? continuity checks ??
- ?? 5 ? continuity baseline signals
- ?????????? 6 ? gate ??
- ? Telegram / API ???? persona ????????

### B-012 persona health ??????
- ??? `/status` ??? persona ?? 30d / 90d ??
- ? `heartbeat_failed -> degraded` ? `continuity_broken -> frozen` ????????
- ?? `persona_30d_survival_ratio` / `continuity_break_rate` / `manual_gate_resolution_time`

### B-013 Lightpanda Windows test launcher ?????
- ?? Windows `.sh` ???????????????????
  - ??????????? stub launcher
  - ??? Windows cancel ? `taskkill` best-effort ????????????


## 2026-04-13 补充：correctness / status 主链路已本地收口

- 已完成并本地验证：
  - `tasks.proxy_id / requested_region / proxy_mode` typed 列持久化
  - `verify_batch` / `replenish` 的事务内显式 claim
  - `/status.proxy_pool_status.recent_hot_regions*` 与 source concentration 指标
  - source summary 的 `declared_geo_quality / effective_geo_quality / geo_coverage_percent`
  - hygiene 对 in-flight / recent-used / sticky-bound proxy 的保护
- 已补回归验证：
  - `cargo test --test integration_api` 全绿
  - `cargo test --tests` 全绿
  - `cargo build --release` 通过
- 因此 backlog 焦点从“本地主链路 correctness”切到“远端 prod-live 95+ 验收”：
  - 远端 30 分钟 `prod-live` 长跑
  - `source_concentration_top1_percent <= 75%`
  - `effective_active_ratio_percent median >= 35%`
  - `promotion_rate median >= 75%`
  - `browser_proxy_not_found_failures = 0`
  - `stateful_continuity_observed = true`
- 新增后续优先项：
  - 把 `recent_hot_regions`、source concentration、effective geo quality summary 直接挂到 release artifact
  - 把 `proxy_claim_lost / hot_region_window_empty / source_concentration_too_high / geo_coverage_too_low` 收口为稳定 reason code
  - 在远端引入至少 2 个独立 private/paid provider 后再评估代理质量是否可上 95+

## 2026-04-13 补充：release/report 95+ 门禁继续内生化

- 已完成：
  - longrun report 输出 `browser_success_rate_percent`
  - longrun / source artifact 输出 `effective_geo_quality_summary`
  - longrun report 输出 `browser_proxy_not_found_failures`
  - release gate 接入 `recent_hot_regions` / source concentration / proxy_claim_lost reason code
- 因此 backlog 再次收窄：
  - 本地剩余主要不是字段缺失，而是远端真实数据面能否稳定过更严格 gate
- 当前主要剩余项：
  - 远端 `prod-live` 30 分钟报告是否稳定达到新默认阈值
  - private/paid provider 是否达到至少 2 个独立来源
  - `avg_geo_coverage_percent` 的真实阈值是否需要从 0 提高到更严格门槛

## 2026-04-13 backlog refresh: continuity control-plane

### B-010 测试 / 兼容债收口
- 状态：**已基本完成**
- 本轮已完成：
  - trust-score 单测断言已与现行 SQL/runtime 权重对齐
    - `local_verify = 4`
    - `runner_verify = 3`
    - `imported/manual/backfill = -1`
  - `integration_api` 测试库路径从时间戳改为 `UUID`
  - SQLite 初始化补 `busy_timeout + WAL`
  - `cargo check / cargo build --release / cargo test --tests` 已恢复全绿
- 剩余只保留为“后续出现的新兼容回归”，不再继续追已消失的 warning

### B-011 小红书 `sample_ready` runtime evaluator + heartbeat probe + bootstrap
- 状态：**已完成第一阶段**
- 本轮已完成：
  - canonical 小红书模板幂等 bootstrap
  - `sample_ready` heartbeat 切换为 `extract_text`
  - `/dashboard` / `/notes` round-robin
  - 5 信号 continuity checks 运行时生效
  - probe 证据进入 `continuity_events` 与 `persona_health_snapshots`
  - manual gate 小红书高风险路径继续复用统一 6 分类
- 下一步：
  - 把同一套 runtime evaluator 复用到 Shopify / 独立站后台
  - 再补平台级 identity marker 精细化

### B-012 多平台 continuity 模板补齐
- 状态：**下一阶段主项**
- 范围：
  - Shopify / 独立站后台
  - Amazon / eBay
  - Walmart / TikTok Shop
- 目标：
  - 不分叉 schema
  - 同一 heartbeat / event / snapshot / manual gate 链路复用

### B-013 30d / 90d persona health 运营化
- 状态：**待开始**
- 范围：
  - `persona_30d_survival_ratio`
  - `continuity_break_rate`
  - `manual_gate_resolution_time`
  - 分 persona/store/platform 的趋势脚本与运营视图

## 2026-04-13 补充：source / region balance 已进入控制面主链

- 已完成：
  - verify batch 在相同 provider cap 之外，开始优先低库存 source
  - replenish global batch 开始优先低库存 source
  - recent hot regions 与 underrepresented region 已进入候选排序
- 因此下一层 backlog 更聚焦于：
  - 把 provider 维度的 concentration / inventory 也提升为一等调度条件
  - 把 hygiene 的 top1 source candidate keep cap 收紧策略进一步与 Rust 控制面口径对齐
  - 远端真实 prod-live 验证这套 balance 策略是否能压低 top1 source concentration

## 2026-04-13 补充：provider balance + hygiene top1 source 收紧已完成本地落地

- 已完成：
  - verify batch / replenish 的候选排序已接入 provider inventory
  - overweight provider 在有替代时会被自动降权
  - `prod_proxy_pool_hygiene.py` 已输出 top1 source concentration summary
  - dominant source 的 candidate keep cap 已可自动收紧
  - underrepresented non-top1 source 已可获得更宽松的 candidate 保留
  - 新增回归：
    - `verify_batch_prioritizes_underrepresented_provider_when_source_balanced`
    - `replenish_tick_global_prioritizes_underrepresented_provider_candidates`
    - `scripts/tests/test_prod_proxy_pool_hygiene.py`
- 这意味着 backlog 再次收窄：
  - 本地控制面 / 脚本层已经把 source + provider + region 三层 balance 基本接齐
  - 当前更值得做的是远端真实 `prod_live` 验证，而不是继续本地补排序分支
- 当前主要剩余项：
  - 远端 30 分钟 `prod_live` 长跑是否能把 `source_concentration_top1_percent` 压到目标线
  - hygiene 新 keep-cap 在真实 DB 上是否足够强；若不够，再继续加严 dominant source cap
  - 是否已经具备至少 2 个独立 private/paid provider；若没有，代理质量上限继续按 90 左右保守评估

## 2026-04-13 backlog refresh: xiaohongshu identity continuity closeout

### B-011 小红书 `sample_ready` runtime evaluator + heartbeat probe + bootstrap
- 状态：**已完成第二阶段，当前可视为小红书样板打透**
- 本轮新增完成：
  - `identity_markers_json` 已进入 platform template / store override 双层配置
  - store-level identity marker 已接入 resolve-persona -> heartbeat task payload -> continuity evaluator
  - `matched_identity_marker` 已进入 `continuity_events` 与 `persona_health_snapshots`
  - snapshot 汇总已支持在最近窗口中回填非空 identity marker
  - API 回归已覆盖：
    - platform template CRUD roundtrip
    - store platform override CRUD roundtrip
  - 集成回归已覆盖：
    - 小红书 store-level identity marker -> probe event -> snapshot
- 剩余只保留为“下一阶段精度加强”，不再属于主阻塞：
  - 店铺/账号名更细粒度 marker 治理
  - 跨平台复用同一 identity continuity 抽象
  - 若后续需要，再考虑 marker 优先级与冲突治理

### B-011-1 小红书 precision hardening
- 状态：**2026-04-14 已完成当前切片**
- 本轮新增完成：
  - login signal 检测已改为只看页面可见证据，不再吃配置回显假阳性
  - `configured_identity_markers / identity_markers_source` 已进入：
    - probe result
    - continuity events
    - persona health snapshots
  - `cargo test --tests` 下 snapshot 断言的时间窗竞争已通过测试 helper 收口
- 剩余仅保留治理类工作：
  - marker 命名规范
  - store/account marker 生命周期治理
  - 与 Shopify / 独立站后台共用一套 marker 审计约定

## 2026-04-14 backlog refresh: prod-live 真实门禁已缩到 source concentration

- 已完成：
  - 远端 `proxy_real_longrun_driver.py` 与 `proxy_mainline_verify.sh` 的 behavior-profile 兼容收口
  - `real-live` 主入口重新打通
  - stateful continuity 短跑稳定点亮
  - aggressive hygiene keep-cap 已证明可把：
    - `effective_active_ratio_percent` 拉到 **43.12**
    - `promotion_rate` 拉到 **78.02%**
  - `release_prod_live_gate.py` 的 promotion-rate 单位 bug 已修复
- 因此 backlog 再次收窄为：
  - **B-009 / 代理质量 runtime 继续提分** 当前唯一硬剩余主项是：
    - `source_concentration_top1_percent <= 75`
  - 当前实测：
    - `source_concentration_top1_percent = 87.79`
    - 最新 gate 失败 reason 已收口为：
      - `source_concentration_too_high`
- 下一步优先顺序：
  1. 接入至少 **2 个独立 private/paid provider**
  2. 或继续提升非 top1 source 的真实 promotion/active 供给
  3. 若没有新增供给，就不要再把代理质量 / 发布就绪度虚报到 95+

## 2026-04-14 backlog refresh: source balance + real-live evidence split delivered

### B-009 代理质量 runtime 继续提分
- 状态：**已完成本轮 runtime/source balance 切片**
- 已落地：
  - `prod_live` 自动选代理新增 source-balance 偏好
  - 过载 top1 source 在可替代且质量接近时会被降权
  - 若替代明显更差则保留 top1，并记录 fallback reason
  - sticky / explicit / continuity 绑定路径不受该偏好干扰
- 已新增回归：
  - `prod_live_auto_selection_prefers_non_overweight_source_when_trust_is_close`
  - `demo_public_auto_selection_does_not_apply_source_balance_preference`

### B-005 summary artifact 第一阶段已落地
- 状态：**已进入第二阶段并完成 real-live evidence artifact**
- 新增：
  - `real_live_evidence_summary_latest.txt`
  - `real_live_evidence_summary_latest.json`
- 三份主摘要现为：
  - `source_quality_summary`
  - `session_continuity_summary`
  - `real_live_evidence_summary`

### 新增：real-live 独立 gate
- 状态：**已完成**
- 新脚本：`scripts/release_real_live_gate.py`
- 判定主项：
  - browser success
  - stateful continuity observed
  - continuity chain observed
  - storage restore/persist evidence
  - no proxy_claim_lost
  - minimum sample volume
- 输出格式统一：`reason_code\tfailure_scope\tdetail`

### 新增：prod-live / real-live 主入口彻底解耦
- 状态：**已完成**
- 已落地：
  - `scripts/proxy_mainline_verify.sh prod-live`
  - `scripts/proxy_mainline_verify.sh real-live`
  两条入口现在分别对应不同 gate，不再复用同一出口码。
- 已落地：
  - `scripts/release_baseline_verify.sh --profile prod-live`
  现在固定走 `prod-live` 入口，不再通过 `real-live` 入口间接判定。
- 后续：
  - 继续用 `RELEASE_VERIFY_PROD_LIVE_MIN_SAMPLE_COUNT` 调整 prod-live 样本门槛
  - 保持 prod/live 与 real/live 的 reason_code 不互相污染

### 新增：release 摘要主链统一到 sidecar artifacts
- 状态：**已完成本地落地**
- 已落地：
  - `scripts/release_report_summary.py`
  - `release_baseline_verify.sh` 通过该 helper 统一引用：
    - `source_quality_summary`
    - `session_continuity_summary`
    - `real_live_evidence_summary`
- 已顺手修复：
  - 非 `prod-live` release profile 不再误带历史 prod-live 指标
- 后续：
  - 远端 release 报告也按同一 helper 同步
  - 如果后续要扩展 release 报告字段，优先补 helper 和其测试，不要回到 shell 内联解析

### 新增：real-live host-level evidence 已完成本地落地
- 状态：**已完成本地脚本与测试收口**
- 已落地：
  - `real_live_evidence_summary` 现已包含 host-level continuity 证据
  - release summary 现可直接读出：
    - `real_live_site_host_count`
    - `real_live_stateful_site_host_count`
    - `real_live_continuity_ready_site_host_count`
    - `real_live_continuity_ready_site_hosts`
- 已补回归：
  - `scripts/tests/test_proxy_longrun_report.py`
  - `scripts/tests/test_release_report_summary.py`
- 这意味着当前 backlog 已不再是“缺少 host-level 证据字段”，而是：
  1. 远端 `real-live` 30 分钟报告是否积累到足够 host 数和 continuity-ready host 数
  2. 是否有至少 1 个以上真实 stateful host 连续稳定出现 `auto_created -> auto_reused + storage restore`
  3. 是否把远端证据厚度从“能生成 artifact”推进到“artifact 内容足以背书 92+”

### 仍然不能虚报到 92+ 的剩余项
- `Dashboard / Operator 控制台`
  - 本轮按范围约束未做 UI 产品化，不应虚报为已收口
- `白名单实站验证成熟度`
  - 代码侧 artifact/gate 已补齐，但真实分数仍取决于远端持续样本，不应只因本地测试通过就报满分

### 新增：prod-live 稳态 preset 默认化 Phase 1 已完成
- 状态：**本地已完成，远端待同步验证**
- 已落地：
  - `scripts/prod_live_presets.py`
  - `PROXY_VERIFY_REAL_PRESET=legacy|stable_v1`
  - `scripts/prod_live_maintenance_tick.sh --preset <name>`
  - longrun / release artifact 新增：
    - `preset`
    - `provider_supply_class`
    - `strict_verdict`
    - `operational_verdict`
    - `provider_cap_reason`
- 已确认：
  - 严格 95+ gate 没有放松
  - 当前无 private/paid provider 时，只允许把
    `lab_only + source_concentration_too_high`
    降格为 `operational_verdict=provider_capped`
- 下一步剩余项：
  1. 同步远端并跑 `bash scripts/prod_live_maintenance_tick.sh --preset stable_v1`
  2. 远端跑 `PROXY_VERIFY_REAL_PRESET=stable_v1 bash scripts/proxy_mainline_verify.sh prod-live`
  3. 验证 release/report 中 `strict_verdict / operational_verdict / provider_supply_class` 已稳定落地
  4. 连续 2 次 30 分钟 `stable_v1 prod_live` 若仅剩 concentration 失败，再考虑 Phase 2 默认切换

### 新增：Phase 2 默认切换仍未开始
- 状态：**未开始**
- 保持现状：
  - 默认 preset 仍是 `legacy`
  - release / longrun / maintenance 可显式指定 `stable_v1`
- 进入 Phase 2 前置条件不变：
  - 连续 2 次 `30min prod_live stable_v1` 满足：
    - `effective_active_ratio_percent median >= 35`
    - `promotion_rate median >= 75%`
    - `browser_success_rate_percent >= 98`
    - `browser_proxy_not_found_failures = 0`
    - `recent_hot_regions >= 3`
    - `stateful_continuity_observed = true`
    - 严格 gate 若失败，仅允许 `source_concentration_too_high`

### 2026-04-14 补充：stable_v1 远端短验收已完成，Phase 1 从“本地完成”推进到“远端已验证”
- 已完成：
  - 远端 `bash scripts/prod_live_maintenance_tick.sh --preset stable_v1`
  - 远端 `120s` `stable_v1 prod-live` 短验收
  - 旧 `/status` contract 兼容修复：
    - `/status.mode` 缺失时 preflight 回退进程环境
    - report/gate 从 driver raw payload / browser events / source summaries 回填 `mode`、`recent_hot_regions`、`source_concentration`
- 远端最新短验收结果：
  - `summary.mode = prod_live`
  - `sample_count = 7`
  - `effective_active_ratio_percent median = 54.0`
  - `promotion_rate median = 88.51%`
  - `browser_success_rate_percent = 100.0`
  - `browser_proxy_not_found_failures = 0`
  - `recent_hot_regions = ["ap-southeast", "cn", "eu-west"]`
  - `strict_verdict = fail`
  - `operational_verdict = provider_capped`
  - `provider_cap_reason = source_concentration_too_high`
- 这意味着：
  - `stable_v1` Phase 1 的“默认化、产品化、报告化”已经不再只停留在本地
  - 当前 provider-capped 语义已在真实远端 `prod_live` 链路中被验证
- backlog 现继续收窄为：
  1. 连续 2 次 `30min stable_v1 prod-live` 验证只剩 concentration 失败
  2. 若无 private/paid provider，不再追求 95+ 严格通过，只继续保持 `provider_capped` 口径稳定
  3. 若后续要进 Phase 2 默认切换，先确认远端长跑样本量、比率、promotion、continuity 都稳定
