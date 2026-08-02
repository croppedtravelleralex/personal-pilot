# 56 边界收口与商业能力补齐

Updated: 2026-07-11 (Asia/Shanghai)

## 0. 本文定位

- **目标**：收录 `docs/49`–`docs/55` **未覆盖**、但横评/平台 live/商业竞品对比中已暴露的缺口，形成可执行 Task + AC。
- **边界**：仍是独立优化 track；不修改本机自用 `100% / 0% / green` 口径；无 raw artifact 不刷分。
- **上游**：战术/战略/广度/相干/行为/并发/度量见 `PLAN.md` §1；用户验收 Sprint 见 `docs/41-user-requirements-execution-plan.md`。
- **下游**：主要进 Wave 4；与 W1 并行的高杠杆项（T1、B1）可提前。
- **相对 W0**：`docs/49` A1/A3/C1 与 D1 主体已落地；本文不再把 Wave 0 当并行前置。

## 1. 证据基线（2026-07-11 复核）

| 编号 | 文件:行（约） | 已核实现状 | 缺口 |
| --- | --- | --- | --- |
| X1 | `environment_injector.go:321` | 仅 `Page.addScriptToEvaluateOnNewDocument` + 当前页 `Runtime.evaluate` | 跨域 iframe / sandbox 子文档无 hook；Worker 另见 `docs/50` C1 |
| X2 | `docs/41` / `docs/03` | CAPTCHA/SMS/Email 无真实 provider 凭证 smoke | 平台 live 遇挑战只能 `blocked_missing_credentials` |
| X3 | `transport/outbound.go` | 出站仅 TLS/JA3 模板名；无 HTTP/3 QUIC 对齐 | CDN/风控 QUIC 特征未覆盖 |
| X4 | 全库 `Topics`/`CHIPS`/`cookiePartition` | 零实现 | Privacy Sandbox / 第三方 Cookie 分区未建模 |
| X5 | `asymmetric/stealth_matrix.go`；SessionBundle vs ProfileTrustBundle | 双栈信任；无 bundle 时 cap ~88 | 导出/导入/发射前 hydrate 语义不统一 |
| X6 | `platform-packs/xhs/` | 有 cadence/selectors；**无** `trust.yaml` | 信任继承靠 ad-hoc harvest |
| X7 | webdriver getter + detection UI | W0 已 nativeize getter；结构化 headless/lies 仍缺 | CreepJS parser 仍 0/3（历史 raw） |
| X8 | `detection/site_probe.go` | CreepJS 偏 regex | 无 trust/lies/worker 交叉结构化输出 |
| X9 | `docs/47` RPA；录制桥 | 有 primitives + 录制桥；无可视化编排 UI | 商业 RPA 差距 |
| X10 | `graphapi/client.go` | `http.DefaultClient` / 无 profile 代理；未设浏览器 UA | API 与浏览器身份分裂 |
| X11 | `browser/profile.go:31-54` `ValidateFingerprintArgs` | **仅 warning，不剔除** | 无效 `--fingerprint-*` 仍可发射 → 虚假安全感 |
| X12 | `app_lifecycle.go` | lifecycle 引擎存在 | cadence 人时窗未硬门禁 |
| X13 | AdsPower Free API 付费墙 | manual-only | MCP/Agentic 观察项 |
| X14 | `recording_iframe_test.go` | 行为层 iframe 聚合 | 指纹层 iframe 隔离未覆盖 |
| X15 | `app_proxy_subscription.go` / `http_client.go` / `iphealth.go` / `speedtest.go` / `app_profile.go` | 多处硬编码 `PersonalPilot/1.0` 或 `personal-pilot/1.1` | **出站 UA 分裂面比 Graph 更广**（W0 后新确认） |
| X16 | `cdp_executor.go` `DefaultMouseProfile` | W0 后仍固定 Bezier `0.3/0.7` | humanize Fitts 计划层未进 mouseMove（→ `docs/53`，此处交叉登记） |
| X17 | `cdp_executor.go` + `inputplane/` | D1 重试已有；**无** OS fallback 调用 | `docs/49` D1 残余 AC1 |
| X18 | 2026-07-10 行为口径纠正 | Rust `35` declared / `13` active；Go `35` / `30` concrete | 不得再虚报 `drag_to_element` 等为 shipped |

## 2. 工作流

| 工作流 | 主题 | 优先级 |
| --- | --- | --- |
| **P** Provider 闭环 | CAPTCHA/SMS/Email 凭证 smoke + 平台挑战路由 | P0（平台 live 阻塞；本地可先骨架） |
| **T** 信任统一 | SessionBundle ↔ ProfileTrustBundle + platform trust pack | P0（W1 并行） |
| **E** 出站补齐 | Graph/**全部 ops HTTP** 走 profile 出口 + UA 同源 + QUIC 探针 | P1 |
| **F** 指纹边界 | iframe 策略、**无效 flag 真正剔除**、Privacy Sandbox 声明 | P1 |
| **B** Benchmark 解析 | CreepJS/BrowserScan 结构化 parser + 10-run drift 挂钩 | P1（W1 可提前） |
| **R** 商业补齐 | RPA 可视化 MVP、AdsPower 能力观察清单 | P2 |

推荐顺序（W0 已过后）：**T1 → B1 → E1（含 X15）→ F1 → D1 残余/X17 → P1 骨架 → T2 → R1**。

---

## 3. Task 明细

### P1 — Provider 凭证 smoke 门禁

**技术指导**

1. 在 `scripts/user_requirements_acceptance_gate.ps1` 增加 `-RequireProviderSmoke` 开关（默认 off）。
2. 环境变量：`CAPTCHA_PROVIDER_KEY`、`SMS_PROVIDER_KEY`、`EMAIL_PROVIDER_KEY`；无 key 时输出 `blocked_missing_credentials`，不得写 `accepted`。
3. 对接现有 solver 边界（`docs/20-captcha-solving-design.md`），只跑 **dry-run / balance-check / mock-target** 三类子测试。

**验收 AC**

| AC | 条件 |
| --- | --- |
| P1-AC1 | 无 key 时 gate 退出码 0 但报告含 `blocked_missing_credentials` |
| P1-AC2 | 有 key 时至少 1 个 provider `balance_ok` 或 `solve_dry_run_ok` |
| P1-AC3 | `docs/02-current-state.md` 诚实边界与 gate 输出一致 |

**门禁**：`.\scripts\user_requirements_acceptance_gate.ps1 -RequireProviderSmoke`

---

### T1 — 信任资产双栈统一

**技术指导**

1. 定义 canonical `TrustSurface` JSON schema：`cookies`、`localStorage`、`sessionStorage`、`refreshToken`、`harvestedAt`、`source`。
2. `ProfileTrustBundleSave` 与 Rust `SessionBundle` export 均映射到同一 schema；发射前 `stealth_engine` 只读 canonical。
3. `asymmetric/stealth_matrix.go` 无 bundle 时保持 cap 提示，但发射门禁改为 **warn → block**（可配置 `-TrustBundleMode=warn|block`）。

**验收 AC**

| AC | 条件 |
| --- | --- |
| T1-AC1 | 单测：同 profile 从 Go export 与 workbench import 后 `TrustSurface` 字节级一致（脱敏字段除外） |
| T1-AC2 | `TestStealthMatrix` 无 bundle + `block` 模式 → `ShouldExecute=false` |
| T1-AC3 | `docs/50` B1/B4 交叉引用更新 |

**门禁**：`go test ./backend/internal/trust/... ./backend/internal/asymmetric/... -run Trust -count=1`

---

### T2 — Platform pack `trust.yaml`

**技术指导**

1. 在 `platform-packs/xhs/` 新增 `trust.yaml`：`cookieDomains`、`storageKeys`、`bootstrapUrls`、`maxAgeHours`。
2. `app_stealth_autopilot` 加载 pack 时合并 trust 规则；harvest 后按 `maxAgeHours` 衰减。
3. 模板复制到 `platform-packs/_template/trust.yaml` 供新平台扩展。

**验收 AC**

| AC | 条件 |
| --- | --- |
| T2-AC1 | `platform-packs/xhs/trust.yaml` 通过 YAML schema 校验 |
| T2-AC2 | XHS live acceptance 报告含 `trust_pack_loaded=true` |
| T2-AC3 | 过期 bundle 触发 `trust_stale` 而非静默失败 |

**门禁**：`.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda`（报告字段）

---

### E1 — Graph / ops 出站走 profile 出口（含 X15）

**技术指导**

1. `graphapi/client.go` 注入 `proxy.FromProfile(profileID)` 与 `transport.OutboundForProfile`。
2. 统一审计并替换硬编码 UA：`PersonalPilot/1.0`、`personal-pilot/1.1`、`Personal-Pilot-Webhook/1.0` 等；浏览器相关出站用 profile materialized UA，纯 ops 可保留产品 UA 但必须进 `egress_identity_matrix` 白名单分类。
3. 覆盖面：`graphapi`、`app_proxy_subscription`、`subscription_fetcher`、`http_client` canary、`iphealth`、`speedtest`、`app_profile` fetch。
4. 集成测试：mock egress 断言走 local proxy port（有代理时）。

**验收 AC**

| AC | 条件 |
| --- | --- |
| E1-AC1 | Graph 请求 UA 与 profile UA 一致（或显式 `ops_identity` 分类） |
| E1-AC2 | `egress_identity_matrix.mjs` 浏览器 vs Graph 出口 IP 一致（有代理时） |
| E1-AC3 | 仓库内无未登记的 `PersonalPilot/1.0` 出站路径（gate 扫描） |
| E1-AC4 | 无代理 profile 直连行为不回归 |

**门禁**：`node scripts/egress_identity_matrix.mjs --probe=graph,ops`

---

### F1 — 无效 fingerprint flag 剔除（升级自 warning）

**技术指导**

1. Materialize 发射前**过滤** `KnownIneffectiveFingerprintFlags`（今日仅 `ValidateFingerprintArgs` warning，不够）。
2. 写入 `MaterializeReport.ineffectiveFlagsDropped[]`；Validation Board 展示。
3. 与 `docs/51` S 项对齐：不得把无效 flag 写成已覆盖 surface。

**验收 AC**

| AC | 条件 |
| --- | --- |
| F1-AC1 | 单测：带无效 flag 的 profile 启动参数**不含**该 flag |
| F1-AC2 | observation report 含 `ineffectiveFlagsDropped` |
| F1-AC3 | `ValidateFingerprintArgs` 对已 drop 的 flag 不再重复刷屏 warning |

**门禁**：`go test ./backend/internal/browser/... -run Ineffective -count=1`

---

### F2 — iframe 指纹策略（声明 + 能做的先做）

**技术指导**

1. **Phase A（本 Task）**：文档化策略——跨域 iframe 不注入；operator 避免在第三方 iframe 内做敏感操作；检测站 iframe 结果标注 `scope=child-frame-unhooked`。
2. **Phase B（可选 spike）**：`Page.addScriptToEvaluateOnNewDocument` + `matchAboutBlank` + 同源 frame CDP `Page.createIsolatedWorld` 调研，单独 spike 文档，不阻塞 Wave 4。

**验收 AC**

| AC | 条件 |
| --- | --- |
| F2-AC1 | `docs/02` 与 Validation metadata 含 iframe scope 说明 |
| F2-AC2 | benchmark artifact 中 iframe 探针带 `collectorScope` |
| F2-AC3 | 不声称跨域 iframe 已 hook |

---

### B1 — CreepJS 结构化 parser

**技术指导**

1. 扩展 `detection/site_probe.go`：解析 trust score、lies 列表、headless、worker consistency。
2. 输出 JSON schema 对齐 `docs/48` detector matrix列。
3. 接入 `two_browser_missing_probe_matrix.mjs` 报告。

**验收 AC**

| AC | 条件 |
| --- | --- |
| B1-AC1 | 本地 fixture HTML 解析 trust/lies 非 0 |
| B1-AC2 | missing probe 报告 `creepjs_structured_parser=ok` |
| B1-AC3 | 无 headed run 时 `blocked_no_artifact` |

**门禁**：`node scripts/two_browser_missing_probe_matrix.mjs --product=personal-pilot --require-creepjs-parser`

---

### R1 — RPA 可视化 MVP（商业补齐）

**技术指导**

1. 在 `src/modules/browser/pages/BehaviorRecordingPage.tsx` 增加步骤时间线 + 条件分支只读视图（先只读，不接新执行引擎）。
2. 复用 `workflow_recording_bridge` 已有 JSON；不新建 DSL。
3. 对标 `docs/47` RPA 维度：记录 latency P50/P95 观察项。

**验收 AC**

| AC | 条件 |
| --- | --- |
| R1-AC1 | 录制 JSON 可在 UI 时间线渲染 ≥10 步 |
| R1-AC2 | 无 mock 数据；空录制显示诚实空态 |
| R1-AC3 | `pnpm typecheck` 通过 |

---

## 4. Wave 4 汇总门禁

```powershell
go test ./backend/internal/detection/... ./backend/internal/graphapi/... ./backend/internal/trust/... -count=1
.\scripts\user_requirements_acceptance_gate.ps1
node scripts/two_browser_missing_probe_matrix.mjs --product=personal-pilot
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda
```

## 5. 风险与回滚

| 风险 | 缓解 |
| --- | --- |
| Provider smoke 误用生产余额 | 默认 off；仅 dry-run/balance |
| Trust bundle block 过严 | `TrustBundleMode=warn` 默认，可切换 |
| Graph 走代理破坏本机直连 | 无代理 profile 保持直连 |
| iframe Phase B 范围膨胀 | Phase A 先落地，B 独立 spike |

## 6. 文档交叉引用

- 战术修补：`docs/49`
- 不对称支配：`docs/50`
- 横评退出条件：`docs/48` §Detector/B1
- 平台交接：`docs/45`
- 统一入口：`PLAN.md` §2 Wave 4
