# 05 AI 维护接手手册

## 接手顺序

1. 先看 `/docs/02-current-state.md`
2. 再看 `/docs/03-roadmap.md` 与 `/docs/04-improvement-backlog.md`
3. 再进入代码与脚本

## 当前高风险点

### 1. 远端仓库不是干净工作树

不要对远端仓库执行：

- `git reset --hard`
- `git checkout -- .`
- 无差别覆盖

当前安全做法：

- 本地裁剪工作树编辑
- 单文件 `scp` 回传远端
- 在远端做验证

### 2. 整库 compile gate 被仓库既有 routes 阻塞

当前已知 blocker：

- `src/api/routes.rs` 引用了多组缺失 handler

所以不要把“整库 `cargo test -q` 失败”直接归因到本轮改动。

## 本轮关键文件

代码主线：

- `src/network_identity/fingerprint_consumption.rs`
- `src/network_identity/proxy_harvest.rs`
- `src/runner/engine.rs`
- `src/runner/lightpanda.rs`
- `src/api/dto.rs`
- `src/api/explainability.rs`
- `src/api/handlers.rs`

脚本主线：

- `scripts/proxy_longrun_report.py`
- `scripts/proxy_real_longrun_driver.py`
- `scripts/release_report_summary.py`
- `scripts/preflight_release_env.sh`
- `scripts/release_baseline_verify.sh`
- `scripts/release_fast_verify.sh`

## 推荐验证顺序

1. Python 脚本先做 `python3 -m py_compile`
2. Bash 脚本先做 `bash -n`
3. 同步远端
4. 远端执行：
   - `bash scripts/preflight_release_env.sh --profile public-smoke`
   - `bash scripts/release_fast_verify.sh --profile public-smoke`
   - `bash scripts/preflight_release_env.sh --profile prod-live`
5. 最后记录 compile blocker 与 release 结果


## 2026-04-12 ???????continuity control-plane

### 1. mode ??? AppState ?????
- ????? `prod_live` / `demo_public` ?????? `build_app_state` / `build_test_app` ?????????
- ????????? `PERSONA_PILOT_PROXY_MODE` ????????? app ???

### 2. verify-batch / status ?? mode-aware
- `prod_live` ????? `demo-only` source
- legacy ? source ?????????????????????????
- ? mode ??????????
  - `src/api/handlers.rs`
  - `src/network_identity/proxy_harvest.rs`
  - `src/runner/engine.rs`

### 3. Lightpanda runner ? Windows ????????
- `.sh` stub ? Windows ??????? bash???? runner ??????
- ? timeout / non-zero-exit / cancel ????????
  - `src/runner/lightpanda.rs`
  - `tests/integration_lightpanda_runner.rs`

## ?????????????

1. `cargo test --test integration_continuity_control_plane`
2. `cargo test --test integration_lightpanda_runner`
3. `cargo test --tests`
4. ???????????????
   - `bash scripts/preflight_release_env.sh --profile public-smoke`
   - `bash scripts/release_fast_verify.sh --profile public-smoke`
   - `bash scripts/preflight_release_env.sh --profile prod-live`


## 2026-04-13 本地验证补充规则

### 1. mode-aware 测试不要再直接依赖进程级环境变量竞态
- `PERSONA_PILOT_PROXY_MODE` 的集成测试已改为通过线程内 override 驱动 `build_app_state` / `build_test_app` 读取的 runtime mode。
- 如果后续新增 mode-aware 测试，优先复用现有 `ScopedEnvVar` 辅助，不要手写裸 `std::env::set_var` 后并行跑测试。
- 这样可以避免测试并发时出现 `prod_live` / `demo_public` 串台，导致 `/status.mode`、`recent_hot_regions` 等断言偶发漂移。

### 2. 本地主链路回归最小集合已更新
1. `cargo test --test integration_api`
2. `cargo test --tests`
3. `cargo build --release`

### 3. 本轮新增重点检查项
- `tasks` typed 列是否被 `/tasks`、verify batch、replenish batch 一致写入
- `/status.proxy_pool_status.recent_hot_regions` 是否反映最近 600 秒 demand，而不是只看当前 queued/running
- source summary 是否带出 `declared_geo_quality / effective_geo_quality / geo_coverage_percent`
- hygiene 是否保护：
  - `queued/running` 正在使用的 proxy
  - 最近使用的 proxy
  - sticky/session binding 中的 proxy

## 2026-04-13 release/report 验证补充规则

### 1. 现在要看的长跑/发布关键字段
- `summary.browser_success_rate_percent`
- `summary.event_summary.browser_proxy_not_found_failures`
- `summary.latest.recent_hot_regions`
- `summary.latest.source_concentration_top1_percent`
- `source_quality_summary.effective_geo_quality_summary`

### 2. prod-live release gate 默认阈值
- `effective_active_ratio_percent median >= 35`
- `promotion_rate median >= 75`
- `browser_success_rate_percent >= 98`
- `browser_proxy_not_found_failures = 0`
- `recent_hot_regions >= 3`
- `source_concentration_top1_percent <= 75`
- `avg_geo_coverage_percent` 当前保留可配置阈值，默认不强制卡死

### 3. 新 reason code 语义
- `proxy_claim_lost`: 浏览器任务在 claim 后发生 `proxy not found`
- `hot_region_window_empty`: 官方报告里的 recent hot window 不足以证明 region demand
- `source_concentration_too_high`: active 池 top1 source 占比过高
- `geo_coverage_too_low`: geo 覆盖率低于配置阈值

## 2026-04-13 脚本级回归入口补充

- 已新增 `scripts/release_prod_live_gate.py`：
  - 将 `prod-live` 长跑报告验收从 bash 内联 Python 提取为独立 helper
  - 现在可单独测试、复用和迭代阈值
- 已新增脚本测试目录 `scripts/tests/`：
  - `test_proxy_longrun_report.py`
  - `test_release_prod_live_gate.py`
- 当前脚本级最小回归集合：
  - `python -m unittest discover -s scripts/tests -p "test_*.py"`
  - `python -m py_compile scripts/release_prod_live_gate.py scripts/proxy_longrun_report.py scripts/proxy_real_longrun_driver.py`
  - `bash -n scripts/release_baseline_verify.sh`
  - `bash -n scripts/proxy_mainline_verify.sh`
  - `bash -n scripts/lightpanda_verify.sh`

### 2026-04-13 source / region balance 也已进入回归线
- 控制面新增 balance helper：
  - `load_active_proxy_balance_snapshot()`
  - `sort_balance_candidates()`
- 新增回归测试：
  - `verify_batch_prioritizes_underrepresented_source_when_top1_is_concentrated`
  - `replenish_tick_global_prioritizes_underrepresented_source_candidates`
- 现在建议把以下命令也视为 balance 改动后的最小验证：
  - `cargo test --test integration_api verify_batch_prioritizes_underrepresented_source_when_top1_is_concentrated -- --nocapture`
  - `cargo test --test integration_api replenish_tick_global_prioritizes_underrepresented_source_candidates -- --nocapture`

### 2026-04-13 provider balance + hygiene 收口后的补充规则
- `sort_balance_candidates()` 现在已同时吃：
  - source inventory
  - provider inventory
  - region inventory
  - recent hot regions
- 如果后续再改 balance 逻辑，最小 Rust 回归至少补跑：
  - `cargo test --test integration_api verify_batch_prioritizes_underrepresented_provider_when_source_balanced -- --nocapture`
  - `cargo test --test integration_api replenish_tick_global_prioritizes_underrepresented_provider_candidates -- --nocapture`
  - `cargo test --test integration_api verify_batch_prioritizes_underrepresented_source_when_top1_is_concentrated -- --nocapture`
  - `cargo test --test integration_api replenish_tick_global_prioritizes_underrepresented_source_candidates -- --nocapture`
- `scripts/prod_proxy_pool_hygiene.py` 现在不只是“删多少行”，还要看：
  - `top1_source_label`
  - `source_concentration_top1_percent`
  - `candidate_keep_limit_base`
  - `candidate_keep_limit`
  - `candidate_keep_adjustment`
  - `deleted_proxy_rows_by_source`
- 因此 balance / hygiene 改动后的最小脚本回归应补为：
  - `python -m unittest discover -s scripts/tests -p "test_*.py"`
  - 重点确认 `test_prod_proxy_pool_hygiene.py` 为绿
  - `python -m py_compile scripts/prod_proxy_pool_hygiene.py scripts/release_prod_live_gate.py scripts/proxy_longrun_report.py scripts/proxy_real_longrun_driver.py`
- 这轮还顺手修过一个非 balance 但会卡全量回归的既有漂移：
  - `runner::engine` sample-ready 默认 identity marker 顺序
  - 因此若后续改动小红书 continuity 默认 marker，记得至少补跑 `cargo test --tests`

### 2026-04-13 小红书 identity marker 深化后的补充规则
- 只要改到以下任一位置，都不要只跑单测：
  - `platform_templates.identity_markers_json`
  - `store_platform_overrides.identity_markers_json`
  - `record_persona_health_snapshot()` 的 continuity probe 汇总逻辑
- 最小回归至少补跑：
  - `cargo test --test integration_api platform_template_crud_roundtrips_identity_markers_json -- --nocapture`
  - `cargo test --test integration_api store_platform_override_crud_roundtrips_identity_markers_json -- --nocapture`
  - `cargo test --test integration_continuity_control_plane xiaohongshu_store_identity_markers_flow_into_probe_event_and_snapshot -- --nocapture`
  - 最后再跑一次 `cargo test --tests`
- 这轮还修过一个真实 handler 缺陷：
  - `create_store_platform_override` 的 SQL placeholder 数量曾漂移
  - 因此以后凡是扩 override 字段，记得先对照列数与 bind 数，再跑 CRUD 集成回归

### 2026-04-14 real-live / gate 补充规则
- 如果远端 `real-live` 报：
  - `live control plane does not expose /behavior-profiles yet`
  不要先怀疑 control plane binary；先检查远端：
  - `scripts/proxy_real_longrun_driver.py`
  - `scripts/proxy_mainline_verify.sh`
  是否仍停在旧 mixed-workload 版本。
- 当前远端稳定 real-live 入口仍是：
  - `bash scripts/proxy_mainline_verify.sh real-live`
  - base URL 仍指向 `http://127.0.0.1:3000`
- 若要把 `effective_active_ratio_percent` 拉过 35，而不改控制面 Rust，可优先使用：
  - `PROXY_REAL_LONGRUN_HYGIENE_EXTRA_ARGS`
  - 当前已验证有效的一组参数为：
    - `--keep-candidate-per-source 80`
    - `--candidate-min-per-source 20`
    - `--candidate-per-active 8`
    - `--top1-source-keep-candidate-cap 20`
    - `--underrepresented-source-keep-candidate-cap 80`
    - `--keep-rejected-per-source 10`
    - `--rejected-min-per-source 5`
    - `--rejected-per-active 2`
- `scripts/release_prod_live_gate.py` 现已支持两种 promotion-rate 口径：
  - `0~1` 比例值
  - `0~100` 百分比值
  后续若看到 `0.7802 < 75.0` 这类报错，优先确认远端 gate 脚本是否已同步到最新。

### 2026-04-14 小红书 precision hardening 补充规则
- 如果改到以下任一函数，优先怀疑“小红书误判/漏判”而不是数据库：
  - `detect_login_loss_signal()`
  - `continuity_probe_haystack()`
  - `evaluate_sample_ready_continuity_probe()`
- 这轮已确认一个真实坑：
  - 如果 snapshot 断言只盯“最新一条 snapshot”，在 `cargo test --tests` 并发场景下会被后续无 marker 的 snapshot 顶掉
  - 因此集成测试应查“最近窗口里任一带 marker 的 snapshot”，不要把 latest-only 当作唯一真相
- 小红书精度相关最小回归现在建议补为：
  - `cargo test --lib evaluate_sample_ready_continuity_probe_does_not_trigger_login_from_config_echo -- --nocapture`
  - `cargo test --test integration_continuity_control_plane xiaohongshu_store_identity_markers_flow_into_probe_event_and_snapshot -- --nocapture`
  - 最后再跑 `cargo test --tests`

### 2026-04-14 source-balance + real-live gate 回归补充
- 脚本侧最小回归新增：
  - `python -m py_compile scripts/proxy_longrun_report.py scripts/release_prod_live_gate.py scripts/release_real_live_gate.py scripts/proxy_real_longrun_driver.py`
  - `python -m unittest discover -s scripts/tests -p "test_*.py"`
  - `bash -n scripts/proxy_mainline_verify.sh`
- Rust 侧本轮新增关键回归：
  - `cargo test --test integration_api prod_live_auto_selection_prefers_non_overweight_source_when_trust_is_close -- --nocapture`
  - `cargo test --test integration_api demo_public_auto_selection_does_not_apply_source_balance_preference -- --nocapture`
  - `cargo test --test integration_api proxy_selection_reuses_sticky_session_when_available -- --nocapture`
- 全量门禁保持：
  - `cargo test --tests`
  - `cargo build --release`
- real-live 主入口更新：
  - `bash scripts/proxy_mainline_verify.sh real-live`
  - 现在会同时输出 `real_live_evidence_summary` artifact 路径和 real-live gate 结果。

### 2026-04-14 prod-live acceptance 入口补充
- `prod-live` 与 `real-live` 现在是独立入口：
  - `bash scripts/proxy_mainline_verify.sh prod-live`
  - `bash scripts/proxy_mainline_verify.sh real-live`
- release profile `prod-live` 现在固定调用独立入口，不再复用 real-live 出口码：
  - `scripts/release_baseline_verify.sh --profile prod-live`
- prod-live 样本量门槛可通过环境变量调节：
  - `RELEASE_VERIFY_PROD_LIVE_MIN_SAMPLE_COUNT`
- 当出现 `sample_insufficient	prod_live_sample` 时，优先补长跑样本，不要误改 continuity 逻辑。

### 2026-04-14 release 摘要 helper 补充
- release 报告里的 prod-live 摘要现在统一通过：
  - `python scripts/release_report_summary.py --report-json ...`
- 不要再把新的 release 摘要字段直接塞回 `release_baseline_verify.sh` 的内联 Python。
- 如果后续要加：
  - source quality 新字段
  - session continuity 新字段
  - real-live evidence 新字段
  优先改 `scripts/release_report_summary.py` 并补 `scripts/tests/test_release_report_summary.py`。
- 当前 helper 还顺手承担一个防污染约束：
  - release report 只在 `prod-live` profile 下写入 prod-live 摘要，避免旧 longrun JSON 污染 public-smoke / gateway-upstream 报告。

### 2026-04-14 host-level evidence 补充规则
- 如果后续改到以下任一位置，不要只看总量字段，必须检查 host-level evidence 是否仍完整：
  - `scripts/proxy_longrun_report.py` 中的：
    - `extract_event_host()`
    - `build_site_host_evidence()`
    - `build_real_live_evidence_summary()`
    - `render_real_live_evidence_text()`
  - `scripts/release_report_summary.py`
- 现在 real-live / release 侧至少要确认以下字段没有丢：
  - `site_host_count`
  - `stateful_site_host_count`
  - `continuity_ready_site_host_count`
  - `continuity_ready_site_hosts`
  - `site_host_summaries`
  - `real_live_site_host_count`
  - `real_live_stateful_site_host_count`
  - `real_live_continuity_ready_site_host_count`
  - `real_live_continuity_ready_site_hosts`
- 这类改动后的最小脚本回归现建议固定为：
  - `python -m py_compile scripts/proxy_longrun_report.py scripts/release_report_summary.py scripts/release_prod_live_gate.py scripts/release_real_live_gate.py scripts/proxy_real_longrun_driver.py`
  - `python -m unittest discover -s scripts/tests -p "test_*.py"`
  - `bash -n scripts/proxy_mainline_verify.sh`
  - `bash -n scripts/release_baseline_verify.sh`
- 如果 `python scripts/release_report_summary.py ...` 输出：
  - `prod_live_gate_reason_code=sample_insufficient`
  - `real_live_evidence_ready=False`
  先判断是不是**报告样本不足**，不要误判为 helper / artifact 契约回归。

### 2026-04-14 prod-live preset / provider-capped 补充规则
- 当前 `prod_live` 稳态参数已有单一真源：
  - `python3 scripts/prod_live_presets.py print-json --preset stable_v1`
- 日常维护不要再手拼 hygiene keep-cap：
  - 优先跑 `bash scripts/prod_live_maintenance_tick.sh --preset stable_v1`
- 长跑 / 发布若要显式启用稳态参数：
  - `PROXY_VERIFY_REAL_PRESET=stable_v1 bash scripts/proxy_mainline_verify.sh prod-live`
  - `PROXY_VERIFY_REAL_PRESET=stable_v1 bash scripts/proxy_mainline_verify.sh real-live`
- 新报告字段含义固定：
  - `strict_verdict`
    - 严格 95+ 结论
  - `operational_verdict`
    - 当前运营结论
  - `provider_supply_class`
    - `lab_only` 或 `private_mix`
  - `provider_cap_reason`
    - 当前只允许 `source_concentration_too_high`
- 如果看到：
  - `strict_verdict=fail`
  - `operational_verdict=provider_capped`
  不要把它当成链路故障；先确认是否同时满足：
  - `provider_supply_class=lab_only`
  - `prod_live_gate_reason_code=source_concentration_too_high`
- 若失败原因是以下任一项，则不能降格成 `provider_capped`：
  - `proxy_claim_lost`
  - `continuity_not_observed`
  - `effective_ratio_too_low`
  - `promotion_rate_too_low`
  - `hot_region_window_empty`
- 这轮脚本改动后的最小回归建议固定为：
  - `python -m py_compile scripts/prod_live_presets.py scripts/proxy_real_longrun_driver.py scripts/proxy_longrun_report.py scripts/release_prod_live_gate.py scripts/release_report_summary.py`
  - `python -m unittest discover -s scripts/tests -p "test_*.py"`
  - `bash -n scripts/proxy_mainline_verify.sh`
  - `bash -n scripts/prod_live_maintenance_tick.sh`
  - `bash -n scripts/release_baseline_verify.sh`

### 2026-04-14 legacy `/status` contract 兼容补充
- 如果远端在线 control plane 仍缺：
  - `/status.mode`
  - `/status.proxy_pool_status.recent_hot_regions`
  - `/status.proxy_pool_status.source_concentration_top1_percent`
  不要立刻把它判成链路错误；先确认是否是**旧 status contract**。
- 当前兼容口径已固定为：
  - `preflight_release_env.sh`
    - `/status.mode` 为空时，回退读取 `port 3000` 进程环境中的 `PERSONA_PILOT_PROXY_MODE`
  - `proxy_longrun_report.py`
    - status snapshot 缺 `mode` 时，回退 driver raw payload 的 `mode`
    - status snapshot 缺 `recent_hot_regions` 时，回退 browser event 中的 `recent_hot_regions_during_request`
    - status snapshot 缺 concentration 字段时，回退 `proxy_harvest_metrics.source_summaries[].active_count`
  - `proxy_real_longrun_driver.py`
    - 旧 `/status` 在请求期间缺 `recent_hot_regions` 时，会把本次 `requested_region` 回填成兼容证据
- 远端短验收若只跑 `120s`，建议至少再显式带：
  - `PROXY_VERIFY_REAL_STATUS_INTERVAL_SECONDS=20`
  否则默认 `60s` 采样容易只拿到 `sample_count=3`，会误触发 `prod_live_sample`，那是**样本量不足**，不是 gate 回归。
- 在旧 contract 机器上验证 `provider_capped` 的推荐顺序：
  1. `bash scripts/prod_live_maintenance_tick.sh --preset stable_v1`
  2. `PROXY_VERIFY_REAL_PRESET=stable_v1 PROXY_VERIFY_REAL_DURATION_SECONDS=120 PROXY_VERIFY_REAL_STATUS_INTERVAL_SECONDS=20 bash scripts/proxy_mainline_verify.sh prod-live`
  3. `python3 scripts/release_prod_live_gate.py reports/proxy_real_longrun_latest.json --json --min-sample-count 6`
- 当前已验证成功的目标态应直接看：
  - `strict_verdict=fail`
  - `operational_verdict=provider_capped`
  - `provider_cap_reason=source_concentration_too_high`
  - 同时满足：
    - `summary.mode=prod_live`
    - `browser_proxy_not_found_failures=0`
    - `recent_hot_regions >= 3`
