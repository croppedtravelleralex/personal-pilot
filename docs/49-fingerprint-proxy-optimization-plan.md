# 49 指纹 / 代理 / 行为全方位性能优化落地计划

Updated: 2026-07-11 (Asia/Shanghai)

## 0. 本文定位与边界

- **目标**：把 Personal Pilot 在指纹真实性、指纹/代理/行为运行时性能、可观测证据链上的短板，落成**可执行、可验收、证据驱动**的工程任务，逐步追平 BitBrowser 并在黑盒 A 层接近 AdsPower。
- **不改主线口径**：本机自用仍按 `100% / 0% / green` 维护；本计划是独立优化 track，任务完成才刷新评分。
- **诚实边界**：不 fork Chromium，不做「数学 100% 不可检测」，不伪造 `accepted`；无证据的项一律 `blocked`。
- **关联文档**：
  - 统一入口：`PLAN.md`
  - 架构：`docs/46-global-99plus-architecture.md`（L0–L6 + Input Plane）
  - 横评基线：`docs/47-personal-pilot-adspower-bitbrowser-benchmark.md`、`docs/48-three-browser-benchmark-matrix-plan.md`
  - 交接纪律：`docs/45-stealth-platform-handoff.md`
  - 代理供应链：`docs/29-proxy-supply-chain.md`
  - 边界补齐：`docs/56-boundary-closure-and-commercial-parity.md`

## 0.1 Wave 0 落地状态（2026-07-10 实现 · 2026-07-11 复核）

| Task | 状态 | 已落地 | 仍缺 / 诚实边界 |
| --- | --- | --- | --- |
| **A1** toString 原生化 | **landed** | `makeNative` + WeakMap；本机 Chrome CDP smoke 确认 Canvas/WebGL/webdriver/toString 呈 `[native code]` | CreepJS lies before/after 需 B1 parser + 新 raw artifact 回填（AC4 未关） |
| **A3** UA/内核单一真相源 | **landed** | `core_version.go` 派生；materialize 去掉 131 硬编码；`ua_core_coherent` | 横评 `missing-1783494756348` 仍是改前 `0/3`；**未用新 raw 刷分** |
| **C1** DNS 探针修正 | **landed** | `DNSStatus=inconclusive`；不再把 CDN A 记录当泄漏 | 权威 DNS-token 仍外部阻塞（AC3） |
| **D1** 鼠标往返/重试 | **partial** | press/release `200/400/800ms` 退避；轨迹上限按 RTT 压到 32/24/16/12；逐事件 overlay 已移除 | **OS/inputplane fallback 未接线**；deep matrix 行为 6/6 **未复跑** |

> Wave 0「实现完成」≠「横评分刷新」。评分仍以有新 detector/deep raw artifact 为准。

## 1. 证据基线（code-level）

### 1.1 历史基线（2026-07-08，改前快照，勿当现状）

| 编号 | 改前事实 | 问题 |
| --- | --- | --- |
| E1 | 无 `Function.prototype.toString` 原生化 | hook `.toString()` 露馅 |
| E5 | 硬编码 Chrome/131 | UA/core mismatch 0/3 |
| E8 | CDN A 记录比对出口 IP → 恒 suspect | DNS 假阳性 |
| E10/E11 | 鼠标最多 80 步 + 每步 overlay + 无重试 | click timeout |

### 1.2 现行基线（2026-07-11 复核，改前必读）

| 编号 | 文件:行（约） | 已核实现状 | 仍开缺口 |
| --- | --- | --- | --- |
| E1′ | `environment_injector.go` `makeNative` | WeakMap + `Function.prototype.toString` 补丁已落地 | Worker 作用域仍无同一脚本（→ `docs/50` C1） |
| E2 | webdriver getter | 仍 `defineGetter`；descriptor 痕迹未收敛 | → Task A2 |
| E3 | `ApplyEnvironmentInjection` ~302–334 | 仍 `Runtime.enable` + `addScriptToEvaluateOnNewDocument` + 当前页 `Runtime.evaluate` | 双层注入 / CDP-minimal → A2/A4 |
| E4 | canvasNoise | 红通道 + 固定 shift 策略未改 | → A5 |
| E5′ | `runtime_materialize.go` + `core_version.go` | UA/brand-version 由内核版本派生 | 需新 missing-probe raw 验证 3/3 |
| E6 | `runtime_projection.go` | ~26 env-backed / 80 declared | 深度仍浅 → `docs/51` |
| E7 | `detection/site_probe.go` | CreepJS 仍偏 regex | → B1 / `docs/56` B1 |
| E8′ | `leak_probe.go` `classifySystemDNSObservation` | 固定 `inconclusive`，`DNSLeakSuspect=false` | DNS-token 仍缺 |
| E9 | `proxy_ip_monitor.go` | 顺序阻塞 check 未改 | → C2 |
| E10′ | `cdp_executor.go` `mouseMoveStepCount` | 步数已按 level/RTT 压缩 | 轨迹仍 `DefaultMouseProfile` 固定 0.3/0.7 Bezier（→ `docs/53`） |
| E11′ | `dispatchMouseEvent` | 有重试；**无** `syncPointerOverlay` 逐事件调用 | OS fallback 未接；D2 overlay 门控可并入收尾 |
| E12 | `verify_v2.go` | streak 结构仍在，自动化闭环未改 | → C3 |
| E13 | `transport/profile.go` | TLS 模板名未绑 UA major | → C4 |

已确认的**并发正例**（复用，不重造）：`backend/internal/browser/proxy_speed.go:84-110` `runAll` 已用 `sem + WaitGroup`（默认 concLimit=5）。

## 2. 工作流总览与优先级

| 工作流 | 主题 | 追平对象 | 优先级 |
| --- | --- | --- | --- |
| **A** 指纹真实性 | toString 原生化、注入门控、UA 一致性、噪声质量 | BitBrowser+AdsPower | P0 |
| **B** 指纹可观测 | CreepJS 结构化解析、指纹 DNA、10-run drift | BitBrowser | P1 |
| **C** 代理性能 | DNS 探针修正、监控并发化、verify 闭环、TLS 基线 | BitBrowser+AdsPower | P0/P1 |
| **D** 行为/CDP 性能 | 鼠标往返压缩、click 重试+OS fallback、overlay 门控 | BitBrowser | P0 |

执行顺序（依赖）：`A1 → A2 → A3 → A4 → A5`；`A3` 前置 `C4`；`D1/D2` 独立可并行；`B1` 前置 `B2`；`C1/C2` 独立。
**波次现状（2026-07-11）**：Wave 0 = A1+A3+C1+D1(partial) **已落地实现**；下一本地波次 = A2+A4+A5+B1+C2 + `docs/50` C1 Worker + `docs/52` DP1/2 + `docs/53` L5 + `docs/56` T1；Wave 2 = B2+C3+C4+D2（及 50/51/54）。

---

## 3. 工作流 A：指纹真实性

### Task A1 — Function.prototype.toString 原生化框架（P0，最高 ROI）

**现状/根因**（E1）：所有 hook 直接替换原型方法，`fn.toString()` 返回 JS 源码而非 `function xxx() { [native code] }`；CreepJS/BrowserScan 通过 `toString` 一致性判定 lies。

**技术指导**：

1. 在注入脚本顶部新增 `makeNative(fake, original)` 工具：
   - 用 `Object.defineProperty` 覆写 `fake.toString`，返回 `original.toString()` 原文（保留 `function get xxx() { [native code] }` 风格）；
   - 同时保护 `fake.toString.toString`（防二级检测），并把 `toString` 自身也标记为原生；
   - 维护一个 `WeakMap<fake, original>`，改写全局 `Function.prototype.toString`（一次）走 `Reflect.apply(原始 toString, map.get(this) ?? this, args)`。
2. 所有 `defineGetter`、原型方法替换（canvas/webgl/audio/intl/fonts/mediaDevices/RTC）替换后统一调用 `makeNative`。
3. 全局 `Function.prototype.toString` 改写本身也要 `makeNative`（自举）。

**改动文件**：`backend/internal/behavior/environment_injector.go`（注入脚本模板）。

**验收标准 AC**：
- AC1：新增 `environment_injector_test.go` 用例，断言编译出的脚本包含 `makeNative` 且覆盖全部被 hook 的符号名（canvas/webgl/audio/intl/fonts/mediaDevices/RTCPeerConnection/navigator getters）。
- AC2：真实内核 CDP smoke（复用 `scripts/full_observed_fingerprint_probe.mjs` 链路）中，对 `HTMLCanvasElement.prototype.toDataURL.toString()`、`WebGLRenderingContext.prototype.getParameter.toString()`、`navigator.webdriver` getter、`Function.prototype.toString.toString()` 全部返回含 `[native code]`。
- AC3：`go test ./backend/internal/behavior/... -count=1` passed。
- AC4：CreepJS 结构化解析（依赖 B1）后 lies 数不升反降（记录 before/after，允许 B1 完成后回填）。

### Task A2 — 注入方式与 descriptor 痕迹收敛（P1）

**现状/根因**（E2）：`configurable:true` + 原型 getter 使 `Object.getOwnPropertyDescriptor(Navigator.prototype,'webdriver')` 可见异常；部分属性应落在实例而非原型。

**技术指导**：
1. `webdriver` 优先由内核 `--disable-blink-features=AutomationControlled` 提供；注入层仅在检测到仍暴露时兜底，且 getter 返回 `false`/`undefined` 需与真实 Chrome 行为一致（真实 Chrome 无痕迹时不应强行 define）。
2. 对确需覆写的属性，`configurable` 与真实描述符对齐；能用 `Reflect.defineProperty` 保持 enumerable/configurable 与原生一致的就对齐。
3. 新增「注入前探测」：若目标符号已由内核 materialize（A3 提供的 core 版本能力表），则**跳过注入**，避免双层。

**改动文件**：`environment_injector.go`、`backend/internal/browser/runtime_projection.go`（能力表查询）。

**验收标准 AC**：
- AC1：单测断言：当传入「内核已提供 webdriver 隐藏」标志时，脚本不再 define `webdriver`。
- AC2：CDP smoke 下 `Object.getOwnPropertyDescriptor(Navigator.prototype,'webdriver')` 结果与同版本真实 Chrome 一致（记录对照）。
- AC3：`go test ./backend/internal/behavior/... -count=1` passed。

### Task A3 — UA / UA-CH / 内核版本单一真相源（P0）

**现状/根因**（E5）：Materialize 写死 131，内核实际 139，UA/`--fingerprint-brand-version`/`uaDataFullVersionList` 与真实内核 major 不一致。

**技术指导**：
1. 新增 `backend/internal/browser/core_version.go`：
   - 输入 core 二进制路径/`browser_cores` 记录；优先读 CDP `/json/version` 的 `Browser`/`User-Agent`，或二进制 product version；输出 `major/full/uaTemplate`。
   - 结果带短 TTL 缓存，避免每次启动都探测。
2. `MaterializeRuntimeArgs` / `runtime_auth_preset.go` 改为**从 core 派生** UA、`--fingerprint-brand`、`--fingerprint-brand-version`、`--fingerprint-platform-version`；删除硬编码 131（保留常量兜底但标注为 fallback）。
3. CDP 注入层的 `userAgent`/UA-CH 与内核派生值一致，禁止注入与内核不同的 major（配合 A2 注入门控）。
4. `IdentityStrengthReport`（`identity_report.go`）新增 `uaCoreCoherent` 子分：UA major == 内核 major == UA-CH fullVersion major 才 pass，否则降级 `weak`。

**改动文件**：`core_version.go`（新）、`runtime_materialize.go`、`runtime_auth_preset.go`、`identity_report.go`、`environment_injector.go`。

**验收标准 AC**：
- AC1：单测覆盖 139→139、131→131 两套派生，断言 UA / brand-version / UA-CH major 三者一致。
- AC2：`scripts/two_browser_missing_probe_matrix.mjs --product=personal-pilot` 中 **UA/core major match ≥ 3/3**（当前 0/3）。
- AC3：Validation Board 显示 `uaCoreCoherent=pass`。
- AC4：`go test ./backend/internal/browser/... -count=1` passed。

### Task A4 — 减双层注入 + CDP-minimal 收敛（P1）

**现状/根因**（E3）：当前页二次 `Runtime.evaluate` 注入 + `Runtime.enable` 常驻，形成双层且扩大暴露面。

**技术指导**：
1. 注入统一走 `addScriptToEvaluateOnNewDocument`，在**导航前**注入；启动流程保证首次导航即带 hook，去掉对已打开页的二次 `Runtime.evaluate`（或仅在「实例已停在真实页且必须补丁」时降级执行，并记 warning）。
2. `Runtime.enable` 改为**一次性**：注入完成后按需 `Runtime.disable`，或改用不依赖 enable 的注入路径；保留 `Page` 域。
3. 新增 `injection_coherence_report`：列出每个指纹族的 `declared / kernel / injected` 三层归属，禁止 kernel 已覆盖仍注入（与 A2 门控共用能力表）。

**改动文件**：`environment_injector.go`、`backend/app_environment_injection.go`。

**验收标准 AC**：
- AC1：CDP 命令序列 smoke 断言：正常启动路径**不出现**对当前页的重复脚本 `Runtime.evaluate`（除显式降级分支）。
- AC2：`injection_coherence_report` 中不存在 `kernel=true && injected=true` 的族。
- AC3：XHS live（`scripts/xhs_live_acceptance.ps1`）仍 12/12 PASS，CreepJS trust 不下降。

### Task A5 — Canvas/WebGL/Audio 噪声质量升级（P1）

**现状/根因**（E4）：canvas 仅改红通道且 shift 固定；跨站可关联、稳定性/真实性两头不讨好。

**技术指导**：
1. Canvas：对 RGBA 四通道按 `seed+坐标` 派生亚可见抖动（±1~2），保证**同 profile 同 session 稳定、跨 profile 不同**；`toDataURL` 采样范围从 64×64 提升到实际尺寸（性能允许时）或分块。
2. WebGL：除 vendor/renderer 外，覆盖 `UNMASKED_VENDOR/RENDERER`、`getShaderPrecisionFormat`、`readPixels` 噪声与 renderer 字符串一致。
3. Audio：噪声改为与 seed 绑定的确定性微扰，避免每次调用值漂移导致 in-session drift。
4. 全部经 A1 `makeNative` 处理。

**改动文件**：`environment_injector.go`。

**验收标准 AC**：
- AC1：10-run drift（依赖 B2）中 canvas/webgl/audio 同 profile 稳定率 ≥ 9/10。
- AC2：两个不同 profile 的 canvas/webgl hash 不相同（碰撞率 0）。
- AC3：`go test ./backend/internal/behavior/... -count=1` passed。

---

## 4. 工作流 B：指纹可观测

### Task B1 — CreepJS/检测站结构化解析（P1）

**现状/根因**（E7）：`ParseCreepJSProbePayload` 仅 regex trust + `lie` 词频，3s 采样，实测 parser 0/3。

**技术指导**：
1. 新增 `backend/internal/detection/creepjs_parser.go`：
   - 采样策略从固定 3s 改为**轮询等待**（poll `FP ID` 稳定，最长 15s，间隔 2s，可配置）。
   - 用 DOM 选择器/结构提取 `trust score`、`lies` 列表与数量、`headless`、`stealth`、`bot` 字段，而非纯词频。
2. 新增 `browserleaks_parser.go`：WebRTC（local/public candidate）、DNS（resolver/country）结构化。
3. 接入 `WorkbenchRunStealthProbeSuite`（`backend/app_stealth_engine.go`），trust 用真实值而非启发式；`EvaluateStealthMatrix` bonus 用结构化 trust。
4. 新增门禁 `scripts/detector_structured_parse_gate.ps1`。

**验收标准 AC**：
- AC1：`missing_probe` CreepJS parser **≥ 3/3** 可提取 trust 数值与 lies 数。
- AC2：`StealthMatrix` 的 `CreepJSTrust` 来源从启发式切为解析值（代码路径断言 + 报告字段标注 `source=parsed`）。
- AC3：`go test ./backend/internal/detection/... -count=1` passed。

### Task B2 — 指纹 DNA + 10-run 跨 session drift（P1，对标 BitBrowser）

**现状/根因**：无跨窗相似度、无多次启动漂移量化（横评缺口）。

**技术指导**：
1. 新增 `backend/internal/browser/fingerprint_dna.go`：把 canvas/webgl/audio/fonts/screen/tz/lang 归一化为向量，算池内最大余弦相似度，`>0.92` 标 `alert`。
2. 批量创建 profile 时计算并入库；`BrowserListPage.tsx` 列表标红。
3. 新增 `scripts/two_browser_drift_matrix.mjs`：同 profile 10 次 cold start 采指纹 hash，输出 `driftScore`/`stableRate`。

**验收标准 AC**：
- AC1：100 个随机 profile 无 `>0.92` 碰撞（`fingerprint_dna_gate.ps1`）。
- AC2：PersonaPilot canvas/webgl 10-run 稳定率 ≥ 9/10。
- AC3：UI 列表能显示相似度并标红（源码级 gate + 截图证据）。

---

## 5. 工作流 C：代理性能

### Task C1 — DNS 泄漏探针修正（P0）

**现状/根因**（E8）：`ProbeDNSConsistency` 用本机 `net.DefaultResolver` 解析 `api.ipify.org`（CDN），A 记录与代理出口无关 → 几乎恒 `DNSLeakSuspect=true`，是**假信号**。

**技术指导**：
1. 重新定义 DNS 泄漏语义：应验证「浏览器/出站的 DNS 查询是否经代理出口」，而非比对 CDN A 记录。
2. 短期（无受控域名）：改为**经代理 HTTP** 访问返回「解析方 IP/国家」的服务（如 DNS echo 服务），与出口 IP 国家一致性比对；把当前不成立的本机 resolver 比对**移除或降级为 `inconclusive`**，不再输出 `LeakSuspect=true` 假阳性。
3. 长期（W1-3 依赖受控域名）：`dns_token_probe.go` — 随机子域 + 权威 DNS 日志三方比对（浏览器解析 vs 代理 ip-api vs 权威日志）。
4. 修正后接入 `collectLiveDetectionSignals`（`backend/app_detection_probes.go`）的 `DNSConsistent`。

**改动文件**：`backend/internal/proxy/leak_probe.go`、`app_detection_probes.go`；（长期）`dns_token_probe.go`（新）。

**验收标准 AC**：
- AC1：单测：CDN 域名场景不再输出 `DNSLeakSuspect=true`（改为 `inconclusive` 或走代理侧判定）。
- AC2：`go test ./backend/internal/proxy/... -count=1` passed。
- AC3（长期）：受控域名可用后 `dns_token_proof_gate.ps1` 三方一致 3/3。

### Task C2 — ProxyIPMonitor 并发化 + 自适应间隔（P1）

**现状/根因**（E9）：`runOnce` 串行遍历运行实例，逐个阻塞；固定 60s。

**技术指导**：
1. `runOnce` 改为 `sem + WaitGroup` 并发（复用 `proxy_speed.go` 模式，concLimit 默认 5，可配置）。
2. 自适应间隔：稳定（连续 N 次无漂移）时退避到 120s，检测到漂移或 sticky 临期时收紧到 30s。
3. 每 proxy 检查加独立超时，单点慢不拖累整体。

**改动文件**：`backend/internal/browser/proxy_ip_monitor.go`。

**验收标准 AC**：
- AC1：单测：10 个运行实例、每个 checkFn 阻塞 200ms，总耗时 < 1s（证明并发）。
- AC2：漂移回调仍准确触发（现有 `NoteExitIP` 语义不回归）。
- AC3：`go test ./backend/internal/browser/... -count=1` passed。

### Task C3 — VerifyV2 自动化采样闭环 + sing-box 桥预热（P1）

**现状/根因**（E12）：`VerifyV2Streak` 仅数据结构，无自动连续采样；桥有 45s idle TTL/refcount（`singbox.go:22-24`）但无「即将使用预热」。

**技术指导**：
1. 新增采样调度：绑定/启动时对出口做 3 次间隔采样，写入 `VerifyV2Streak`，达标才标记 profile 可用；失败进 `routing_reconcile`。
2. sing-box 桥预热：Profile 启动前若已知绑定 proxy，提前 `EnsureBridge` 并 `waitPortReady`，减少首个请求冷启动（复用现有 `tryReuseBridge`）。
3. 采样与预热都要有超时与脱敏日志。

**改动文件**：`backend/app_proxy_reconcile.go`/`app_proxy_monitor.go`、`backend/internal/proxy/verify_v2.go`（调度 helper）、`singbox.go`（预热入口）。

**验收标准 AC**：
- AC1：单测：连续 3 次同出口 OK → `Evaluate` pass；中途换 IP → streak 断。
- AC2：桥预热后首请求延迟较冷启动下降（记录 before/after 本机数值）。
- AC3：`go test ./backend/internal/proxy/... -count=1` passed。

### Task C4 — 出站 TLS/JA3 与 UA 基线一致（P2，前置 A3）

**现状/根因**（E13）：`transport/profile.go` 仅模板名，未与 profile UA major 绑定。

**技术指导**：
1. 建 `ChromeMajorTLSBaseline`：Chrome major → 期望 ALPN/cipher 顺序/utls 指纹族。
2. `applySingBoxTransportProfile`（`singbox.go:833`）/`mergeXrayStreamSettings`（`xray.go:670`）按 **profile UA major**（A3 派生）选模板。
3. deep matrix 增 `tlsUaCoherent`；不一致进 `AsymmetricBootstrapGaps`。
4. 禁止把模板名当真实 observed JA3 冒充。

**验收标准 AC**：
- AC1：单测：profile major=139 → 选中对应 baseline；major 变更 → 模板变更。
- AC2：经代理访问 tls.browserleaks.com，JA3 与 UA major **≥ 2/3** 自洽（A3 修复后）。
- AC3：`go test ./backend/internal/proxy/... ./backend/internal/transport/... -count=1` passed。

---

## 6. 工作流 D：行为 / CDP 性能与稳定

### Task D1 — 鼠标 CDP 往返压缩 + click 重试 + OS fallback（P0）

**状态（2026-07-11）**：**partial** — 往返压缩 + press/release 重试 + 默认路径去 overlay **已落地**；OS/inputplane fallback 与 deep matrix 6/6 复跑 **未关**。

**现状/根因**（E10′/E11′）：步数已按 humanization/RTT 压缩；`dispatchMouseEvent` 对 press/release 有 3 次退避；逐事件 `syncPointerOverlay` 已移除。仍缺：失败后降级 `inputplane` OS click；轨迹仍用固定 Bezier 控制点（L5 接线见 `docs/53`）。

**技术指导（残余）**：
1. click 稳定收尾：重试仍失败且 headed 前台时降级 `inputplane` S0 OS click；审计 `clickFallback=os`。
2. 与 D2 合并确认：仅 `show-mouse-pointer` 标签才安装/同步 overlay（当前默认路径已不发 overlay evaluate）。
3. 复跑 `two_browser_benchmark_deep_matrix.mjs` 行为 cell，目标 6/6（改前 5/6）。

**改动文件**：`backend/internal/behavior/cdp_executor.go`、`backend/internal/behavior/inputplane/router.go`、`backend/app_workbench_inputplane.go`。

**验收标准 AC**：
- AC1：单测：模拟一次 dispatch timeout → 触发重试；重试仍失败 → 记录 OS fallback 分支。（**重试已有；OS 分支未关**）
- AC2：`scripts/two_browser_benchmark_deep_matrix.mjs` 行为 cell **6/6**，且单次点击总 CDP 往返数下降（记录 before/after）。（**未复跑**）
- AC3：`go test ./backend/internal/behavior/... -count=1` passed。（本地 W0 门禁已过）

### Task D2 — Overlay 与调试开销门控（P2）

**现状/根因**（E11）：非调试场景也走 overlay 同步，产生额外往返与页面副作用。

**技术指导**：
1. `syncPointerOverlay`/overlay 安装脚本仅当 profile 含 `show-mouse-pointer` 标签或 workbench 显式开启时执行。
2. 默认关闭时完全不注入 overlay 相关脚本，减少页面 `window.__personalPilotPointerOverlay` 痕迹（降低检测面）。

**改动文件**：`cdp_executor.go`、`backend/app_instance_mouse.go`。

**验收标准 AC**：
- AC1：单测：无标签时 `syncPointerOverlay` 不产生 `Runtime.evaluate`。
- AC2：无标签启动的页面上下文无 overlay 全局变量（CDP evaluate 断言 `typeof window.__personalPilotPointerOverlay==='undefined'`）。
- AC3：XHS live 鼠标可视化验收在带标签时仍可见（不回归）。

---

## 7. 依赖关系与执行波次

```text
Wave 0（最高 ROI）— 2026-07-10 实现 / 2026-07-11 复核
  A1 toString 原生化     ✅ landed（AC4 lies 回填待 B1）
  A3 UA/内核一致性       ✅ landed（横评分待新 raw）
  C1 DNS 探针修正        ✅ landed（DNS-token 仍外部）
  D1 鼠标往返+重试       🟡 partial（OS fallback + deep 6/6 未关）

Wave 1（本地下一批）
  A2 注入门控 ← A3 能力表
  A4 减双层  ← A2
  A5 噪声质量
  B1 CreepJS 结构化解析 ─→ 回填 A1/A4 的 lies before/after
  C2 监控并发化
  D1 残余 OS fallback
  （并行：docs/50 C1 Worker · docs/52 DP1/2 · docs/53 L5 · docs/56 T1）

Wave 2
  B2 指纹 DNA + 10-run drift ← A5、B1
  C3 verify 闭环 + 桥预热
  C4 TLS/JA3 基线 ← A3
  D2 overlay 门控 ← D1
```

## 8. 里程碑与评分口径

评分只在**有 raw artifact** 后刷新 `docs/47` §12；不得凭空提分。

| 阶段 | PersonaPilot 目标分 | 关键证据门禁 |
| --- | --- | --- |
| 改前基线 | 73 | `deep-1783482661915`、`missing-1783494756348`（UA/core 0/3） |
| Wave 0 实现 | 73（**不刷分**） | 代码 + 本机 Chrome nativeization smoke；待新 missing/deep raw |
| Wave 0 证据关闭 | 78–82 | 新 raw：UA match ≥3/3、行为 6/6、DNS 假信号消除、toString native 3/3 |
| Wave 1 完成 | 83–86 | CreepJS parser 3/3、注入无双层、监控并发、Worker 一致 |
| Wave 2 完成 | 87–90 | DNA 无碰撞、10-run ≥9/10、TLS/UA 自洽、桥预热提速 |

## 9. 总验收门禁（每波次末必跑）

```powershell
# Go 单测（改到哪个包跑哪个，至少覆盖）
go test ./backend/internal/behavior/... ./backend/internal/browser/... ./backend/internal/detection/... ./backend/internal/proxy/... ./backend/internal/transport/... -count=1

# 平台离线门禁
.\scripts\platform_99_gate.ps1 -Track all

# 指纹真实性 / 检测站结构化（B1 落地后）
.\scripts\detector_structured_parse_gate.ps1
node scripts/two_browser_missing_probe_matrix.mjs --product=personal-pilot

# 深度矩阵（行为 / TLS / IP）
node scripts/two_browser_benchmark_deep_matrix.mjs

# 10-run drift + DNA（B2 落地后）
node scripts/two_browser_drift_matrix.mjs --runs=10
.\scripts\fingerprint_dna_gate.ps1

# 平台 live（回归，不得下降）
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda
```

**通过判据**：所有 `go test` passed；`platform_99_gate` 29/29；`missing_probe` UA/core match、CreepJS parser 达 AC 值；XHS live 12/12 不回归；新增 gate 各自 AC 达标。任一未达标记 `blocked` 并记 `failureReason`，禁止改写为 `accepted`。

## 10. 风险与回滚

| 风险 | 缓解 |
| --- | --- |
| toString 原生化改写全局 `Function.prototype.toString` 引入递归/性能问题 | WeakMap 映射 + 自举 `makeNative`；单测覆盖二级 `toString.toString` |
| 注入门控误跳过内核未真正覆盖的族 | 能力表以真实 CDP 探测为准，未确认一律注入并记 warning |
| UA 派生读取内核失败 | 保留 131 fallback 常量并标注 `source=fallback`，Validation 显示 `weak` |
| 鼠标流水线化破坏事件顺序/isTrusted | 仅对 move 系列流水线，press/release 仍同步；高敏走 S0 OS |
| DNS 探针改动影响既有 `DNSConsistent` 判定 | 先降级为 `inconclusive` 而非直接反转；受控域名到位再上三方证据 |
| 回滚 | 用户级 `~/.cursor/anti-lazy/scripts/rollback.py`；每个 Task 独立提交，便于单点回退 |

## 11. 变更同步要求

每个 Task 完成后：
1. 更新 `docs/02-current-state.md` 对应 live truth 行（带证据路径）。
2. 更新 `docs/04-improvement-backlog.md` 退出条件状态。
3. 评分变化只写进 `docs/47` §12 且必须附 raw artifact 路径。
4. 不把 target/schema 误报成 observed/live。
