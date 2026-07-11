# 51 扩展指纹面覆盖工程指导与验收

Updated: 2026-07-08 (Asia/Shanghai)

## 0. 定位

- **目标**：把 `backend/internal/behavior/environment_injector.go` 当前**未覆盖 / 覆盖过浅**的指纹面补齐到「与真实 Chrome 一致、与内核 seed 同源、跨作用域一致」，消除检测器可用的横向矛盾点。
- **三文档关系**：
  - `docs/49`：修**已有 hook**的真实性（toString 原生化、UA 一致、噪声质量、DNS 探针、鼠标稳定）。
  - `docs/50`：**战略维度**（出站身份统一、同一真实用户、连接隐匿、信任优先、成本不对称）。
  - `docs/51`（本文）：**指纹面广度**——把没 hook 的 surface 补上，且强制「同源派生 + makeNative + worker 作用域一致」。
- **强依赖**：所有新增 hook **必须**经 `docs/49` A1 的 `makeNative` 原生化，并经 `docs/50` C1 注入到 Worker 作用域，否则新增即新漏。
- **诚实边界**：能由 fingerprint-chromium 内核 seed 提供的，**优先内核**，注入层只补内核未覆盖项（避免 `docs/50` F6 的双层 lies）。无证据不写 observed。

## 1. 证据基线：注入器当前覆盖 vs 缺口

`environment_injector.go` 已 hook（本轮核实）：`navigator.webdriver/languages/platform/vendor/userAgent/hardwareConcurrency/deviceMemory/maxTouchPoints/doNotTrack`、`screen.colorDepth`、`plugins/mimeTypes`、`Intl.DateTimeFormat/getTimezoneOffset`、`canvas(getImageData/fillText/toDataURL)`、`WebGL(getParameter vendor/renderer + extensions)`、`Audio(AnalyserNode/AudioBuffer)`、`document.fonts(Proxy)`、`mediaDevices(enumerate/getUserMedia)`、`RTCPeerConnection`。

| 编号 | 缺口指纹面 | 现状 | 检测向量 |
| --- | --- | --- | --- |
| S1 | **`window.chrome` 对象** | 未构造 | headless/自动化检测查 `window.chrome.runtime/loadTimes/csi`；缺失或异常即判 bot |
| S2 | **`navigator.permissions.query`** | 未 hook | `Notification` 权限=`denied` 但 `Notification.permission=default` 矛盾是经典 headless 信号 |
| S3 | **WebGPU (`navigator.gpu`)** | `docs/00` 明确「只采集不伪造」 | `requestAdapter().info`、limits 泄露真实 GPU；与 WebGL renderer 矛盾 |
| S4 | **`speechSynthesis.getVoices`** | 未 hook | voice 列表随 OS/语言强特征；空列表或与 platform 矛盾 = 异常 |
| S5 | **Battery (`getBattery`)** | 未 hook | 真实设备有 level/charging；缺失或恒定值可疑 |
| S6 | **NetworkInformation (`navigator.connection`)** | 未 hook | `effectiveType/rtt/downlink` 与代理网络矛盾 |
| S7 | **`matchMedia` / prefers-*** | 未 hook | `prefers-color-scheme/reduced-motion/color-gamut/dynamic-range` 与 UA/OS 矛盾 |
| S8 | **ClientRects (`getClientRects`/`getBoundingClientRect`)** | 未 hook | 无噪声则字体渲染度量可跨站关联（CreepJS 用） |
| S9 | **`navigator.userAgentData`（高熵）** | 注入层未设置 | `getHighEntropyValues` 与 UA/`--fingerprint-brand-version` 不一致（`docs/49` A3 联动） |
| S10 | **WebGL 深参** | 仅 vendor/renderer/extensions | `UNMASKED_*`、`getShaderPrecisionFormat`、`MAX_*`、`readPixels`、WebGL2 未覆盖 |
| S11 | **plugins 真实性** | 透传 `profile.plugins`（常空） | 真实 Chrome 有 5 个 PDF 相关固定 plugin；空/异常结构可疑 |
| S12 | **字体 measureText 枚举** | 仅 `document.fonts` Proxy（Proxy 本身可检测） | 检测器用 `measureText` 宽度差枚举字体，绕过 `fonts.check` |
| S13 | **screen/window 几何一致性** | 仅 colorDepth | `availWidth/Height`、`outerWidth/Height`、`screenX/Y`、`devicePixelRatio` 与 `--window-size` 需自洽 |
| S14 | **`navigator.storage.estimate` / mediaCapabilities** | 未 hook | quota、codec 支持与设备画像矛盾（低优先） |

## 2. 统一实现原则（所有 S 任务通用，先读）

1. **同源派生**：所有值由 profile `HumanizeSeed` 确定性派生，保证同 profile 稳定、跨 profile 不同（禁止随机漂移导致 in-session drift）。
2. **内核优先**：先确认 fingerprint-chromium seed 是否已提供该 surface（`KnownIneffectiveFingerprintFlags` 参考）；内核已覆盖则**不注入**，只在缺口注入（防双层 lies）。
3. **makeNative 强制**：每个覆写经 `docs/49` A1 的 `makeNative`，`toString` 返回原生形态。
4. **Worker 作用域一致**：经 `docs/50` C1 的 `Target.setAutoAttach` 把同一脚本注入 worker，值与主线程一致。
5. **一致性优先于伪造**：宁可少改，也不制造 surface 间矛盾（矛盾比"不够假"更容易被抓）。

## 3. 任务分解（按优先级 P0/P1/P2）

> 优先级依据：检测器命中频率 + 与现有 hook 的矛盾风险。P0 = CreepJS/BrowserScan 常查且当前矛盾面。

### P0

#### Task S1 — `window.chrome` 对象构造
- **技术指导**：注入符合真实 Chrome 的 `window.chrome`（`runtime`、`loadTimes()`、`csi()`、`app`），值合理；仅在检测到缺失时构造；经 makeNative。
- **AC**：CDP smoke 下 `typeof window.chrome==='object'` 且 `window.chrome.runtime` 存在；`chrome.loadTimes` 为函数且 `.toString()` 含 `[native code]`；单测断言脚本含该族。

#### Task S2 — `navigator.permissions.query` 一致性
- **技术指导**：hook `permissions.query`，使 `notifications/geolocation/camera/microphone` 返回与 `Notification.permission`、mediaDevices 策略**自洽**的 state；经 makeNative。
- **AC**：`Notification.permission` 与 `permissions.query({name:'notifications'})` 不矛盾（对照断言）；worker 作用域一致；`go test ./backend/internal/behavior/... -count=1` passed。

#### Task S9 — `navigator.userAgentData` 高熵一致（联动 docs/49 A3）
- **技术指导**：注入层 `userAgentData.brands`/`getHighEntropyValues(platformVersion/fullVersionList/architecture/bitness)` 与内核 UA、`--fingerprint-brand-version` **同 major 同源**。
- **AC**：`getHighEntropyValues` 的 fullVersion major == UA major == 内核 major（三方一致）；`missing_probe` UA/core match 不回退。

#### Task S10 — WebGL 深参补全
- **技术指导**：扩展现有 webglPatch：覆盖 `UNMASKED_VENDOR/RENDERER_WEBGL`、`getShaderPrecisionFormat`、关键 `MAX_*` 参数与 renderer 字符串自洽；覆盖 WebGL2；`readPixels` 与 canvas 噪声策略一致；经 makeNative + worker。
- **AC**：worker 与主线程 `getParameter(UNMASKED_RENDERER)` 一致；WebGL2 与 WebGL1 renderer 一致；`go test` passed。

### P1

#### Task S3 — WebGPU 伪造
- **技术指导**：hook `navigator.gpu.requestAdapter`，返回与 WebGL renderer 家族一致的 adapter `info`（vendor/architecture/description）与合理 limits；无法一致时**统一禁用**（`navigator.gpu=undefined`，与部分真实环境一致）优于矛盾暴露。
- **AC**：`navigator.gpu` 要么与 WebGL 家族一致，要么干净缺失；不出现「WebGPU 高端 GPU vs WebGL 集显」矛盾（对照断言）。

#### Task S4 — speechSynthesis voices
- **技术指导**：`getVoices` 返回与 `--lang`/platform 一致的 voice 集（Windows + zh-CN 的合理集合）；经 makeNative；处理异步 `voiceschanged`。
- **AC**：voice 列表非空且 lang 与 profile locale 一致；跨 session 稳定。

#### Task S7 — matchMedia / prefers-*
- **技术指导**：hook `matchMedia`，`prefers-color-scheme/reduced-motion/color-gamut/dynamic-range` 返回与 OS/显示画像一致值；经 makeNative。
- **AC**：`matchMedia('(prefers-color-scheme: dark)').matches` 稳定且与 profile 画像一致；worker/iframe 一致。

#### Task S8 — ClientRects 噪声
- **技术指导**：对 `getClientRects`/`getBoundingClientRect` 加 seed 派生亚像素噪声（与 canvas 噪声同源策略），稳定不漂移；经 makeNative。
- **AC**：同 profile 同元素 rect 稳定；跨 profile 有差异；不破坏页面布局功能（XHS live 回归）。

#### Task S11 — plugins 真实性
- **技术指导**：默认注入真实 Chrome 的固定 PDF 插件集（`PDF Viewer`/`Chrome PDF Viewer` 等 5 项标准结构），而非空/自定义；与 `mimeTypes` 联动一致。
- **AC**：`navigator.plugins.length` 与结构匹配真实 Chrome 基线；`mimeTypes` 与 plugins 交叉一致。

#### Task S12 — 字体 measureText 一致 + 去 Proxy 痕迹
- **技术指导**：`document.fonts` 的 Proxy 方案改为**非 Proxy** 的 defineProperty/包装（Proxy 可被 `toString`/`Symbol` 探测）；并对 `measureText`/`offsetWidth` 字体探测路径与 allowlist 一致（未安装字体回退度量一致）。
- **AC**：`document.fonts` 无 Proxy 痕迹（`Object.prototype.toString.call` 正常）；measureText 枚举结果与 allowlist 一致；worker 一致。

### P2

#### Task S5 — Battery
- **技术指导**：`getBattery` 返回合理 level（seed 派生，缓慢变化）/charging；台式画像可返回 charging=true、level=1。
- **AC**：`getBattery()` resolve 合理值；跨 session 稳定；makeNative。

#### Task S6 — NetworkInformation
- **技术指导**：`navigator.connection.effectiveType/rtt/downlink` 返回与代理链路量级合理一致的值。
- **AC**：`connection.effectiveType` 存在且合理；不与 UA/平台矛盾。

#### Task S13 — screen/window 几何一致性
- **技术指导**：`screen.availWidth/Height`、`window.outerWidth/Height`、`screenX/Y`、`devicePixelRatio` 与 `--window-size`/`--force-device-scale-factor` 自洽（taskbar 高度等合理差值）。
- **AC**：几何字段互相自洽（无 avail > screen 等矛盾）；与启动参数一致。

#### Task S14 — storage.estimate / mediaCapabilities（低优先）
- **技术指导**：按设备画像返回合理 quota 与 codec 支持；无把握则不动（保持真实内核值）。
- **AC**：不引入与画像矛盾的值；默认可 `no-op`。

## 4. 覆盖率与验收模型

1. 扩展 `backend/internal/browser/runtime_projection.go` / observed 采集，新增 S1–S14 的 `declared / kernel / injected / observed` 归属，纳入 observed coverage gate。
2. 新增 `scripts/fingerprint_surface_coverage_gate.ps1`：统计 S1–S14 覆盖状态与「主线程 vs worker 一致性」，缺口列 `failureReason`。
3. 与 `docs/50` C3 `stealth_surface_audit` 合并展示 worker 一致性结果。

## 5. 总验收门禁（本文任务）

```powershell
go test ./backend/internal/behavior/... ./backend/internal/browser/... -count=1
.\scripts\fingerprint_surface_coverage_gate.ps1     # S1–S14 覆盖 + worker 一致
node scripts/full_observed_fingerprint_probe.mjs    # 真实内核 CDP 采集（现有）
node scripts/two_browser_missing_probe_matrix.mjs --product=personal-pilot
.\scripts\platform_99_gate.ps1 -Track all
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda     # 回归不得下降
```

**通过判据**：
- S1/S2/S9/S10（P0）覆盖且主线程 == worker 一致；`window.chrome`、permissions、userAgentData、WebGL 深参无矛盾。
- 所有新增 hook `.toString()` 含 `[native code]`（makeNative 生效）。
- `document.fonts` 无 Proxy 痕迹（S12）。
- CreepJS lies 数不升（有 raw 对照）；XHS live 12/12 不回归。
- 任一未达标 `blocked` + `failureReason`，禁止改写 accepted。

## 6. 优先级与 docs/49、docs/50 的合并执行

| 波次 | 本文任务 | 并行依赖 |
| --- | --- | --- |
| Wave 1 | S1、S2、S9 | 依赖 docs/49 A1 makeNative、A3 UA 源；docs/50 C1 worker |
| Wave 2 | S10、S3、S11、S12 | 依赖 S1 族与 worker 注入 |
| Wave 3 | S4、S7、S8、S13 | 独立 |
| Wave 4 | S5、S6、S14 | 低优先，可选 |

## 7. 风险与回滚

| 风险 | 缓解 |
| --- | --- |
| 新增 hook 未经 makeNative 反成新 lie | CI gate 强制：每个覆写符号都要在 makeNative 集合内 |
| 伪造值与内核真实值矛盾（双层） | 内核优先原则 + `declared/kernel/injected` 归属表，kernel 覆盖则跳过 |
| ClientRects/字体噪声破坏页面布局 | 亚像素级 + XHS live 回归；异常回退关闭 |
| WebGPU 伪造不一致 | 无法一致时统一干净缺失，优于矛盾 |
| 回滚 | 每 Task 独立提交；`~/.cursor/anti-lazy/scripts/rollback.py` |

## 8. 变更同步

每 Task 完成后更新 `docs/02-current-state.md`（observed 覆盖 + 证据路径）与 `docs/04-improvement-backlog.md`（退出条件）；`docs/00-master-todo.md` 中「反检测深度改进」对应项勾除并回指本文 Task 编号。
