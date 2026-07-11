# PersonaPilot / AdsPower / BitBrowser 深度横评

Updated: 2026-07-08 (Asia/Shanghai)

## 0. 结论先行

本项目 PersonaPilot 已经具备一个“可审计、可改造、本机自用”的指纹浏览器工作台雏形：有本机桌面入口、profile/core/proxy/workbench/API、CDP 环境注入、fingerprint health、identity report、sing-box/xray/SSH 代理桥、Windows OS input plane、行为 replay 和 evidence board。它的强项是透明、可定制、可把每个能力落到源码和报告证据。

但如果按“商业闭源指纹浏览器成品”比较，AdsPower 和 BitBrowser 仍明显领先于开箱成熟度、团队协作、批量 profile 生命周期、RPA 模板生态、长期兼容性、客服/支付/套餐和大规模用户验证。本项目当前更像“可继续长成商业级内核和运营台的自研底座”，不是已经全面追平 AdsPower/BitBrowser 的闭源成品。

最硬的判断：

| 维度 | 当前领先者 | 判断 |
| --- | --- | --- |
| 可审计性 / 可二开 | PersonaPilot | 源码、报告、gate 都可查；闭源产品只能看 API/文档/黑盒结果 |
| 开箱多账号产品成熟度 | AdsPower | profile、团队、RPA、同步器、套餐和文档最完整 |
| 性价比 / 免费额度 | BitBrowser | 官网价格页标注永久免费 10 profiles；本机 7.1.3 已装且本地 API 可访问 |
| 指纹真实性的商业成熟度 | AdsPower / BitBrowser | 两者都有长期闭源内核积累；PersonaPilot 主要是 JS/CDP 注入 + 自有验证体系 |
| IP/代理治理透明度 | PersonaPilot | sing-box/xray/SSH/检测链路源码透明；但缺商业代理生态和真实 provider 验收 |
| RPA / 同步生态 | AdsPower / BitBrowser | 两者都有官方 RPA、同步器和团队功能；PersonaPilot 有底层能力但模板生态弱 |

## 1. 评测边界

本次做了四类取证：

1. 仓库源码与文档：`README.md`、`docs/README.md`、`docs/02-current-state.md`、`docs/13-adspower-deep-comparison.md`、指纹/代理/行为核心 Go 代码、已有 JSON evidence。
2. 本机闭源软件观测：BitBrowser 已安装且 Local API 可用；AdsPower 已安装，但 Free 账号 API & MCP 为付费墙，本轮只作安装、官方资料和免费 UI/手工观察。
3. 官方公开资料：AdsPower、BitBrowser 官网和官方帮助文档。
4. 本地验证：`tsc --noEmit`、后端相关包 `go test`、`git diff --check`。

没有做的事：

- 没启用 AdsPower paid/trial API access，也没把 AdsPower 纳入自动化 launch 矩阵。
- 已跑当前可执行 PersonaPilot + BitBrowser 黑盒矩阵：基础 launch-loop、CreepJS / BrowserLeaks / BrowserScan / Pixelscan 页面可达性、TLS/H2 echo、WebRTC probe、行为探针、截图、HAR-lite、CDP trace。详见第 10 节。
- 没使用完整 US/JP/DE 同类住宅代理账号做三地区对等矩阵；DE 仍缺同类节点，UDEAL 当前只有 LA 单出口。
- 没把任何第三方闭源软件反编译或绕过授权。

因此，“闭源产品指纹性能”现在有了首批同代理黑盒 artifact，但仍不能外推为长期真实平台过检率；“实际过检测率”仍需要同账号池、同目标站、长期重复采样和合法授权场景。

## 2. 本机商业软件观测

BitBrowser：

- Windows 卸载表显示：`比特浏览器 7.1.3`，Publisher `BitBrowser`，卸载路径 `D:\SelfMadeTool\bitbrowser\Uninstall 比特浏览器.exe`。
- 主程序：`D:\SelfMadeTool\bitbrowser\比特浏览器.exe`，FileVersion `7.1.3`，ProductVersion `7.1.3.0`。
- 进程监听：`127.0.0.1/:: :54345`，进程名 `比特浏览器`。
- 本地 API 只读探测：
  - `GET http://127.0.0.1:54345/` 返回工作台 HTML。
  - `POST /browser/list` 返回 `success=true`，`totalNum=0`。
  - `POST /browser/fingerprint/random` 返回 `success=false`，`msg=请传入 browserId`，说明接口存在但需要 profile ID。
- 结论：本机 BitBrowser 可观测为已安装、已运行、本地 API 可用，但没有本地 profile 可用于实际指纹/IP黑盒测试。

AdsPower：

- 2026-07-07 已下载官方 8.6.3 Windows x64 安装器，SHA256 `99AC2ABEF961520919B8376448923DCE28A5BDBB543B77895BC3BE40AE90BD90`，Authenticode valid。
- 已静默安装到 `D:\SelfMadeTool\ads\AdsPowerGlobal`，主程序为 `AdsPower Global.exe`，卸载表显示 `AdsPower Global 8.6.3`。
- `127.0.0.1:50325/status` 已确认可用，但 profile list 返回 `Require api-key`。
- 用户截图确认当前 Free 账号 API & MCP 仅限付费套餐。
- 结论：本机已有 AdsPower 安装实例，但本轮不付费时不能跑 API 自动化 profile 级指纹/IP黑盒测试；本轮 AdsPower 只保留安装、官方资料和免费 UI/手工观察，不进入 launch 矩阵。

## 3. 官方公开资料摘要

AdsPower 官方资料显示：

- AdsPower profile 是一组独立浏览器设置，模拟 User Agent、IP、Canvas 等指纹，并隔离环境；价格页标注 Free 方案 2 profiles，Professional/Business/Enterprise 分层，Local API 请求限额从 120/300/600 requests/minute 分层。
- Profile 创建流程内置代理填写和 Check proxy，支持 SunBrowser/FlowerBrowser，即 Chrome / Firefox stealth browsers。
- Local API Open Browser V2：启动 profile 后返回调试接口，可接 Selenium/Puppeteer。
- 官方 RPA 文档定义了流程、调度、任务日志、线程数、权限。
- 官方 Synchronizer 可以把主窗口操作同步到多个已打开 profile，并有窗口平铺、统一大小、文字输入、模拟动作等功能。

BitBrowser 官方资料显示：

- 官方文档称其为 HongKong Bit-Internet Technology Limited 开发的多账号防关联指纹浏览器，基于 Chrome 和 Firefox 开源内核深度开发。
- 价格页标注永久免费 10 browser profiles、每日打开 50 次、支持 HTTP/HTTPS/SOCKS5/SSH、Local API、RPA、同步器、扩展/脚本市场、多人协作。
- Local Service Guide：安装并登录后，可在 Settings 找到 Local API URL/port；API 用 POST，返回 `success/data/msg`。
- Browser Profiles API 支持 profile 新增/修改/删除/列表、打开/关闭、返回 ws/http debug address、coreVersion、chromedriver path、代理检测、随机指纹、窗口排列、RPA、cookies、文件读取等。
- 同步系统支持主控窗口同步鼠标键盘到多个被控窗口，包含窗口排列、标签页、同文本/不同文本模拟输入、验证码识别、随机延迟等。
- Upgrade Log 记录了 7.0.4 代理兼容、字体/媒体设备随机指纹优化，7.0.5 Chrome 134、CPU 优化和 Cloud Phone synchronizer，7.0.6 安全机制和性能优化。

## 4. PersonaPilot 源码能力画像

### 4.1 指纹能力

已落地：

- `backend/internal/browser/runtime_projection.go` 声明 80 个 first-family control fields，覆盖 UA、OS、locale/timezone、screen、硬件、WebGL/canvas/audio/fonts/media、proxy、行为等。
- `runtime_materialize.go` 会在启动前补默认 fingerprint args：Chrome/Windows/UA/窗口/硬件并发/内存/WebGL/canvas/audio/fonts/touch/WebRTC/AutomationControlled 等。
- `environment_injector.go` 通过 CDP `Page.addScriptToEvaluateOnNewDocument` 注入 JS hook，覆盖 navigator.webdriver/languages/platform/vendor/UA/hardwareConcurrency/deviceMemory、plugins/mimeTypes、timezone、canvas、WebGL、audio、fonts、mediaDevices、WebRTC candidate filter。
- `fingerprint_health.go` 和 `identity_report.go` 会对 CDP 采集到的 fingerprint snapshot 打分，检测 UA、platform、screen、canvas、WebGL、font、expected args、webdriver、coherence、profile persistence、proxy/network、behavior naturalness。
- 已有 evidence：`observed-fingerprint-coverage-gate-1782793511983.json` 为 `passed_full_observed_fingerprint_coverage`，summary 显示 observedSignalCount `470`、targetSignalCount `450`、coveredFamilyCount `12/12`。

边界：

- 这是“可观测信号覆盖 + JS/CDP 注入 + 自有评分”，不是 Chromium/Firefox fork 级内核改造。
- `FullRuntimeProjectionReport` 会把不少字段作为 implicit control 计入 80/80；它适合做内部完整性清单，不等于每个字段都有独立浏览器运行时真值证明。
- `profile-browser-comparison-1782793600653.json` 状态是 `partial_comparison_only`，原因是 desktop WebView 与 profile browser evidence 相差 `14329.64` 分钟，超过 120 分钟窗口。这说明“同轮 desktop/profile 对照”仍未闭环。
- Camoufox 路径会过滤 Chromium-only fingerprint flags，当前更像 Firefox/Camoufox 可启动能力，不应宣称拥有等价指纹注入深度。

### 4.2 IP / 代理能力

已落地：

- 浏览器启动链路可把 profile proxy 转为浏览器 `--proxy-server`，并在需要时走 sing-box/xray 桥。
- `proxy_launch.go` 修正 `socks5h://` 到 Chromium 可接受的 `socks5://`，并追加 `--host-resolver-rules` 与 `--webrtc-ip-handling-policy=disable_non_proxied_udp`。
- `app_instance.go` 会在启动前通过 proxy health 的 country 调 `ApplyGeoLocale`，把语言/Accept-Language/timezone 向代理国家对齐。
- 代理层有 DNS/WebRTC leak probe、verify v2 稳定 streak、sticky session tracker、HTTP exit IP fetch。
- 文档记录 UDEAL 经 `?pp_via_ssh=panda` 可走本机 SSH local forward，再接 sing-box/上游代理。

边界：

- `ProbeDNSConsistency` 用系统 resolver 解析目标 host 再和 exit IP 比对，这只是启发式告警，不是真正的浏览器上下文 DNS 泄漏证明；CDN/Anycast 场景容易误判。
- `ApplyGeoLocale` 当前国家表很小，US 固定 New York，不做州/城市级 timezone；真实 IP 地理一致性仍粗。
- 真实 provider credential-backed smoke 是唯一保留 blocker；没有真实服务商账号、余额和 API key，不能宣称 provider 生产闭环。
- 没有商业级代理市场、自动购买/续费、住宅代理库存、质量 SLA。

### 4.3 行为 / 自动化能力

已落地：

- `inputplane` 可在 OS 与 CDP 间路由，click/type/double-click/right-click/click-offset 可走 OS plane。
- Windows 鼠标使用窗口消息，键盘使用 `SendInput`；比纯 CDP click/type 更接近真实输入路径。
- `humanize/trajectory.go` 有 Fitts Law、ease path、粉噪、四阶段点击、双击、拖拽、右键、click target 偏移模型。
- `primitive_executor.go` 支持 wait/readiness/content stable、scroll、focus/blur、hover/click/type、DOM snapshot、screenshot、evaluate、natural browsing 等 primitive。
- `live-replay-runtime-gate-1782793512806.json` 为 `passed_full_local_replay_runtime`，summary 显示 targetEventCount `450`、replayedEventCount `461`、contractOnlyEventCount `0`。

边界：

- Windows `KeyboardSender.TypeString` 对非 ASCII 字符直接 skip。对于小红书/中文登录/中文内容，这是实际能力缺口。
- 行为 replay gate 是本地 deterministic harness，不等于目标站真实行为风控全链路通过。
- AdsPower/BitBrowser 有更完整的 RPA/同步器/模板/团队权限生态，本项目目前是底层能力强，产品化生态弱。

## 5. 横向评分

评分为本次审计主观工程评分，满分 10；闭源产品未做同条件黑盒跑分，分数代表“公开功能成熟度 + 可观测性 + 合理推断”。

| 维度 | PersonaPilot | AdsPower | BitBrowser |
| --- | ---: | ---: | ---: |
| 指纹信号覆盖 | 7.5 | 8.5 | 8.0 |
| 指纹真实性/长期抗检测 | 5.8 | 8.6 | 8.1 |
| IP/代理协议与启动链路 | 7.6 | 8.2 | 8.3 |
| IP 质量治理/商业生态 | 5.8 | 8.0 | 7.6 |
| WebRTC/DNS/TLS 泄漏治理 | 6.8 | 8.0 | 7.8 |
| 行为/RPA/同步 | 6.9 | 8.8 | 8.4 |
| Profile/session 生命周期 | 7.0 | 9.0 | 8.4 |
| API/自动化集成 | 8.2 | 8.3 | 8.5 |
| 可观测性/调试证据 | 8.8 | 6.5 | 6.3 |
| 团队协作/权限/商业运营 | 3.5 | 9.0 | 8.2 |
| 成本/自由度 | 8.5 | 6.5 | 8.8 |
| 可维护/可二开 | 9.0 | 3.0 | 3.0 |

总体判断：

- PersonaPilot：研发底座强，商业成品弱。
- AdsPower：商业完整度最强，黑盒不可审。
- BitBrowser：低成本和 API 面强，本机可跑但未建 profile；透明度同样受闭源限制。

## 6. 维度详评

### 指纹性能

PersonaPilot 的优势是能精确知道“声明了什么、注入了什么、采集到了什么、报告怎么算分”。这在闭源产品里通常很难拿到。它的 `450/450` evidence 能证明采集和覆盖框架有了，但不是“真实反检测过关率 100%”。

AdsPower/BitBrowser 的优势是长期商业内核、profile 生成器、Chrome/Firefox 双核、团队用户规模和黑盒更新节奏。它们更可能在真实站点上有经验积累，但用户无法独立审计内部实现。

短板对照：

- PersonaPilot：更依赖 JS/CDP hook；PluginArray/MimeTypeArray、ClientRects、Audio/Canvas 稳定扰动、UA-CH、WebGPU、TLS/HTTP2、字体、媒体设备等还需要更深同轮检测。
- AdsPower：功能成熟但闭源；实际参数与检测站通过率只能黑盒验证。
- BitBrowser：API 和 random fingerprint 面广；但本机无 profile，无法验证 7.1.3 实际输出质量。

### IP 性能

PersonaPilot 的代理链路工程味很强：sing-box/xray/SSH bridge、认证代理适配、Chromium socks5h 修正、host-resolver-rules、防 WebRTC 非代理 UDP、sticky session、verify streak 都有源码。但它缺商业代理库存和真实 provider 验收，IP “质量”本身取决于你接入的上游。

AdsPower/BitBrowser 更像产品化代理管理：创建 profile 时填代理并检查，支持保存/随机/批量/代理检测，BitBrowser 公开写了 HTTP/HTTPS/SOCKS5/SSH。它们对普通用户更省心，但具体检测逻辑和泄漏保护不透明。

真实 IP 性能横评必须后补：

- 同一批住宅代理、同地区、同协议。
- 每个产品 10 次启动，记录 cold start、首包、页面加载、出口 IP hash 稳定、IP 国家/城市、ASN/hosting/proxy 标记。
- 浏览器内跑 WebRTC/DNS/BrowserLeaks，外部抓 TLS/JA3/HTTP2 指纹。
- 同账号/同站点策略下测封控率，只能在合法授权场景做。

### 自动化 / RPA

AdsPower/BitBrowser 直接胜在产品化：同步器、RPA、任务日志、权限、模板、窗口管理都是用户可操作层。

PersonaPilot 的优势在底层真实性：OS input plane、SendInput、humanized trajectory、CDP primitive 和 evidence 能继续细化。短板是模板市场、非技术用户流程编排、错误恢复 UI、调度观察面还不如两款商业工具。

### API

从产品能力看三者都能做本地 API，但本轮自动化只跑 PersonaPilot + BitBrowser：

- PersonaPilot：LaunchServer、本地 HTTP API、Wails/Tauri bridge、源码可控。
- AdsPower：Open Browser V2 启动 profile 后拿 debug interface，可接 Selenium/Puppeteer；当前 Free 账号 API & MCP 付费墙，所以本轮不自动化。
- BitBrowser：Local API 面很宽，打开浏览器返回 ws/http debug address、coreVersion、driver，还覆盖代理检测、RPA、cookies、文件读取。

如果是“自研 agent + 深度定制”，PersonaPilot 更好；如果是“快速接脚本跑规模”，AdsPower/BitBrowser 更省时间。

## 7. 代码/文档审计发现

### High

1. 非 ASCII 输入会被静默跳过
`backend/internal/wininput/keyboard_windows.go` 的 `TypeString` 遇到 `ch > 127` 直接 `continue`。中文目标站会出现“看似成功执行，实际没输入”的问题。建议支持 Unicode `SendInput`、剪贴板 fallback 或 IME-aware typing，并让跳过行为返回 warning。

2. DNS leak 评估过粗
`ProbeDNSConsistency` 把系统 DNS 解析结果和代理出口 IP 比较。很多正常 DNS 解析本来就不会等于出口 IP；这不是浏览器上下文 DNS 泄漏证明。建议用浏览器内探针、DoH/远端 resolver、WebRTC/STUN、代理侧 DNS 日志或专用 leak test 服务交叉验证。

3. 同轮 profile-browser / desktop 对照证据未通过
最新 comparison gate 是 `partial_comparison_only`，desktop/profile evidence 差 14329.64 分钟。当前不能把 desktop WebView probes 当 profile browser proof。建议把 full observed probe 与 desktop WebView probe 做成同一 run id 的原子 gate。

### Medium

4. 80/80 runtime projection 容易被误读
`hasImplicitRuntimeControl` 大量返回 true，适合内部 checklist，不适合对外宣称完整 runtime materialization。建议在 UI/报告里分成 `declared / arg-backed / injection-backed / observed / implicit`。

5. Camoufox 指纹深度与 Chromium 路径不等价
Camoufox 会过滤 `--fingerprint*`、`--disable-blink-features`、`--host-resolver-rules` 等 Chromium-only flags。建议文档显式标注 Camoufox 当前是启动/页面打开/基础 runner，不是 Chromium fingerprint args 等价执行器。

6. Geo/locale 映射太小
当前只覆盖 US/CA/GB/DE/FR/JP/CN/AU 等少量国家，且 US 默认 New York。建议接 IP metadata 后做国家/州/城市级 timezone、locale、Accept-Language、货币、日期格式一致性。

7. JS hook 仍有可检测面
plugins/mimeTypes 用普通冻结对象数组模拟，canvas `toDataURL` 会写回 canvas，WebRTC wrapper 未覆盖所有边界。建议引入检测站回归矩阵和原型链/属性描述符一致性测试。

### Low

8. README 口径与 docs live truth 有历史分歧
根 README 仍写整体终态 `40% / 60% / yellow`，docs 入口写本机自用 `100% / 0% / green`。这不是代码 bug，但横评时容易混淆。建议根 README 明确分成“本机自用完成度”和“商业竞品追平度”。

9. `pnpm typecheck` 被依赖安装策略挡住
直接 `tsc --noEmit` 通过，但 `pnpm typecheck` 会先触发 pnpm install 并因 `@swc/core` ignored builds 失败。建议固定项目验证命令或记录 `pnpm approve-builds` 前置状态。

## 8. 后续 benchmark 方案

要真正回答“谁的指纹/IP性能更强”，建议做一个合法授权的同条件黑盒 harness：

1. PersonaPilot 与 BitBrowser 各建 3 个 profile：US、JP、DE，各绑定同类型住宅代理；AdsPower 保留为 manual-only 观察项。
2. 每个 profile 连续启动 10 次，记录启动耗时、内存、进程数、debug API ready 时间。
3. 指纹检测：CreepJS、BrowserLeaks、BrowserScan、Pixelscan、WebGL/Canvas/Audio/Fonts/WebRTC/DNS。
4. IP 检测：ipinfo/ip-api/MaxMind 类元数据、ASN、hosting/proxy 标记、DNS 出口、STUN candidate。
5. 稳定性：同 profile 多次启动 fingerprint hash 漂移率；不同 profile 碰撞率。
6. 行为：同一页面执行点击、中文输入、滚动、拖拽、复制粘贴、弹窗/iframe/新 tab。
7. API：打开/关闭/profile 查询/代理检测/RPA 触发的错误率和平均耗时。
8. 产物：输出 raw JSON、截图、HAR、CDP trace、检测站报告 URL/截图；所有敏感 token/proxy/password 脱敏。

2026-07-08 代理预检后，矩阵执行拆成两个子矩阵：

- Clash 机场子矩阵：当前 US `3/3` 可用、JP `9/15` 可用、DE `0`；成功节点均为 `AS137409 GSL Networks Pty LTD`，ip-api `proxy=true`。该矩阵只评估机场节点下 PersonaPilot + BitBrowser 指纹/行为/API/兼容性，不进入“住宅 IP 质量”结论。
- UDEAL-LA 子矩阵：panda SSH reachable，现有本机桥 `18082` / `18090` 可用，出口为 LA，ip-api `proxy=false`、`hosting=false`，ASN 名称 `COGENT-174`。该矩阵只评估 UDEAL-LA 链式代理，不和 Clash 混分。
- DE 当前是 provider gap；没有 Germany 同类节点前，不给完整 US/JP/DE 同类代理横向总分。

## 9. 推荐路线

如果目标是“自己本机养号/研究/可控自动化”，继续投 PersonaPilot 是对的：它透明、可改、已经有强 evidence 框架，适合你这种高强度定制工作流。

如果目标是“今天就规模化管理多平台账号”，AdsPower 更稳：profile、团队、RPA、同步、Local API 和商业支持更完整。

如果目标是“低成本、多 profile、能接 API 和同步器”，BitBrowser 性价比强，本机 7.1.3 也确实可观测运行。

PersonaPilot 下一阶段最该补的不是继续堆“更多字段数字”，而是：

1. 同轮 profile-browser evidence gate。
2. Unicode/中文输入。
3. 真实浏览器上下文 DNS/WebRTC/TLS/IP benchmark。
4. 代理 provider credential-backed smoke。
5. 指纹 hook 原型链/属性描述符/ClientRects/WebGPU/UA-CH 深水区。
6. RPA 模板、错误恢复、非技术 operator 体验。

## 10. 2026-07-08 实测矩阵更新

本节是对上面“后续 benchmark 方案”的落地更新；仍然不是实测结论。

已完成：

- AdsPower 官方 8.6.3 Windows x64 安装器已下载到 `D:\SelfMadeTool\ads\AdsPower-Global-8.6.3-x64.exe`，SHA256 `99AC2ABEF961520919B8376448923DCE28A5BDBB543B77895BC3BE40AE90BD90`，Authenticode valid。
- AdsPower 已静默安装到 `D:\SelfMadeTool\ads\AdsPowerGlobal`，主程序为 `AdsPower Global.exe`，卸载表显示 `AdsPower Global 8.6.3`。
- BitBrowser 目录确认为 `D:\SelfMadeTool\bitbrowser`，主程序 `比特浏览器.exe`，版本 `7.1.3`，Local API `54345` 可访问；当前 `browser/list totalNum=0`。
- 已新增矩阵计划：`docs/48-three-browser-benchmark-matrix-plan.md`。
- 已生成脱敏代理预检：Clash 机场 US/JP 可用但 `proxy=true`，DE 缺；UDEAL-LA 经 panda/本机桥可用且为单出口 LA。汇总见 `data/reports/three-browser-benchmark/proxy-preflight/proxy-preflight-summary-1783442692772.json`。
- 2026-07-08 已新增 readiness harness：`scripts/three_browser_benchmark_readiness.mjs`。本轮 `--dry-run` 报告 `data/reports/three-browser-benchmark/readiness/readiness-1783473069728.json` 证明 PersonaPilot UDEAL-LA dry-run profile 创建、启动、CDP `/json/version`、关闭通过；BitBrowser UDEAL-LA dry-run profile 创建、`/browser/open`、CDP `/json/version`、`/browser/close` 通过。脚本已补 AdsPower v2 create/start/stop gated dry-run；最新 readiness-only 报告 `data/reports/three-browser-benchmark/readiness/readiness-1783475041481.json` 已把 AdsPower 阻塞改记为 Free plan API paywall。
- AdsPower Local API `127.0.0.1:50325/status` 可用，但 profile list 返回 `Require api-key`；用户截图确认当前 Free 账号 API & MCP 仅限付费套餐。未启用 paid/trial API access 并提供 `ADSPOWER_API_KEY` 前，不能创建/打开 AdsPower benchmark profile。
- 用户确认不为 AdsPower API 付费后，自动化实测范围收敛为 PersonaPilot + BitBrowser。`data/reports/three-browser-benchmark/readiness/readiness-1783475270066.json` 为当前双产品 UDEAL-LA smoke：PersonaPilot Chrome `139.0.7258.154`、BitBrowser Chrome `126.0.6478.271`，均打开 `https://browserleaks.com/ip`、CDP 可达、关闭成功；AdsPower 为 skipped。
- 2026-07-08 已新增基础 launch-loop harness：`scripts/two_browser_benchmark_matrix.mjs`。该脚本为 PersonaPilot + BitBrowser 创建/复用正式矩阵 profile，输出脱敏 `config.redacted.json`、`profiles.redacted.json`、`raw/*.json`、`summary.json`、`scorecard.md`，并使用浏览器页访问 `ipwho.is`，原始出口 IP 只保存 SHA256 hash。
- Clash 机场 US/JP 子矩阵已跑完：`data/reports/three-browser-benchmark/matrix/matrix-1783480553485/summary.json`，`40/40 ok`，PersonaPilot `20/20`、BitBrowser `20/20`，国家匹配 `40/40`；Clash controller 已恢复 `rule`，GLOBAL hash 已恢复。
- UDEAL-LA 子矩阵已跑正式候选：`data/reports/three-browser-benchmark/matrix/matrix-1783480223173/summary.json`，`19/20 ok`，PersonaPilot `10/10`、BitBrowser `9/10`。唯一失败为 BitBrowser Local API 返回“内存使用率超出 95%”；前一轮 `matrix-1783480022601` 在系统内存 `98%+` 时 BitBrowser `1/10`，已标记为 resource-contaminated run，不作为指纹/IP结论。

当前新增深度实测：

- 新增 `scripts/two_browser_benchmark_deep_matrix.mjs`，输出 `screenshots/`、`har-lite/`、`cdp-trace/`、`detector-reports/`、`transport/`、`behavior/`、raw、summary 和 scorecard。
- UDEAL smoke `data/reports/three-browser-benchmark/deep-matrix/deep-1783482616216/summary.json`：`2/2 ok`，PersonaPilot 和 BitBrowser 均完成 CDP/IP/TLS-H2/WebRTC/行为截图链路；外部 detector 按参数跳过。
- 当前可执行主跑 `data/reports/three-browser-benchmark/deep-matrix/deep-1783482661915/summary.json`：`6/6 ok`，覆盖 Clash US、Clash JP、UDEAL-LA 三个 cell 的双产品各 1 次；国家匹配 `6/6`，TLS/H2 observed `6/6`，行为 `5/6`。PersonaPilot 行为 `2/3`，唯一失败是 Clash US 的 CDP `Input.dispatchMouseEvent timeout`；ASCII input、中文 CDP insertText、textarea、scroll、iframe 均通过。BitBrowser 行为 `3/3`。
- Detector 主跑：PersonaPilot `18/18`；BitBrowser 主跑 `12/12`，其中 Clash JP 首轮 detectorSuite 有瞬时 `Page.navigate timeout`。随后用 `data/reports/three-browser-benchmark/deep-matrix/deep-1783490711021/summary.json` 精确补跑 BitBrowser Clash JP，结果 `1/1 ok`、detector `6/6`。
- Clash controller 复检为 `rule`，GLOBAL hash 已恢复；deep report 敏感扫描未命中明文代理凭证、Bearer/API key 或已知 UDEAL 原始出口 IP。

仍待下一阶段：

- AdsPower 本轮不进入自动化 launch 矩阵，保留安装、官方资料、免费 UI/手工观察。
- DE 等 Germany 同类节点补齐后再跑完整双产品 US/JP/DE 对等矩阵。
- 权威 DNS-token leak proof 需要受控域名；当前 WebRTC probe 已入 raw，但不伪造 DNS-token 结论。
- 根据 detector / transport / behavior raw artifact 重算 PersonaPilot catch-up P0/P1；当前不得把单次检测站页面可达性写成长期平台过检率。

评分纪律：

- 本报告第 5 节评分仍是“公开资料 + 本机代码审计 + 局部可观测事实”的工程评分。
- 在 `docs/48-three-browser-benchmark-matrix-plan.md` 的实测产物完成前，不把 AdsPower/BitBrowser 的闭源指纹/IP性能写成已验证结论。

## 11. A/B/C 详细横评

### 11.1 口径说明

这里的 A/B/C 按 `docs/48-three-browser-benchmark-matrix-plan.md` 的子矩阵定义：

- A：Clash 机场 US/JP 子矩阵。
- B：UDEAL-LA via panda/local bridge 子矩阵。
- C：DE 同类节点补齐后的完整对等矩阵；当前还不能执行。

AdsPower 是第三个产品维度，但本轮 Free 账号 API & MCP 付费墙未解除，所以只保留安装、官方资料、免费 UI/手工观察，不进入 A/B 自动化评分。

### 11.2 A：Clash 机场 US/JP 子矩阵

证据：

- 基础 launch-loop：`matrix-1783480553485`，`40/40 ok`。
- Deep matrix：`deep-1783482661915`；BitBrowser JP detector 以 `deep-1783490711021` 补齐。
- 低配额缺口 probe：PersonaPilot `missing-1783494756348`、BitBrowser `missing-1783495149879`；每产品 3 cell（Clash US、Clash JP、UDEAL-LA），用于补 UA/core、WebRTC、BrowserLeaks DNS 页面、CreepJS text/parser、canvas in-session 和 timezone 证据。
- 代理标记：成功节点均为 `AS137409 GSL Networks Pty LTD`，ip-api `proxy=true`、`hosting=false`。该子矩阵只能评估产品在机场节点下的启动、指纹页面、传输和行为兼容性，不能代表住宅 IP 质量。

| 维度 | PersonaPilot | BitBrowser | 判定 |
| --- | --- | --- | --- |
| 10 连启稳定性 | `20/20 ok` | `20/20 ok` | 平手 |
| 启动延迟 | P50 `1864ms`，P95 `1999ms` | P50 `2394ms`，P95 `4785ms` | PersonaPilot 明显更快、更稳 |
| 国家匹配 | `20/20` | `20/20` | 平手 |
| Detector 页面 | 主跑 US/JP `12/12` | US `6/6`；JP 首轮 timeout，补跑 `6/6` | 最终平手；BitBrowser JP 有瞬时可达性波动 |
| TLS/H2 | US/JP `2/2`，HTTP `h2` | US/JP `2/2`，HTTP `h2` | 平手；JA4 family 不同 |
| WebRTC | deep + missing probe 均 candidate `0` | deep + missing probe 均 candidate `0` | 平手；本地 IP 泄漏风险降低，但这不是 DNS-token proof |
| 行为/CDP parity | JP 过；US 一次 CDP click timeout，其余输入/滚动/iframe 过 | US/JP 全过 | BitBrowser 更稳 |
| UA/内核一致性 | CDP Browser `139`，页面 UA `Chrome/131`，missing probe `0/3` 匹配 | CDP Browser `126`，页面 UA `125/127`，missing probe `0/3` 匹配 | 双方都不合格；BitBrowser 旧 deep 的“更接近”不能继续当优势 |
| 指纹多样性 | WebGL/Canvas/ClientRects 有差异，webdriver false；canvas in-session `3/3` 稳定 | WebGL/Canvas 有差异，webdriver false；canvas in-session `3/3` 稳定 | 都有差异化和单 session 稳定；仍需多轮碰撞率/漂移验证 |

A 子矩阵结论：

- PersonaPilot 赢在启动性能：Clash US/JP 下 P95 大约 `2.0s`，BitBrowser 到 `4.8s`。
- BitBrowser 赢在行为稳定性：本轮 CDP 行为 `2/2`，PersonaPilot `1/2`。
- Detector 不能拉开差距：补跑后双方在当前页面可达性上都是 `12/12`。
- 双方最大共同扣分都是 UA/内核版本不一致：PersonaPilot 是 Browser `139` / UA `131`；BitBrowser 新建 profile 是 Browser `126` / UA `125` 或 `127`。PersonaPilot 因源码可控进入 P0 修复项；BitBrowser 只能作为黑盒风险记录。

### 11.3 B：UDEAL-LA 链式代理子矩阵

证据：

- 基础 launch-loop：`matrix-1783480223173`，`19/20 ok`。
- Deep smoke：`deep-1783482616216`，`2/2 ok`。
- Deep matrix：`deep-1783482661915` 的 `udeal:la` cell。
- 低配额缺口 probe：同上，UDEAL-LA cell 补到双方 country/timezone/TLS/WebRTC/canvas/CreepJS text/DNS page。
- 代理标记：LA 单出口，ip-api `proxy=false`、`hosting=false`；ASN/ISP 观测为 Cogent/GTT 相关。该子矩阵只评估 UDEAL-LA 链路，不和 Clash 混合评分。

| 维度 | PersonaPilot | BitBrowser | 判定 |
| --- | --- | --- | --- |
| 10 连启稳定性 | `10/10 ok` | `9/10 ok` | PersonaPilot 胜 |
| 失败原因 | 无 | 1 次 BitBrowser Local API “内存使用率超出 95%” | BitBrowser 资源保护是实测风险 |
| 启动延迟 | P50 `1933ms`，P95 `2829ms` | P50 `3552ms`，P95 `5688ms` | PersonaPilot 明显更快 |
| 国家匹配 | `10/10` | `9/9 observed` | PersonaPilot 胜在完整成功率 |
| Deep detector | `6/6` | `6/6` | 平手 |
| TLS/H2 | `1/1`，HTTP `h2` | `1/1`，HTTP `h2` | 平手 |
| WebRTC | deep + missing probe 均 candidate `0` | deep + missing probe 均 candidate `0` | 平手 |
| 行为/CDP parity | `1/1` | `1/1` | 平手 |
| UA/内核一致性 | CDP Browser `139`，页面 UA `Chrome/131` | CDP Browser `126`，missing probe UA `Chrome/127` | 双方都 mismatch；BitBrowser 旧 deep 的 UDEAL 一致性未在新建 profile 中复现 |

B 子矩阵结论：

- PersonaPilot 在 UDEAL-LA 下是当前更可靠的一方：启动 `10/10`，延迟也明显低。
- BitBrowser 只要能打开，detector/TLS/行为都能过；但 Local API 的内存保护阈值会让矩阵稳定性掉分。
- 代理本身是 LA 单出口，不能推导 JP/DE 或多地区质量。

### 11.4 C：DE / AdsPower / DNS-token 外部缺口

C 当前不是“产品跑输了”，而是没有满足执行条件：

| 项 | 当前状态 | 影响 |
| --- | --- | --- |
| DE 同类节点 | Clash 预检 DE `0`；UDEAL 只有 LA 单出口 | 不能声明完整 US/JP/DE 对等矩阵 |
| AdsPower API | 已安装，`127.0.0.1:50325/status` 可用；Free 账号 API & MCP 付费墙 | 不能进入 API 自动化 launch / detector matrix |
| 权威 DNS-token | 无受控域名和权威 DNS 日志 | 只能写 WebRTC candidate `0`，不能写 DNS-token leak proof |
| 长期过检率 | 只有当前检测站页面可达性和一次深度矩阵 | 不能外推到真实平台长期过检率 |

C 子矩阵结论：

- 现在不应该给 C 打分。
- 下一步如果要补 C，优先级是：Germany 同类代理节点 > 受控 DNS-token 域名 > AdsPower paid/trial API。

### 11.5 产品横评结论

| 维度 | 当前结论 |
| --- | --- |
| 启动/API 稳定性 | PersonaPilot 更好：基础矩阵总计 `30/30 ok`；BitBrowser `29/30 ok`，唯一失败为内存保护。 |
| 启动速度 | PersonaPilot 更好：Clash P95 `1999ms` vs BitBrowser `4785ms`；UDEAL P95 `2829ms` vs `5688ms`。 |
| Detector 页面可达性 | 当前可执行 cell 补跑后双方都可达：PersonaPilot `18/18`，BitBrowser `18/18`。 |
| 行为/CDP parity | BitBrowser 更稳：`3/3`；PersonaPilot `2/3`，一次 CDP click timeout。 |
| TLS/H2/WebRTC | 当前证据平手：双方 HTTP `h2`、transport observed `3/3`、missing probe TLS `3/3`、WebRTC candidate `0/3`。 |
| UA/内核一致性 | 双方都扣分：PersonaPilot Browser `139` / UA `131`；BitBrowser Browser `126` / UA `125` 或 `127`。 |
| 可审计与可修复性 | PersonaPilot 更好；脚本、raw artifact、源码和修复入口都在本仓库。 |
| 商业成熟度 | BitBrowser/AdsPower 仍强于 PersonaPilot；本轮只证明 PersonaPilot 在当前本机矩阵下已可跑通关键黑盒项。 |

当前 P0/P1：

1. P0：PersonaPilot UA/UA-CH/core version 对齐，避免 Browser `139` / UA `131`。
2. P0：PersonaPilot CDP click timeout 复现和稳定性修复；这不是 OS input plane 结论。
3. P1：Deep harness CDP target 选择优化，避免 BitBrowser 初始采样落在 console tab。
4. P1：把 detector raw 从“页面可达/文本 hash”升级到 per-signal 解析，不只统计页面打开成功。
5. P1：补 CreepJS trust/lies parser、受控 DNS-token proof 和 10-run drift；低配额 probe 已证明双方 UA/core mismatch 与 WebRTC candidate `0/3`，但未解决 DNS/trust/lies。

## 12. 100 分评分表

### 12.1 评分口径

本节只按本轮已经落盘的本机证据评分：

- 基础 launch-loop：`matrix-1783480553485`、`matrix-1783480223173`。
- Deep matrix：`deep-1783482661915`、`deep-1783490711021`。
- Low-quota missing probe：PersonaPilot `missing-1783494756348`、BitBrowser `missing-1783495149879`。
- 既有 PersonaPilot 内部 evidence：observed coverage、behavior replay、源码可审计性。

扣分原则：

- detector 页面打开成功只算“页面可达”，不等于 CreepJS trust 高、lies 少或长期平台过检。
- 没有 DNS-token、JA3/UA 基线、10-run 跨 session 重复采样、per-signal detector JSON 的项目，必须标缺数据。
- AdsPower 本轮因 Free 账号 API & MCP 付费墙没有自动化 raw artifact，不给实测分。
- BitBrowser 免费额度本轮只消耗 3 次打开做 targeted probe；不把它外推成完整漂移矩阵。

### 12.2 综合分

| 产品 | 实测综合分 | 证据级别 | 结论 |
| --- | ---: | --- | --- |
| PersonaPilot | `73 / 100` | launch-loop + deep matrix + missing probe + 源码/evidence | 启动速度、可审计性、代理工程强；WebRTC/canvas/timezone 补证后略加分；UA/内核一致性、OS Unicode、per-signal detector 解析和 click 稳定性扣分 |
| BitBrowser | `71 / 100` | launch-loop + deep matrix + missing probe + 本地 API 黑盒 | 行为稳定；但新建 profile `3/3` UA/core mismatch，启动慢、UDEAL 有内存保护失败、闭源不可审计扣分 |
| AdsPower | `N/A` | 安装/官方资料/免费 UI 手工观察 | API 自动化缺失，不进入实测分 |

### 12.3 维度 1：指纹真实性/IP/行为/证明/API

| 子项 | PersonaPilot | BitBrowser | AdsPower | 依据与缺口 |
| --- | ---: | ---: | ---: | --- |
| 指纹真实性/深度 | `65` | `64` | N/A | 双方 detector 页面可达且 canvas in-session `3/3` 稳定；PersonaPilot Browser `139` / UA `131`；BitBrowser Browser `126` / UA `125/127`，missing probe `0/3` 匹配；双方都缺 trust/lies per-signal |
| IP/代理工程 | `82` | `70` | N/A | PersonaPilot 透明支持 Clash/UDEAL/sing-box/SSH 链路且 30/30 基础启动成功；BitBrowser 可用但 UDEAL `9/10`，1 次内存保护失败 |
| 行为/RPA 广度 | `66` | `82` | N/A | Deep 行为 PersonaPilot `2/3`，BitBrowser `3/3`；PersonaPilot 本轮中文只是 CDP insertText，不是 OS SendInput proof；商业 RPA 生态 BitBrowser 更成熟 |
| 检测对抗可证明性 | `73` | `65` | N/A | PersonaPilot 可审计、有 raw/HAR/trace/missing probe；BitBrowser 黑盒 raw 充分但不可审计；双方当前只是 detector 页面可达，不是 trust/lies 解析 |
| API/自动化 | `84` | `78` | N/A | PersonaPilot LaunchServer 可控、快；BitBrowser Local API 可用但启动慢且有内存保护失败；AdsPower Free API 缺失 |
| 本维度小结 | `74` | `70` | N/A | PersonaPilot API/代理/可审计性占优；BitBrowser 行为仍强，但 UA/core mismatch 抹掉原一致性优势 |

### 12.4 维度 2：协议/供应链/验证深度/稳定性/泄漏防护

| 子项 | PersonaPilot | BitBrowser | AdsPower | 依据与缺口 |
| --- | ---: | ---: | ---: | --- |
| 协议广度/供应链 | `78` | `70` | N/A | PersonaPilot 链路覆盖 Clash、UDEAL、本机桥、SSH/sing-box 文档与源码；BitBrowser 本轮只测 HTTP 本机代理入口 |
| 验证深度（多层探针） | `78` | `72` | N/A | PersonaPilot 有源码/evidence board + deep matrix + missing probe；BitBrowser 有黑盒 CDP/raw + missing probe；双方缺 per-signal detector JSON 和长期重复 |
| 稳定性/重连 | `86` | `78` | N/A | PersonaPilot launch-loop `30/30`；BitBrowser `29/30`，UDEAL 有内存保护失败；重连策略未专项测 |
| 泄漏防护证据 | `72` | `72` | N/A | missing probe 双方 WebRTC candidate `0/3`、BrowserLeaks DNS 页面可达 `3/3`；无 DNS-token 和代理侧 DNS 日志；只能算部分证据 |
| 本维度小结 | `79` | `73` | N/A | PersonaPilot 胜在供应链透明和稳定性；双方 WebRTC 初证较好，DNS 证据仍不足 |

### 12.5 维度 3：内核 materialize/TLS/RPA/Chrome/CreepJS

| 子项 | PersonaPilot | BitBrowser | AdsPower | 依据与缺口 |
| --- | ---: | ---: | ---: | --- |
| 内核级指纹 materialize | `61` | `62` | N/A | PersonaPilot 有 schema/materialize 源码但更偏 JS/CDP hook，且 UA mismatch；BitBrowser 黑盒输出可用但新建 profile 也有 UA/core mismatch，且不可审 |
| TLS/JA3/H2 指纹 | `76` | `74` | N/A | 双方 TLS/H2 `3/3`，HTTP `h2`；PersonaPilot/BitBrowser JA4 family 不同；缺 JA3 与 Chrome/UA 基线一致性判定 |
| RPA 生态广度 | `60` | `78` | N/A | PersonaPilot 底层 primitive 强但模板/同步/RPA 生态弱；BitBrowser 官方生态更成熟，本轮只实测行为页 |
| Chrome 内核版本跟进 | `58` | `52` | N/A | PersonaPilot Browser `139` 但 UA `131`；BitBrowser Browser `126` 且页面 UA `125/127`，版本旧且不稳定自洽 |
| CreepJS 探针 | `55` | `55` | N/A | CreepJS 页面当前可达；missing probe parser `0/3` 提取 trust/lies，所以只能给页面可达分 |
| 本维度小结 | `62` | `64` | N/A | BitBrowser 行为生态仍更成熟；双方内核/UA 自洽和 CreepJS 结构化解析都不足 |

### 12.6 维度 4：静态指纹与泄漏细项

| 子项 | PersonaPilot | BitBrowser | AdsPower | 当前数据 | 缺哪些数据 |
| --- | ---: | ---: | ---: | --- | --- |
| CreepJS trust score | N/A | N/A | N/A | 页面可达；text hash/长度已留 | trust score、lies 数、per-signal JSON |
| lies 数 | N/A | N/A | N/A | 未解析 | CreepJS/BrowserScan structured lies |
| WebRTC 本地 IP 泄漏 | `86` | `86` | N/A | deep raw + missing probe 均未暴露 candidate；missing probe `0/3` | BrowserLeaks WebRTC 页面结构化字段、更多网络状态 |
| DNS 泄漏 | N/A | N/A | N/A | 未做受控 DNS-token | 受控域名、权威 DNS 日志、代理侧 DNS 日志 |
| Canvas hash 稳定性 | `60` | `60` | N/A | missing probe in-session `3/3` 稳定 | 同 profile 多 session 漂移率、跨 profile 碰撞率 |
| WebGL hash/renderer 稳定性 | `55` | `55` | N/A | WebGL renderer/hash 有记录 | 同 profile 多 session、worker/iframe parity |
| 跨 session 稳定 | `38` | `38` | N/A | missing probe 有一次 against previous deep 的对照，但不是 10-run gate | 同 profile 10 次 fingerprint hash drift |
| TLS JA3 与 UA 一致性 | `45` | `46` | N/A | JA3/JA4/H2 有记录；双方 UA/core mismatch；BitBrowser 不再按一致性加分 | Chrome major baseline、JA3/UA mapping |
| 代理一致性/geo 三角 | `84` | `84` | N/A | country/timezone `3/3`、国家匹配全过；BrowserLeaks DNS 页面可达 | 城市级 locale、权威 DNS 出口、长期 sticky |
| 本维度小结 | `61` | `61` | N/A | WebRTC/canvas/geo 初证更强 | 最大缺口仍是 trust/lies、DNS-token、10-run 跨 session |

### 12.7 维度 5：参数广度/运行时深度/真实性/地理/TLS/双引擎

| 子项 | PersonaPilot | BitBrowser | AdsPower | 依据与缺口 |
| --- | ---: | ---: | ---: | --- |
| 参数广度（schema） | `85` | `72` | N/A | PersonaPilot 有 80 controls、450 taxonomy、源码可查；BitBrowser 黑盒只能看输出/API 文档 |
| 运行时深度（materialize） | `72` | `70` | N/A | PersonaPilot final probe 有 UA/WebGL/canvas/locale 且 in-session canvas 稳定；BitBrowser 输出可用但 UA/core mismatch 且不可审 |
| 内核级真实性 | `55` | `64` | N/A | PersonaPilot 仍偏注入/hook，UA mismatch；BitBrowser闭源成品感更强但新建 profile UA/core mismatch，无法审计 |
| 一致性/地理对齐 | `84` | `84` | N/A | US/JP/LA 国家、timezone `3/3` 对齐；语言细节仍需城市级/站点级验证 |
| TLS/传输指纹 | `76` | `74` | N/A | 双方 H2/JA3/JA4 已观察；缺 Chrome baseline 和长期稳定性 |
| 双引擎成熟度 | `45` | `55` | N/A | 本轮只测 Chromium；PersonaPilot Camoufox/Firefox 路径不等价；BitBrowser官方有双核但本轮未实测 Firefox |
| 本维度小结 | `70` | `68` | N/A | PersonaPilot schema 强且可修；BitBrowser黑盒运行输出成熟但本轮一致性被 UA/core mismatch 拉低 |

### 12.8 缺失数据清单

低配额已补：

| 已补项 | PersonaPilot | BitBrowser | 仍不能外推 |
| --- | --- | --- | --- |
| country/timezone | `3/3` | `3/3` | 只有 US/JP/LA，非 DE/多城市 |
| WebRTC candidate | `0/3` | `0/3` | 还缺 BrowserLeaks WebRTC 结构化字段和更多网络状态 |
| TLS observed | `3/3` | `3/3` | 还缺 JA3/JA4 与 Chrome major baseline |
| UA/core major match | `0/3` | `0/3` | 双方都 mismatch；BitBrowser 不再按一致性优势打分 |
| Canvas in-session stability | `3/3` | `3/3` | 不是跨 session / 跨 profile drift |
| BrowserLeaks DNS 页面 | `3/3` observed | `3/3` observed | 不是权威 DNS-token proof |
| CreepJS trust/lies parser | `0/3` | `0/3` | 页面文本/hash 有，trust/lies 未结构化 |

| 缺失项 | 阻塞原因 | 补齐后能回答什么 |
| --- | --- | --- |
| AdsPower 自动化 raw | Free 账号 API & MCP 付费墙 | AdsPower 与两者同矩阵启动、detector、TLS、行为对比 |
| DE 同类节点 | 当前 Clash DE `0`，UDEAL 只有 LA | 完整 US/JP/DE 对等矩阵 |
| CreepJS trust/lies | missing probe parser `0/3`，当前只保存页面文本/hash/可达性 | 静态指纹真实评分、lies 数、headless 细项 |
| BrowserLeaks/BrowserScan/Pixelscan 结构化字段 | 当前未做 per-signal parser | WebRTC/DNS/Canvas/WebGL/automation 细项量化 |
| DNS-token proof | 只观察到 BrowserLeaks DNS 页面，缺受控域名和权威 DNS 日志 | 浏览器 DNS 出口是否与代理一致 |
| 10-run 跨 session hash drift | 当前 deep 每 cell 1 次，missing probe 只做 targeted 3 cell，不采 10-run fingerprint hash | Canvas/WebGL/audio/font/fingerprint 稳定率 |
| TLS/JA3 与 Chrome baseline | 当前只采 JA3/JA4/H2，不做版本基线匹配 | 判断 TLS 指纹是否与 UA/core 自洽 |
| OS Unicode input proof | 本轮中文输入是 CDP insertText | PersonaPilot Windows `SendInput` 中文真实能力 |
| Firefox/Camoufox/双引擎实测 | 本轮只测 Chromium | 双引擎成熟度真实评分 |

### 12.9 当前优先级

1. PersonaPilot P0：修 UA/UA-CH/core version 对齐。
2. PersonaPilot P0：复现并修 CDP click timeout；另起 OS Unicode input smoke，不把 CDP insertText 当 OS 输入。
3. Benchmark P1：加 CreepJS/BrowserLeaks/BrowserScan/Pixelscan per-signal parser。
4. Benchmark P1：等 BitBrowser 免费额度刷新后，同 profile 10 次 deep probe，计算 fingerprint hash drift / canvas drift / WebGL drift。
5. Benchmark P1：补受控 DNS-token 域名和 DE 同类节点。

## 13. Sources

Official / primary:

- AdsPower Browser Fingerprint help: https://help.adspower.com/docs/browser_fingerprint
- AdsPower Open Browser V2 Local API: https://localapi-doc-en.adspower.com/docs/Open-Browser-V2
- AdsPower pricing/features: https://www.adspower.com/pricing
- AdsPower RPA: https://help.adspower.com/docs/rpa
- AdsPower Synchronizer: https://help.adspower.com/docs/synchronizer
- AdsPower Create Profile: https://help.adspower.com/docs/creating_browser_profiles
- BitBrowser official docs home: https://doc.bitbrowser.net/
- BitBrowser Local Service Guide: https://doc.bitbrowser.net/api-docs/local-service-guide-demo-download
- BitBrowser Browser Profiles API: https://doc.bitbrowser.net/api-docs/browser-profiles
- BitBrowser Synchronize System: https://doc.bitbrowser.net/tongbu/help
- BitBrowser Upgrade Log: https://doc.bitbrowser.net/updatelog
- BitBrowser price/features: https://www.bitbrowser.net/price

Local evidence:

- `D:\SelfMadeTool\personal-pilot\backend\internal\behavior\environment_injector.go`
- `D:\SelfMadeTool\personal-pilot\backend\internal\browser\runtime_projection.go`
- `D:\SelfMadeTool\personal-pilot\backend\internal\browser\runtime_materialize.go`
- `D:\SelfMadeTool\personal-pilot\backend\internal\browser\proxy_launch.go`
- `D:\SelfMadeTool\personal-pilot\backend\internal\proxy\leak_probe.go`
- `D:\SelfMadeTool\personal-pilot\backend\internal\behavior\inputplane\`
- `D:\SelfMadeTool\personal-pilot\data\reports\observed-fingerprint-coverage\observed-fingerprint-coverage-gate-1782793511983.json`
- `D:\SelfMadeTool\personal-pilot\data\reports\live-replay-runtime\live-replay-runtime-gate-1782793512806.json`
- `D:\SelfMadeTool\personal-pilot\data\reports\profile-browser-comparison\profile-browser-comparison-1782793600653.json`
