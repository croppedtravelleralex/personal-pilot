# 50 不对称支配架构：让风控「不值得拦」

Updated: 2026-07-11 (Asia/Shanghai)

## 0. 战略命题

> 与风控处在**同一规则层**（比谁的指纹更假）时，永远无法 100% 隐形——因为风控可以无限收紧阈值。真正的 99 分，是把战争拉到**自己拥有绝对优势的维度**：让流量与真实用户存在足够交集，使得「拦截我」= 「误伤真实用户 + 承担转化率损失」，而「放行我」几乎零成本。当判定模型面对这种流量，其最优决策只有一个：**放行**。

本文是把该命题落成架构的 canonical 蓝图。它**位于 `docs/49` 之上**：

- `docs/49`：战术层——修补指纹/代理/行为的实现缺陷（toString 原生化、UA 一致、DNS 探针、鼠标往返等）。
- `docs/50`（本文）：战略层——**维度转移**：统一「同一真实用户」身份、收缩自动化暴露面、把操作路由到风控看不见的信任维度。
- 二者关系：`docs/49` 是 `docs/50` 的必要地基；`docs/50` 决定 `docs/49` 各项的**优先级排序理由**。

**诚实边界**：不承诺「数学不可检测」；不 fork 维护自研内核；无证据的项一律 `blocked`，不伪造 `accepted`。

---

## 1. 现状架构的真实事实（证据基线 F1–F7）

改动前必 Read 复核，避免过期。以下均为本轮源码核实：

| 编号 | 事实 | 证据 文件:行 | 战略影响 |
| --- | --- | --- | --- |
| F1 | 浏览器经 `exec.Command` 拉起内置 fingerprint-chromium（默认 139），分配 **`--remote-debugging-port`(TCP)**，`/json/version` 就绪后 attach page CDP | `backend/internal/browser/camoufox.go:65-75`、`backend/app_instance.go:308-322`、`config.yaml:50-62` | CDP over TCP 端口是可探测暴露面；非 nodriver 式 pipe/attach |
| F2 | 环境注入**仅** `Page.addScriptToEvaluateOnNewDocument`（+ 当前页一次 evaluate） | `backend/internal/behavior/environment_injector.go:271` | **不覆盖 Worker/ServiceWorker/OffscreenCanvas 作用域** → 主线程 vs worker 指纹矛盾（CreepJS lie 高危） |
| F3 | 所有 Go 出站 = 裸 `net/http`；Graph/trust 用默认 client、UA `PersonalPilot/1.0`、**不走 profile 代理** | `backend/internal/proxy/http_client.go:62-71,96`、`backend/internal/graphapi/client.go:45-48`、`backend/app_trust_graph.go:72-81` | API 与浏览器是**两个网络身份**（Go TLS + 不同出口 IP） |
| F4 | `transport/` 的 JA3/cipher/H2 仅 explain metadata；真实只合并 ALPN 且硬编码 Chrome family；无 JA4/uTLS | `backend/internal/transport/outbound.go:43-55`、`backend/internal/proxy/xray.go:609`、`singbox.go:695` | 传输指纹一致性未落地 |
| F5 | `bundle.LocalStorage` 死字段；harvest 不落盘 web storage；无启动 hydrate；sticky `Validate` 生产零调用；challenge 旋转 seed | `backend/internal/trust/harvest.go:139-159`、`backend/internal/proxy/sticky_session.go`、`backend/app_trust_graph.go:84-107` | 「同一真实用户」连续性不完整 |
| F6 | fingerprint-chromium **忽略多数 `--fingerprint-*` flag**（seed 模型）；CDP JS 是补偿层，但存在 toString/原型 lies（见 `docs/49` E1） | `backend/internal/browser/profile.go:16-18` `KnownIneffectiveFingerprintFlags` | 主指纹靠 fork seed；JS 补偿层若露馅反成负资产 |
| F7 | 30min refresh loop 是 **lazy**（仅剩 2h 到期才刷）、CLIENT_ID 依赖全局 env、失败静默 | `backend/app_stealth_engine.go:379-387` | token 边界过期风险；非生产级管家 |

**已具备的优势（保留，不推翻）**：fingerprint-chromium 内核级指纹（优于 stock Chrome）、per-profile canonical `user-data-dir` 持久化、Microsoft L0 TrustBundle 侧车、CDP-minimal workbench session、sing-box/xray/SSH 代理供应链、`docs/49` 规划的战术修补。

---

## 2. 对用户三项提案的工程评估

| 提案 | 评估 | 结论 |
| --- | --- | --- |
| **用 nodriver 直连系统 Chrome** | 系统 stock Chrome **无内核级指纹**，会**丢掉 fingerprint-chromium 现有优势**；nodriver 的真正价值是**连接隐匿**（避免 `--remote-debugging-port` TCP、去 CDP 痕迹、pipe 传输） | **不替换内核**。保留 fingerprint-chromium，**吸收 nodriver 的连接隐匿技术**（见 Pillar C） |
| **curl_cffi 作为备用 HTTP 层，JA4 与浏览器一致** | 命中 F3/F4 真实痛点：API/运维出站暴露 Go TLS。curl_cffi（或 uTLS impersonate）能让**非浏览器出站**也具备浏览器 JA3/JA4/H2 | **采纳**，作为 Pillar A 的出站身份统一层 |
| **nodriver 自动加载 Profile 的 Cookie/LocalStorage，风控视角同一用户** | 命中 F5：当前 localStorage 未接入、API/浏览器异构身份 | **采纳并强化**为 Pillar B「同一真实用户引擎」 |

**核心判断**：最优解不是「换 nodriver」，而是 **fingerprint-chromium 内核 + nodriver 连接隐匿 + curl_cffi 出站身份统一 + 统一会话 hydrate + 信任优先路由**的合成体。

---

## 3. 五大支柱（Pillars）

```text
Pillar A  出站身份统一   —— 所有出站（浏览器/API/运维）同一 JA3/JA4/H2/UA/出口 IP
Pillar B  同一真实用户   —— cookie+localStorage+IndexedDB+sticky IP+seed 锁，跨会话/跨路径一致
Pillar C  连接隐匿收敛   —— 去 remote-debugging-port TCP、Worker 作用域注入、去 CDP 痕迹
Pillar D  信任优先路由   —— 尽量走 API/会话继承，少碰可检测的浏览器自动化面（L0 优先）
Pillar E  成本不对称度量 —— 量化并证明「拦截成本 > 放行成本」，作为发布门禁
```

对应权重（沿用 `docs/46` L0–L6 语义）：B/D 属 L0 信任层（最高杠杆），A 属 L1 网络，C 属 L2/L3 引擎与控制平面，E 属 L6 反馈。

---

## 4. Pillar A — 出站身份统一（对标 curl_cffi）

**目标**：浏览器页面流量、API-first 流量、运维探测流量，在风控看来是**同一台设备同一浏览器**——相同 UA、UA-CH、JA3/JA4、H2 SETTINGS/帧序、头序，且**同一出口 IP**。

### Task A0 — 出站身份基线冻结（前置）
- **技术指导**：定义 `EgressIdentity{ uaMajor, ja3, ja4, h2Settings, headerOrder, acceptLang, exitIP }`，由 `docs/49` A3 的 core 版本单一真相源派生；作为 A1/A2 的输入。
- **AC**：单测断言同一 profile 的 EgressIdentity 三路（browser/api/ops）取值同源；`go test ./backend/internal/browser/... -count=1` passed。

### Task A1 — 非浏览器出站 impersonation 层（curl_cffi 等价物）
- **现状/根因**（F3/F4）：Go `net/http` 出站暴露 Go JA3；`transport/` 仅 ALPN 生效。
- **技术指导**（二选一，先做 spike 决策）：
  1. **进程内 uTLS**：引入 `utls`（mihomo 已间接依赖）自建 `http.RoundTripper`，按 `EgressIdentity` 选 ClientHello preset（Chrome_131/139…）+ `golang.org/x/net/http2` 自定义 SETTINGS/帧序；替换 `graphapi`、代理探测、`iphealth` 的 client。
  2. **curl_cffi 子进程**：打包 `curl_cffi`/`curl-impersonate` 作为 sidecar，Go 通过本地 RPC 调用，`--impersonate chrome131`；隔离但引入 Python/二进制依赖。
- **推荐**：优先 **uTLS 进程内**（无额外运行时依赖，与现有 Go 栈一致）；curl_cffi 作为 fallback/对照。
- **改动文件**：新增 `backend/internal/transport/impersonate/roundtripper.go`；改造 `graphapi/client.go`、`proxy/http_client.go`、`proxy/iphealth.go`。
- **AC**：
  - AC1：经该层访问 `tls.peet.ws`/`tls.browserleaks.com`，JA3/JA4 与同 profile 浏览器**一致**（记录三方对照 raw）。
  - AC2：`graphapi` 请求头序/UA/UA-CH 与浏览器一致（抓包断言）。
  - AC3：`go test ./backend/internal/transport/... -count=1` passed；无回归 mihomo 依赖冲突。

### Task A2 — 出站 TLS/JA4 与 UA 基线绑定（升级 docs/49 C4）
- **技术指导**：把 `docs/49` C4 的 `ChromeMajorTLSBaseline` 扩展到 **JA4**（不只 JA3），sing-box/xray 出站按 profile family 选模板；Metadata→可执行配置的边界仅在**能安全写入**的字段上放开（cipher/curve 顺序经 utls，代理节点侧保持不注入未知字段）。
- **AC**：`tlsUaCoherent`（含 JA4）≥ 2/3；单测覆盖 major 变更→模板变更；不把模板名冒充 observed。

### Task A3 — API 走 profile 代理同出口（联动 Pillar B）
- **现状/根因**（F3）：Graph client 用默认 client、不经代理。
- **技术指导**：`graphClientForProfile` 注入 A1 RoundTripper + profile 的 sing-box 桥出口；使 API 与浏览器**同出口 IP**。
- **AC**：Graph 调用出口 IP == 浏览器出口 IP（同 profile，记录对照）；失败降级有 warning，不静默。

---

## 5. Pillar B — 同一真实用户引擎（对标 nodriver profile hydrate）

**目标**：同一 profile 无论何时、走浏览器还是 API，风控看到的是**同一个有历史的真实用户**：cookie、localStorage、IndexedDB、出口 IP、指纹 seed、时区/语言全部锁定且连续。

### Task B1 — 统一会话 hydrate（启动/首导航前）
- **现状/根因**（F5）：`bundle.LocalStorage` 死字段；仅 user-data-dir 被动持久 + 可选事后 setCookie。
- **技术指导**：
  1. 扩展 harvest：`BundleFromHarvest` 落盘 localStorage/sessionStorage/IndexedDB 关键键（`harvest.go:139-159`）。
  2. 新增启动 hydrate：首导航前经 CDP 注入 cookie（补 `sameSite/expires/partitionKey`）+ `Page.addScriptToEvaluateOnNewDocument` 预置 localStorage/IndexedDB 种子。
  3. 与 user-data-dir 原生持久**去重**（避免 duplicate/mismatch，`app_stealth_engine.go:327-330`）。
- **AC**：
  - AC1：清空 user-data-dir 后，仅凭 TrustBundle hydrate 能恢复 localStorage 关键键（CDP 读回断言）。
  - AC2：cookie 注入含 sameSite/expires，与磁盘 cookie 无冲突（单测 + CDP 校验）。
  - AC3：`go test ./backend/internal/trust/... ./backend/internal/behavior/... -count=1` passed。

### Task B2 — Sticky 出口 IP 强制 + 反 reconcile 冲突
- **现状/根因**（F5）：`StickySessionTracker.Validate` 生产零调用；IP drift 检测可能**主动换代理**破坏连续性。
- **技术指导**：
  1. 启动/路由时 `Validate` sticky 绑定，TTL 内**强制**复用同出口；持久化 binding（跨重启，替代纯内存 map）。
  2. IP drift 时区分「代理故障」与「正常同城漂移」：仅前者 reconcile，后者保连续（`app_proxy_reconcile.go:38-66`）。
- **AC**：同 profile 连续 10 次启动出口 IP 稳定（drift 矩阵，联动 `docs/49` B2）；sticky Validate 有生产调用路径（覆盖率断言）。

### Task B3 — 指纹 seed 生命周期锁
- **现状/根因**（F5）：challenge feedback 可 `ProfileRotateFingerprintSeed` 换 UUID，破坏「同一用户」。
- **技术指导**：seed 旋转改为**高门槛事件**（仅硬封号级信号，且旋转即视为「新用户」需重建 trust）；默认 challenge 走 pause/切 API，不动 seed。
- **AC**：单测：普通 403/验证码不触发 seed 旋转；仅显式硬信号触发，且旋转会重置 trust continuity 标记。

### Task B4 — L0 bootstrap 默认化 + 平台泛化
- **现状/根因**（F5 G6/G7）：harvest/save 仅 `stealth-autopilot`/`auto-99` 标签触发；平台偏 Microsoft。
- **技术指导**：普通启动也做「登录态 harvest → bundle save」（可开关）；把 harvest JS 泛化到 XHS 等平台（`platform-packs/*/trust.yaml` 定义 cookie 域与 storage 键）。
- **AC**：XHS profile 登录一次后，重启无需重登（live 验收）；bundle 覆盖 XHS cookie 域。

---

## 6. Pillar C — 连接隐匿收敛（吸收 nodriver 技术）

**目标**：把「被自动化控制」的可检测痕迹降到最低——不暴露 CDP TCP 端口、Worker 作用域指纹一致、无 `cdc_`/CDP 泄露。

### Task C1 — Worker/OffscreenCanvas 作用域注入（高危缺口 F2）
- **现状/根因**（F2）：注入仅覆盖文档，不覆盖 Web/Service Worker、OffscreenCanvas → 主线程 vs worker 指纹矛盾。
- **技术指导**：
  1. 用 `Target.setAutoAttach{autoAttach:true, flatten:true, waitForDebuggerOnStart:true}` 捕获 worker target，对其也 `addScriptToEvaluateOnNewDocument`/评估同一 hook（经 `docs/49` A1 makeNative）。
  2. 覆盖 `OffscreenCanvas`、`WorkerNavigator` 的 hardwareConcurrency/deviceMemory/userAgent/webgl。
  3. 保持与主线程**同源同值**（同 seed 派生）。
- **AC**：
  - AC1：CreepJS/自建探针中 worker-scope `navigator.hardwareConcurrency/userAgent/webgl` 与主线程一致（对照断言）。
  - AC2：worker 内 `WebGLRenderingContext.getParameter` 返回与主线程相同 vendor/renderer。
  - AC3：`go test ./backend/internal/behavior/... -count=1` passed。

### Task C2 — CDP 连接暴露面收敛（remote-debugging-pipe spike）
- **现状/根因**（F1）：走 TCP `--remote-debugging-port`，本地可探测。
- **技术指导**：
  1. **Spike**：验证 fingerprint-chromium 是否支持 `--remote-debugging-pipe`（stdio 管道），若支持则改 pipe 传输，去掉 TCP 端口监听。
  2. 若 fork 不支持 pipe：端口绑定 `127.0.0.1` 随机高位端口 + 启动后校验无第三方可连；评估 `Runtime.enable` 泄露（`Runtime.consoleAPICalled`/`cdc_` 变量）并清理。
  3. 保持 `docs/49` A4 的 CDP-minimal（用完即 disable）。
- **AC**：
  - AC1（pipe 可行）：实例经 pipe attach，`netstat` 无 CDP TCP 端口；自动化功能不回归。
  - AC2（pipe 不可行）：记录 fork 限制为已知边界，端口仅本地可连 + 无 `cdc_` 全局变量（CDP evaluate 断言）。
  - AC3：XHS live 12/12 不回归。

### Task C3 — 自动化痕迹清理审计
- **技术指导**：新增 `stealth_surface_audit`：检测 `navigator.webdriver`、`window.cdc_*`、`Runtime.enable` 可观测副作用、CDP 端口可达性、Worker 一致性，产出结构化报告。
- **AC**：审计报告全绿（webdriver=false、无 cdc_、无异常 CDP 暴露、worker 一致）；纳入 `platform_99_gate`。

---

## 7. Pillar D — 信任优先路由（L0 dominance）

**目标**：把尽可能多的操作从「可检测的浏览器自动化」搬到「风控几乎无法区分的 API/会话继承」维度——这是「不值得拦」的核心：日常操作根本不经过登录/自动化面。

### Task D1 — Operation Matrix 落地
- **技术指导**：为每个平台定义操作→路由表（读/列表/通知→API；UI 独占→浏览器 S1；高敏登录/OTP→S0 OS）；默认走能力最高、暴露最低的路径（沿用 `docs/46` §4 Operation Matrix + Input Plane）。
- **AC**：XHS/Outlook 场景中「读」类操作走 API 占比可度量且 ≥ 目标阈值；审计字段记录每操作实际 plane。

### Task D2 — 30min token 管家生产化（修 F7）
- **现状/根因**（F7）：lazy 刷新、env CLIENT_ID、失败静默。
- **技术指导**：改为主动刷新（到期前充足冗余）、per-profile OAuth app 支持、失败重试 + 可见告警 + 降级切浏览器路径。
- **AC**：单测：token 在到期前被刷新；刷新失败产生可见事件；`go test ./backend/... -count=1` passed。

### Task D3 — 挑战反馈闭环强化（L6）
- **技术指导**：`AsymmetricRecordChallenge` → 默认 pause/切 API/换出口，**不动 seed**（联动 B3）；记录挑战率并反馈到 E。
- **AC**：挑战后自动降级路径可复现；挑战率进入成本度量。

---

## 8. Pillar E — 成本不对称度量（发布门禁）

**目标**：把「拦截成本 > 放行成本」从口号变成**可度量、可门禁**的指标。

### Task E1 — 不对称度量模型
- **技术指导**：扩展 `EvaluateStealthMatrix`，输出 `interceptCostIndex`（真实用户交集度：住宅 IP 纯净度、trust 有效性、行为熵、历史连续性）与 `releaseCostIndex`，比值 < 1 视为「不值得拦」。
- **AC**：报告输出双指数与比值；无 trust/住宅 IP 时比值升高并给缺口清单（沿用 cap ~88 语义）。

### Task E2 — 真实用户交集证据
- **技术指导**：量化「与真实用户交集」：住宅 IP + 有效 trust + 人类时间窗 + 连续 storage/cookie 历史 + 一致 worker 指纹；缺任一项在报告标注。
- **AC**：`dominance_gate.ps1`：交集项全绿才 pass；任一缺失 `blocked` 且不可伪造 accepted。

---

## 9. 执行波次与依赖

```text
Wave 0（地基，来自 docs/49）— ✅ 2026-07-10 实现 / 2026-07-11 复核
  docs/49 A1 toString ✅ · A3 UA 单一真相源 ✅ · C1 DNS ✅ · D1 鼠标 🟡（OS fallback 未关）

Wave 1（同一用户 + 连接隐匿，最高杠杆）— 本地下一批
  B1 会话 hydrate ← docs/49 A3
  C1 Worker 作用域注入 ← docs/49 A1 makeNative   ★高危优先（makeNative 已落地，可开工）
  B2 sticky 强制
  A0 出站 identity 基线冻结

Wave 2（出站身份统一）
  A1 impersonation 层(uTLS) · A2 JA4 基线 · A3 API 同出口
  D2 token 管家 · B3 seed 生命周期锁

Wave 3（隐匿收敛 + 支配度量）
  C2 remote-debugging-pipe spike · C3 痕迹审计
  D1 Operation Matrix · D3 挑战闭环
  E1 不对称度量 · E2 交集证据 gate · B4 L0 默认化+平台泛化
```

依赖要点：`C1` 依赖 `docs/49 A1`（makeNative 已落地）；`A1` 依赖 `A0`；`A3`/`B2` 相互支撑（同出口）。

---

## 10. 里程碑与评分

评分只在**有 raw artifact** 后刷新 `docs/47` §12；`docs/49` 与本文的 gate 都通过才算维度达成。

| 阶段 | 支配维度达成 | 关键证据 |
| --- | --- | --- |
| Wave 1 | 同一用户连续性 + worker 一致 | B1 localStorage 恢复、C1 worker=main 一致、B2 IP 10-run 稳定 |
| Wave 2 | 出站身份统一 | A1 API JA3/JA4==浏览器、A3 同出口 IP |
| Wave 3 | 连接隐匿 + 可度量支配 | C2/C3 无 CDP 暴露、E2 交集全绿、Operation Matrix API 占比达标 |

**目标态**：任一维度上，风控要拦我就必须同时误伤「住宅 IP + 有效登录态 + 连续历史 + 一致指纹 + 真实 TLS」的真实用户群——即 `interceptCostIndex / releaseCostIndex > 1`，其最优决策为放行。

---

## 11. 总验收门禁（每波次末）

```powershell
# Go 单测（按改动包）
go test ./backend/internal/behavior/... ./backend/internal/browser/... ./backend/internal/trust/... ./backend/internal/transport/... ./backend/internal/proxy/... -count=1

# 平台离线门禁 + 新增支配门禁
.\scripts\platform_99_gate.ps1 -Track all
.\scripts\stealth_surface_audit_gate.ps1     # C3：webdriver/cdc_/CDP 暴露/worker 一致
.\scripts\dominance_gate.ps1                 # E2：真实用户交集全绿

# 出站身份一致性（A 落地后）
node scripts/egress_identity_matrix.mjs      # 浏览器 vs API vs ops 的 JA3/JA4/UA/出口 IP 对照

# 同一用户连续性（B 落地后）
node scripts/two_browser_drift_matrix.mjs --runs=10   # cookie/localStorage/IP/指纹跨会话稳定

# 平台 live（回归不得下降）
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda
```

**通过判据**：所有 `go test` passed；worker 作用域与主线程指纹一致；API 与浏览器 JA3/JA4/UA/出口 IP 一致；同 profile 10-run cookie/localStorage/IP 稳定；`stealth_surface_audit` 无 CDP 暴露；`dominance_gate` 交集全绿；XHS live 12/12 不回归。任一未达标记 `blocked` + `failureReason`，禁止改写 accepted。

（新增脚本 `stealth_surface_audit_gate.ps1`、`dominance_gate.ps1`、`egress_identity_matrix.mjs` 为本文任务产出物，尚不存在。）

---

## 12. 风险与回滚

| 风险 | 缓解 |
| --- | --- |
| uTLS 与 mihomo 间接依赖版本冲突 | 先 spike 锁版本；隔离到 `transport/impersonate` 包；curl_cffi 子进程作为 fallback |
| Worker 作用域注入影响页面功能/性能 | 仅注入指纹相关 hook，经 makeNative；灰度 + XHS live 回归 |
| remote-debugging-pipe fork 不支持 | 作为 spike，不支持则记为已知边界并走端口本地化 + 痕迹清理，不阻塞其他任务 |
| API 走 profile 代理增加延迟/失败面 | 保留直连 fallback，但降级要显式告警（不静默） |
| 会话 hydrate 与磁盘 profile 冲突（duplicate cookie） | hydrate 前做去重/一致性校验；单测覆盖 |
| seed 生命周期锁降低对抗灵活性 | 硬信号仍可旋转，但旋转=显式「新用户」，需重建 trust |
| 回滚 | 每 Task 独立提交；`~/.cursor/anti-lazy/scripts/rollback.py` |

## 13. 变更同步要求

每 Task 完成后：更新 `docs/02-current-state.md`（live truth + 证据路径）、`docs/04-improvement-backlog.md`（退出条件）、评分只写进 `docs/47` §12 且附 raw artifact；不把 target/schema/metadata 误报成 observed/live。
