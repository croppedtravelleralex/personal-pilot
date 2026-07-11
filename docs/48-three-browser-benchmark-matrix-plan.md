# PersonaPilot / BitBrowser 自动化实测矩阵计划

Updated: 2026-07-08 (Asia/Shanghai)

## 0. 状态边界

本文件记录 PersonaPilot + BitBrowser 当前已执行的黑盒实测矩阵、仍未满足的外部条件和后续分析入口。

- 已完成准备、基础 launch-loop 和当前可执行深度实测：AdsPower 安装包下载、签名校验、静默安装；BitBrowser 本机目录与本地 API 确认；PersonaPilot 仓库和既有 evidence 复核；Clash 机场与 panda/UDEAL 本机链路已做脱敏预检；readiness harness、基础 launch-loop harness 和 deep matrix harness 均已新增并跑过。用户确认不为 AdsPower API 付费后，当前自动化实测范围收敛为 PersonaPilot + BitBrowser；AdsPower 只保留安装、官方资料和免费 UI/手工观察。
- 未完成/不可执行：DE 同类节点缺失；AdsPower API 自动化为 paid/trial blocker；权威 DNS-token proof 需要受控域名；BitBrowser 免费额度今天只适合 targeted probe，不适合 10-run 漂移矩阵；最终商业评分只能基于当前 raw artifact 做谨慎更新，不能外推为长期过检率。
- 不改变主线口径：PersonaPilot 本机自用仍按 `100% / 0% / green` 维护；本横评是用户明确重开的独立 benchmark track。
- 安全边界：只测自有/授权账号、授权代理和允许自动化访问的检测页；不得记录明文 proxy password、token、cookie、账号密码或短信/邮箱验证码。

## 1. 本机准备事实

### PersonaPilot

- 仓库：`D:\SelfMadeTool\personal-pilot`
- 当前用户入口：`D:\SelfMadeTool\personal-pilot\personal-pilot-tauri.exe`
- 已有证据：
  - observed fingerprint：`data/reports/observed-fingerprint-coverage/observed-fingerprint-coverage-gate-1782793511983.json`，`passed_full_observed_fingerprint_coverage`，`470 / 450` observed signals，`12 / 12` families。
  - behavior replay：`data/reports/live-replay-runtime/live-replay-runtime-gate-1782793512806.json`，`passed_full_local_replay_runtime`，`461 / 450`，`contractOnly=0`。
  - desktop/profile 对照：`data/reports/profile-browser-comparison/profile-browser-comparison-1782793600653.json`，`partial_comparison_only`，desktop/profile evidence 相差 `14329.64` 分钟，不能当同轮 profile-browser proof。

### AdsPower

- 官方下载页：`https://www.adspower.com/download`
- 当前页面枚举的 Windows x64 安装器：`https://version.adspower.net/software/win64-global/8.6.3/AdsPower-Global-8.6.3-x64.exe`
- HEAD 校验：HTTP 200，Content-Length `338505080`，Last-Modified `Mon, 08 Jun 2026 06:22:54 GMT`。
- 下载路径：`D:\SelfMadeTool\ads\AdsPower-Global-8.6.3-x64.exe`
- 安装包 SHA256：`99AC2ABEF961520919B8376448923DCE28A5BDBB543B77895BC3BE40AE90BD90`
- Authenticode：Valid，签名主体 `SUNFLOWER TECH PTE. LTD.`。
- 安装路径：`D:\SelfMadeTool\ads\AdsPowerGlobal`
- 主程序：`D:\SelfMadeTool\ads\AdsPowerGlobal\AdsPower Global.exe`
- 卸载表：`AdsPower Global 8.6.3`
- 2026-07-08 启动后确认 `127.0.0.1:50325/status` 返回 `code=0 / msg=success`；`local_api` 文件路径为 `C:\Users\Lenovo\AppData\Roaming\adspower_global\cwd_global\source\local_api`。
- 当前状态：`/api/v1/user/list` 返回 `Require api-key`；用户截图确认当前 Free 账号 API & MCP 仅限付费套餐。AdsPower 本轮不进入自动化 launch 矩阵；只有未来启用 paid/trial API access 并设置 `ADSPOWER_API_KEY` 后，harness 才执行 v2 `browser-profile/create`、`start`、CDP `/json/version`、`stop` dry-run。`local.adspower.com` 在当前 Clash 环境会解析到 `198.18.0.118`，本机脚本应直连 `127.0.0.1:50325`。

### BitBrowser

- 安装路径：`D:\SelfMadeTool\bitbrowser`
- 主程序：`D:\SelfMadeTool\bitbrowser\比特浏览器.exe`
- 版本：`7.1.3` / `7.1.3.0`
- 进程：本机已有多个 `比特浏览器.exe` 进程。
- Local API：监听 `54345`；`POST http://127.0.0.1:54345/browser/list` 可返回 `success=true`。
- 2026-07-08 dry-run：`scripts/three_browser_benchmark_readiness.mjs --dry-run` 已创建 UDEAL-LA dry-run profile `benchmark-dryrun-bitbrowser-udeal-la-1783473069728`，`/browser/open` 返回 `httpDebug=127.0.0.1:52029`、`coreVersion=126`，CDP `/json/version` 通过，随后 `/browser/close` 成功。

### Proxy preflight

脱敏报告：

- Clash 机场：`data/reports/three-browser-benchmark/proxy-preflight/clash-airport-preflight-1783439936062.json`
- UDEAL-LA 现有本机桥：`data/reports/three-browser-benchmark/proxy-preflight/udeal-la-existing-local-bridge-preflight-1783442306848.json`
- 汇总：`data/reports/three-browser-benchmark/proxy-preflight/proxy-preflight-summary-1783442692772.json`
- Readiness / dry-run：`data/reports/three-browser-benchmark/readiness/readiness-1783473069728.json`
- Readiness-only paywall blocker recheck：`data/reports/three-browser-benchmark/readiness/readiness-1783475041481.json`
- Two-product UDEAL-LA smoke：`data/reports/three-browser-benchmark/readiness/readiness-1783475270066.json`
- Two-product default readiness：`data/reports/three-browser-benchmark/readiness/readiness-1783478822718.json`
- Two-product UDEAL-LA smoke with `ipwho.is` browser-page probe：`data/reports/three-browser-benchmark/matrix/matrix-1783479977514/summary.json`
- UDEAL-LA formal launch-loop candidate：`data/reports/three-browser-benchmark/matrix/matrix-1783480223173/summary.json`
- Clash US/JP formal launch-loop：`data/reports/three-browser-benchmark/matrix/matrix-1783480553485/summary.json`
- Deep matrix UDEAL smoke：`data/reports/three-browser-benchmark/deep-matrix/deep-1783482616216/summary.json`
- Deep matrix current executable full run：`data/reports/three-browser-benchmark/deep-matrix/deep-1783482661915/summary.json`
- Deep matrix BitBrowser Clash JP detector retry：`data/reports/three-browser-benchmark/deep-matrix/deep-1783490711021/summary.json`

当前结论：

| 子矩阵 | 可用地区 | 当前可用性 | 质量标记 | 评分口径 |
| --- | --- | --- | --- | --- |
| Clash 机场 | US / JP | 18 个候选；US `3/3` ok，JP `9/15` ok，DE `0` | 成功节点均为 `AS137409 GSL Networks Pty LTD`，ip-api `proxy=true`、`hosting=false` | 只能作为机场节点子矩阵，不和 UDEAL 混进同一个 IP 质量分 |
| UDEAL via panda | LA 单出口 | panda SSH reachable；本机 `18082` / `18090` 桥均可用；HTTP/SOCKS 本地消费方式均出 LA | `COGENT-174`，ip-api `proxy=false`、`hosting=false` | 单独跑 UDEAL-LA 链式代理子矩阵 |

DE 当前没有本机可用候选。完整 US/JP/DE 同类住宅代理矩阵仍需要补 Germany 节点或另一组同类型 provider。

基础 launch-loop 结果：

| 子矩阵 | 报告 | 结果 | 备注 |
| --- | --- | --- | --- |
| Clash US/JP | `matrix-1783480553485` | `40/40 ok`；PersonaPilot `20/20`，BitBrowser `20/20`；国家匹配 `40/40` | Clash mode 已恢复 `rule`，GLOBAL hash 已恢复；该结果只代表机场节点基础启动/CDP/IP 国家匹配 |
| UDEAL-LA | `matrix-1783480223173` | `19/20 ok`；PersonaPilot `10/10`，BitBrowser `9/10` | 唯一失败是 BitBrowser Local API 内存保护；`matrix-1783480022601` 是高内存污染 run，不作指纹/IP结论 |

深度矩阵结果：

| 子矩阵 | 报告 | 结果 | 备注 |
| --- | --- | --- | --- |
| UDEAL-LA smoke | `deep-1783482616216` | `2/2 ok`；国家匹配 `2/2`；TLS/H2 `2/2`；行为 `2/2` | 外部 detector 按 `--no-external-detectors` 跳过，用于验证 CDP/IP/TLS/WebRTC/行为截图链路 |
| Clash US/JP + UDEAL-LA | `deep-1783482661915` | `6/6 ok`；国家匹配 `6/6`；TLS/H2 `6/6`；行为 `5/6`；detector 主跑 PersonaPilot `18/18`、BitBrowser `12/12` | 生成 raw、screenshots、HAR-lite、CDP trace、detector reports、transport、behavior；PersonaPilot Clash US 一次 CDP click timeout |
| BitBrowser Clash JP retry | `deep-1783490711021` | `1/1 ok`；国家匹配 `1/1`；TLS/H2 `1/1`；行为 `1/1`；detector `6/6` | 补齐主跑中 BitBrowser Clash JP detector 瞬时 `Page.navigate timeout` 缺口 |

低配额缺口 probe 结果：

| 产品 | 报告 | 结果 | 新增事实 | 仍缺 |
| --- | --- | --- | --- | --- |
| PersonaPilot | `missing-1783494756348` | `3/3 ok`；country `3/3`；timezone `3/3`；TLS `3/3`；WebRTC candidate `0/3`；canvas in-session `3/3`；BrowserLeaks DNS observed `3/3` | Browser `139` / UA `131`，UA/core major match `0/3`；CreepJS trust/lies parser `0/3` | DNS-token、CreepJS structured trust/lies、10-run drift |
| BitBrowser | `missing-1783495149879` | `3/3 ok`；country `3/3`；timezone `3/3`；TLS `3/3`；WebRTC candidate `0/3`；canvas in-session `3/3`；BrowserLeaks DNS observed `3/3` | Browser `126` / UA `125` 或 `127`，UA/core major match `0/3`；CreepJS trust/lies parser `0/3` | DNS-token、CreepJS structured trust/lies、10-run drift |

A/B/C 横评摘要见 `docs/47-personal-pilot-adspower-bitbrowser-benchmark.md` §11；100 分细项评分见 §12。当前矩阵级结论：

| 口径 | 状态 | PersonaPilot | BitBrowser | 结论 |
| --- | --- | --- | --- | --- |
| A. Clash US/JP | 已评估 | launch-loop `20/20`；deep detector `12/12`；行为 `1/2`；P95 `1999ms` | launch-loop `20/20`；deep detector 补跑后 `12/12`；行为 `2/2`；P95 `4785ms` | PersonaPilot 启动性能胜；BitBrowser 行为稳定性胜；IP 质量只代表机场节点 |
| B. UDEAL-LA | 已评估 | launch-loop `10/10`；deep detector `6/6`；行为 `1/1`；P95 `2829ms` | launch-loop `9/10`；deep detector `6/6`；行为 `1/1`；P95 `5688ms` | PersonaPilot 稳定性和速度胜；BitBrowser 有内存保护失败 |
| C. DE / 完整对等矩阵 | 未评估 | 无 DE 同类节点 | 无 DE 同类节点 | 不能打完整 US/JP/DE 总分 |

补测后评分口径：PersonaPilot `73/100`，BitBrowser `71/100`，AdsPower `N/A`。主要变化是双方 WebRTC/canvas/timezone 初证增强，但 BitBrowser 新建 profile `3/3` UA/core mismatch，不能继续按“UA/内核更一致”加分。

## 2. 测试目标

本矩阵要回答四个问题：

1. PersonaPilot 与 BitBrowser 在同一代理、同一地区、同一检测集下，真实暴露出来的 fingerprint / IP / WebRTC / DNS / TLS / 行为信号差异是什么；AdsPower 只保留安装、官方资料和免费 UI/手工观察。
2. PersonaPilot 当前 `declared / configured / launched / observed / verified` 五层证据中，哪些只是 schema 或注入能力，哪些已经能和商业闭源产品黑盒结果对齐。
3. AdsPower、BitBrowser 的闭源优势到底体现在 profile 生命周期、RPA、同步器、内核更新、指纹深度、IP 质量治理还是运行稳定性上。
4. PersonaPilot 下一步补短板时，哪些属于高 ROI 工程改造，哪些需要浏览器内核/传输层/代理供应链长期投入。

## 3. 产品与 profile 矩阵

### 3.1 基础矩阵

当前自动化对等矩阵按双产品、三地区、每 profile 10 连启设计；AdsPower 保留为 API-paywalled/manual-only 观察项：

| 产品 | Profile 数 | 地区 | 代理要求 | 启动次数 | 目标产物 |
| --- | ---: | --- | --- | ---: | --- |
| PersonaPilot | 3 | US / JP / DE | 同类型住宅代理，sticky >= 30 min | 10 / profile | raw JSON、截图、HAR、CDP trace、检测站报告 |
| AdsPower | 0 自动化 | API-paywalled | Free 账号 API & MCP 付费墙 | 0 | 安装、官方资料、免费 UI/手工观察 |
| BitBrowser | 3 | US / JP / DE | 同类型住宅代理，sticky >= 30 min | 10 / profile | raw JSON、截图、HAR、debug endpoint、检测站报告 |

总量：`2 products * 3 regions * 10 launches = 60 launches`。如每个地区再加 1 个重复 profile 做碰撞率验证，则升级为 `120 launches`。

但按 2026-07-08 本机代理现实，实测必须拆成两个子矩阵：

| 子矩阵 | 产品 | 地区/Profile | 启动次数 | 用途 |
| --- | --- | --- | ---: | --- |
| A. Clash 机场 | PersonaPilot / BitBrowser | US / JP | `2 products * 2 regions * 10 = 40` | 启动、检测站、行为/API 对齐；IP 分数只代表机场节点 |
| B. UDEAL-LA 链式代理 | PersonaPilot / BitBrowser | LA 单出口 | `2 products * 1 region * 10 = 20` | 评估 panda/本机桥/UDEAL 链路质量与双产品兼容性 |
| C. DE 补齐后完整矩阵 | PersonaPilot / BitBrowser | DE | `2 products * 1 region * 10 = 20` | 只有拿到 Germany 同类节点后再补跑 |

### 3.2 代理约束

- 完整对等矩阵要求 US / JP / DE 使用同一供应商、同一代理类型、同一计费/轮换策略。
- 每个 profile 固定一个 sticky session；一次 10 连启期间不主动换 IP。
- 两套自动化产品尽量使用同一批地区池，但不要同时复用同一出口 IP，避免检测站把并发访问聚合成同一行为。
- 每次启动记录代理原始 URL 的脱敏 hash、出口 IP hash、ASN、国家、城市、timezone、hosting/proxy 标记；常规报告不保存原始出口 IP。
- Clash 机场、UDEAL-LA 链式代理不是同一类代理，不得混进同一个“IP 质量横评”分。它们只能分别形成 proxy submatrix，然后在总报告里标注 proxy class。

### 3.2.1 当前代理候选

2026-07-08 已完成本机脱敏预检：

- Clash/Mihomo：`verge-mihomo` 监听 mixed-port `7897`、controller `127.0.0.1:9097`，controller secret 存在但不写入报告；测试后模式恢复为 `rule`。
- Clash 节点库存：总候选 `18`，US `3`、JP `15`、DE `0`；成功 `12`、失败 `6`。US `3/3` 可用，JP `9/15` 可用。
- Clash 成功节点：均为 `AS137409 GSL Networks Pty LTD`，ip-api `proxy=true`、`hosting=false`，因此不适合作为“高质量住宅代理”结论，只能作为机场节点可达性和产品兼容性子矩阵。
- panda SSH：`BatchMode` 探测可用。
- UDEAL-LA：现有本机桥 `127.0.0.1:18082`、`127.0.0.1:18090` 均可用；HTTP、SOCKS 本地消费方式均返回 LA；ip-api `proxy=false`、`hosting=false`；ASN 名称为 `COGENT-174`。
- UDEAL-LA 只有单一 LA 出口，不提供 US/JP/DE 三地区对等矩阵。
- 临时旧端口 `56947`、`60476`、`55224` 当前不监听；不纳入下一阶段。

执行规则：

- A 子矩阵只用 Clash 机场 US/JP，记录 `proxyClass=clash_airport`；当前自动化产品为 PersonaPilot + BitBrowser。
- B 子矩阵只用 UDEAL-LA，记录 `proxyClass=udeal_la_via_panda`；当前自动化产品为 PersonaPilot + BitBrowser。
- DE 为 provider gap，只有新增 Germany 节点后才补跑。
- 代理原始 URL、controller secret、subscription URL、username、password、token 不得进入 raw JSON、HAR、截图、日志或 docs；出口 IP 在 proxy preflight 中只保留 hash。

### 3.3 浏览器内核约束

- 优先统一 Chromium 家族测试；Firefox/Camoufox 作为第二矩阵，不混入第一轮总分。
- 记录每个产品真实浏览器内核版本、UA/UA-CH major、Chromedriver/CDP protocol 版本。
- Chrome major 版本差距超过 2 个大版本时，评分中单独扣“版本跟进/生态一致性”而不是直接混进指纹 hash。

## 4. 检测维度

### 4.1 启动与资源

每次启动记录：

- cold/warm 标记
- open API latency
- browser process spawn latency
- debug API ready latency
- 首个检测页 DOMContentLoaded / load 时间
- 进程数、PID tree、RSS / working set / private bytes
- 启动失败、debug endpoint 缺失、profile lock、代理连接失败、页面 timeout

### 4.2 指纹检测

检测站与探针：

- CreepJS：trust score、lied signals、headless、iframe、worker、audio、canvas、webgl、fonts、math、timezone、permissions。
- BrowserLeaks：Canvas、WebGL、Audio、Fonts、WebRTC、DNS、Client Hints、JavaScript、TLS/SSL 可见项。
- BrowserScan：bot/fingerprint consistency、IP/位置、WebRTC、DNS、timezone、language。
- Pixelscan：consistency score、proxy/fingerprint mismatch、automation/headless signals。
- 自建探针：Navigator descriptor、PluginArray/MimeTypeArray prototype、ClientRects、WebGPU、UA-CH high entropy、permissions、media devices、codec、Intl、screen、storage、worker/iframe parity。

每个信号用五层标记：

| 层级 | 含义 | 例子 |
| --- | --- | --- |
| declared | schema 有字段 | `webgl_renderer` 在配置模型中存在 |
| configured | profile 已配置 | profile 写入 renderer 参数 |
| launched | 启动参数/配置已送到浏览器 | `--fingerprint-webgl-renderer=...` |
| observed | 浏览器里采集到值 | CDP / JS 探针返回 renderer |
| verified | 第三方检测或同轮对照通过 | CreepJS / BrowserLeaks 未报异常，且 profile/browser 同轮一致 |

### 4.3 IP / 代理

采集项：

- ipinfo、ip-api、MaxMind 类 GeoIP 元数据
- ASN、ISP、组织、rDNS、hosting/proxy/VPN/datacenter 标记
- 代理连接协议：HTTP / HTTPS / SOCKS5 / SSH bridge / 本机 sing-box / xray
- DNS 出口：浏览器上下文 DNS leak 页面、自建 DNS token、系统 resolver 对照
- STUN candidate：host / srflx / relay，是否暴露本机 LAN、公网真实 IP 或非代理出口
- 地理一致性：IP country/city/timezone、Accept-Language、navigator.language、Intl timezone、货币/日期/数字格式

### 4.4 TLS / JA3 / H2 / Header

采集项：

- JA3 / JA4 或同类 TLS ClientHello 指纹
- ALPN 协商结果
- HTTP/2 SETTINGS、pseudo-header 顺序、priority/window update 行为
- TLS version、cipher suites、extension order、ECH/Grease 可见性
- request header 顺序、Accept-Language 与 UA-CH 一致性

原则：

- 尽量使用自建或明确允许测试的 echo endpoint。
- 区分“浏览器真实网络栈”与“后端 Go HTTP client/代理健康检测网络栈”。
- 如果代理服务端或本机 bridge 改写 TLS/H2，要单独标记为 proxy-layer fingerprint，不把它误记为浏览器内核能力。

### 4.5 稳定性 / 碰撞率

同 profile 10 连启：

- fingerprint hash 漂移率
- canvas/audio/webgl/font hash 漂移率
- UA/UA-CH/timezone/locale/screen 是否意外变化
- IP 是否保持 sticky
- cookies/localStorage/sessionStorage 是否保持

不同 profile：

- 跨 profile fingerprint hash 碰撞率
- canvas/audio/webgl/font 碰撞率
- 同地区不同 profile 是否共享过多硬件/字体/插件特征
- profile 数据目录、cache、service worker、extension state 是否隔离

### 4.6 行为 / RPA / 同步

统一执行同一测试页面：

- 点击、双击、右键、hover、拖拽、滚动、分页停顿
- 中文输入、英文输入、emoji、删除、选中、复制粘贴
- iframe、弹窗、alert/confirm/prompt、文件上传、下载、新 tab
- 表单填写、DOM 稳定等待、元素不可见/遮挡/滚动到视口
- 多窗口同步：主控输入、窗口排列、延迟、失败恢复
- RPA 任务：创建、运行、日志、重试、定时、并发、错误分类

PersonaPilot 当前必须特别测中文输入，因为 `backend/internal/wininput/keyboard_windows.go` 的 `TypeString` 仍会跳过 `ch > 127`。

### 4.7 API / 自动化集成

统一测：

- profile list / create / update / delete
- open / close / status
- proxy check
- debug endpoint 获取
- cookies get/set/import/export
- RPA trigger / task status / logs
- browser window arrange / resize
- 错误率、平均耗时、P95、错误 taxonomy

API 结果不得记录完整 token、proxy password、cookie、Authorization header。

## 5. 评分模型

总分 100，实测后刷新 `docs/47-personal-pilot-adspower-bitbrowser-benchmark.md` 的主观评分。

| 类别 | 权重 | 核心问题 |
| --- | ---: | --- |
| 指纹信号覆盖 | 12 | 信号面是否广，worker/iframe/UA-CH/WebGPU/ClientRects 是否覆盖 |
| 指纹真实性/长期抗检测 | 15 | 是否像真实内核自然输出，而不是浅层 JS patch |
| 运行时 materialize 深度 | 10 | configured 到 launched/observed/verified 的转化率 |
| 内核版本跟进与双引擎成熟度 | 8 | Chrome/Firefox 版本、driver、CDP、Camoufox/Firefox parity |
| IP/代理启动链路 | 10 | 代理协议、认证、bridge、sticky、失败恢复 |
| IP 质量治理 | 8 | ASN/hosting/proxy 风险、供应商、轮换、SLA、池质量 |
| WebRTC/DNS/TLS 泄漏治理 | 10 | STUN、DNS token、JA3/H2/header 是否一致 |
| 行为/RPA/同步 | 10 | OS input、中文、RPA、同步器、模板生态 |
| Profile/session 生命周期 | 7 | 隔离、持久化、迁移、清理、profile lock |
| API/自动化集成 | 6 | 本地 API 面、debug endpoint、错误率、文档和 SDK |
| 可观测性/证据链 | 4 | raw JSON、截图、HAR、trace、报告可复跑 |

硬性降级规则：

- 无 profile 可测：相关产品最多给 planning score，不能给实测 score。
- 无同类代理：IP / DNS / TLS 维度不得横向排名。
- Clash 机场与 UDEAL-LA 必须拆分 proxy submatrix；不得把两者混成一个 IP 质量总分。
- 无 raw artifact：对应检测项最多算 observed，不能算 verified。
- 检测站只截图无 JSON：可作为人工证据，但不得进入 hash 漂移率计算。
- 出现明文凭证进入 artifact：该轮作废并重跑。

## 6. PersonaPilot 补全/追评方案

### 6.1 参数广度 schema

补齐 schema，不只是增加字段名，还要给每个字段标注 materialize path：

- Navigator：webdriver、platform、vendor、productSub、languages、plugins、mimeTypes、permissions、hardwareConcurrency、deviceMemory。
- UA-CH：brands、fullVersionList、platform、platformVersion、architecture、bitness、model、mobile。
- Graphics：WebGL vendor/renderer/extensions/limits、WebGPU adapter info、Canvas 2D、ClientRects、CSS media queries。
- Audio：AudioContext、OfflineAudioContext、sampleRate、oscillator/compressor hash。
- Fonts：字体枚举、文本 metrics、emoji/font fallback、CJK 字体族。
- Media：mediaDevices enumerate、codec support、image decode、video color space。
- OS/Locale：timezone、DST、locale、keyboard layout、IME/input method、date/number/currency/first-day-of-week。
- Network：proxy protocol、DNS mode、WebRTC policy、STUN policy、TLS/H2/header profile。

### 6.2 运行时深度 materialize

把每个字段落到明确执行路径：

- `schema only`：只声明，不能计分。
- `profile configured`：写入 profile，但未启动验证。
- `launch args`：Chrome flag / preference / extension config。
- `CDP injection`：`Page.addScriptToEvaluateOnNewDocument`，同时覆盖 top frame、iframe、worker 可行边界。
- `native/browser preference`：profile Preferences、Local State、policy、extension API。
- `observed proof`：CDP/JS/raw detector output。
- `third-party verified`：CreepJS/BrowserLeaks/BrowserScan/Pixelscan 同轮通过。

现有 `FullRuntimeProjectionReport` 应改成多桶输出：`declared / argBacked / injectionBacked / profileMetadataBacked / implicit / observed / verified`，避免 `80/80` 被误读成内核级全物化。

### 6.3 内核级真实性

短期不 fork Chromium，也要降低浅层 hook 痕迹：

- 给每个 hook 加 prototype chain、property descriptor、native-like `toString`、enumerability、configurability 测试。
- 插件和 mimeTypes 不再用普通冻结数组模拟，改成接近 `PluginArray` / `MimeTypeArray` 的行为模型。
- Canvas 噪声避免污染原 canvas 状态；区分 `getImageData`、`toDataURL`、`toBlob`、WebGL readPixels。
- ClientRects / DOMRect / bounding box 加一致性模型，和字体 metrics、DPR、zoom 绑定。
- iframe / popup / worker / service worker / extension world 做同轮探针，发现上下文穿透。
- CreepJS 探针输出拆成 per-signal remediation，不只存 trust 分。

中期方案：

- Chromium 路径：跟进主流 Chrome major，减少过旧 UA/内核 mismatch。
- Camoufox/Firefox 路径：建立独立 Firefox 指纹 schema，不复用 Chromium-only flag。
- 内核级能力只在真实浏览器输出和第三方 detector 同轮通过后升为 verified。

### 6.4 一致性/地理对齐

基于 IP metadata 生成 profile bundle：

- country/city/timezone/locale/Accept-Language/Intl/date/number/currency 一起生成。
- US 不再固定 New York；按 IP region/city 映射 IANA timezone。
- 代理 ASN、ISP、residential/datacenter、rDNS 与 profile 风险等级绑定。
- locale 与键盘布局/输入法分离，支持 JP/DE/US 真实输入习惯。
- 同 profile 10 连启应保持稳定，不同 profile 保持足够差异。

### 6.5 TLS/传输指纹

PersonaPilot 需要把浏览器网络栈和后端探测网络栈拆开：

- 浏览器真实访问走 browser-core 采样 TLS/H2/headers。
- Go health check 只作为代理可用性，不代表浏览器 TLS。
- sing-box/xray/SSH bridge 产生的代理层传输指纹单独记录。
- 对每个 Chrome major 建 baseline：JA3/JA4、ALPN、H2 SETTINGS、header order。
- 代理供应商如果终止 TLS 或改写 H2，需要标为 provider behavior。

### 6.6 IP/代理工程

补齐商业产品常见工程能力：

- provider catalog：住宅/ISP/机房/移动、国家、城市、sticky TTL、认证方式、并发限制。
- quality score：ASN 风险、hosting/proxy 标记、历史封禁、延迟、失败率、漂移率。
- lease/cooldown：同 profile 绑定 IP 租约，不在敏感操作中途换 IP。
- DNS proof：自建 DNS token + 浏览器 leak 页面 + 代理侧日志三方对照。
- WebRTC proof：STUN candidate raw log，识别 host/srflx/relay 与出口 IP hash 的一致性。
- secret redaction：proxy URL、API key、cookie、bearer 全部 hash/掩码化。

### 6.7 行为/RPA 广度

高优先级补齐：

- Windows Unicode `SendInput` 或剪贴板/IME fallback，修复中文输入静默跳过。
- 拖拽、复制粘贴、文件上传、下载、弹窗、iframe、新 tab 的统一 primitive。
- recorder -> template -> replay 的错误恢复、截图定位、DOM drift 处理。
- 同步器：主控窗口到多 profile 的延迟、失败隔离、窗口布局和日志。
- RPA 生态：可导入/导出模板、任务调度、运行日志、重试策略、参数化变量。

### 6.8 检测对抗可证明性

新增 gate：

- `three_browser_benchmark_prepare_gate`：检查 AdsPower/BitBrowser/PersonaPilot 路径、版本、API 可达性。
- `three_browser_profile_matrix_gate`：检查当前 Clash US/JP 与 UDEAL-LA 子矩阵 profiles、代理地区、sticky、secret redaction；DE 缺口单独标记。
- `three_browser_detector_gate`：跑 CreepJS/BrowserLeaks/BrowserScan/Pixelscan，并输出 raw artifact。
- `three_browser_transport_gate`：跑 TLS/JA3/H2/header echo。
- `three_browser_behavior_gate`：统一执行行为脚本，覆盖中文、iframe、tab、popup。
- `three_browser_analysis_gate`：生成 scoring、漂移率、碰撞率和补短板 backlog。

## 7. 当前短板与改进方向

| 维度 | PersonaPilot 当前短板 | 改进方向 |
| --- | --- | --- |
| 指纹信号覆盖 | 自有 observed 覆盖强，但第三方 detector 同轮 verified 不足 | 将 CreepJS/BrowserLeaks/Pixelscan 输出纳入 raw JSON gate |
| 指纹真实性 | JS/CDP hook 痕迹仍可能被原型链/descriptor/worker 探出 | 建 native-like descriptor 测试、iframe/worker parity、ClientRects/WebGPU 深水区 |
| 长期抗检测 | Chrome 131/139 等配置与实际核心版本需持续对齐 | 增加 core version watcher 和 UA/UA-CH 自动生成 |
| IP 启动链路 | sing-box/xray/SSH 透明，但真实 provider smoke 仍缺账号 | provider catalog + credential-backed smoke + lease/cooldown |
| IP 质量治理 | 缺商业代理库存、风控历史、ASN reputation DB | 建 IP quality ledger，按 ASN/ISP/地区/失败率评分 |
| WebRTC/DNS | 有 CDP STUN 探针和 host-resolver，但 DNS proof 仍偏启发式 | 自建 DNS token、浏览器上下文 leak、代理侧日志三方对照 |
| TLS/H2 | 本机 direct TLS observed 不能代表代理/浏览器全链路 | 控制 echo endpoint，按 browser core 记录 JA3/JA4/H2/header |
| 行为/RPA | 底层 primitive 强，产品化模板生态弱；中文输入缺口明确 | Unicode input、RPA 模板、同步器日志、错误恢复 UI |
| Profile/session | 本机 SessionBundle 强，商业级团队/批量生命周期弱 | profile clone、批量导入、权限/标签/审计、profile lock |
| API 集成 | 自研 API 可控，但缺 AdsPower/BitBrowser 兼容适配层 | 增加 benchmark adapters，不把三方 API 差异写死到 harness |
| 可观测性 | 自家报告强，闭源产品 artifact 需要外部采集 | 统一 HAR、截图、trace、debug endpoint、检测站 URL |

## 8. 下一阶段待办

### 准备完成项

- [x] 创建并使用 `D:\SelfMadeTool\ads`。
- [x] 下载 AdsPower Windows x64 官方安装器并记录 hash/签名。
- [x] 静默安装 AdsPower 到 `D:\SelfMadeTool\ads\AdsPowerGlobal`。
- [x] 确认 BitBrowser 目录 `D:\SelfMadeTool\bitbrowser`、版本 `7.1.3`、API 端口 `54345`。
- [x] 设计 PersonaPilot + BitBrowser 自动化矩阵，并保留 AdsPower manual-only 观察边界。
- [x] 预检 Clash 机场节点、panda SSH 与 UDEAL-LA 现有本机桥，并生成脱敏 summary。
- [x] 新增 `scripts/three_browser_benchmark_readiness.mjs`，并完成 PersonaPilot + BitBrowser UDEAL-LA 1 profile / 1 launch dry-run；报告脱敏扫描通过。脚本支持 `--products=personal-pilot,bitbrowser`，AdsPower 已被跳过。
- [x] 新增 `scripts/two_browser_benchmark_matrix.mjs`，完成基础 launch-loop、CDP page probe、`ipwho.is` 国家/IP hash probe 和脱敏报告输出。
- [x] 新增 `scripts/two_browser_benchmark_deep_matrix.mjs`，完成截图、HAR-lite、CDP trace、detector reports、TLS/H2、WebRTC 和行为探针输出。

### 实测前待办

- [x] AdsPower 已标记为 API-paywalled/manual-only，不进入本轮自动化 launch 矩阵。
- [x] 在 BitBrowser / PersonaPilot 各创建/复用 Clash 机场 US、JP profile，绑定 `proxyClass=clash_airport`。
- [x] 在 BitBrowser / PersonaPilot 各创建/复用 UDEAL-LA profile，绑定 `proxyClass=udeal_la_via_panda`。
- [ ] 补 Germany 同类节点；没有 DE 节点前不跑完整 US/JP/DE 同类代理总分。
- [ ] 准备代理清单：地区、ASN、sticky TTL、协议、账号存在性、脱敏 hash、供应商授权；不得写明文上游 URL。
- [x] 写基础 benchmark harness：双产品 adapter、启动/关闭、debug endpoint、CDP probe、artifact redaction。
- [x] 写检测页 allowlist 和访问频率限制，避免对第三方检测站过度请求。
- [x] 跑 1 profile / 1 region / 1 launch 的 dry-run，确认 artifact 完整。
- [x] 先跑当前可用的 `40 launch` Clash 机场 US/JP 子矩阵和 `20 launch` UDEAL-LA 子矩阵；DE 补齐后再补完整双产品对等矩阵。
- [x] 生成 `screenshots/`、`har-lite/`、`cdp-trace/`、detector raw、transport、behavior；基础 harness 和 deep harness 均已生成 `raw/`、`summary.json`、`scorecard.md`。
- [x] 回写 `docs/47-personal-pilot-adspower-bitbrowser-benchmark.md` 的当前实测结论；最终商业评分仍按 AdsPower/DE/DNS-token 外部条件保留边界。
- [ ] 将 PersonaPilot 高优先级短板拆成实施 issue/backlog。

## 9. 产物目录建议

```text
data/reports/three-browser-benchmark/
  2026-07-matrix-01/
    config.redacted.json
    products/
      personal-pilot/
      adspower/
      bitbrowser/
    raw/
    screenshots/
    har/
    cdp-trace/
    detector-reports/
    transport/
    behavior/
    summary.json
    scorecard.md
```

`config.redacted.json` 只允许出现：

- proxy hash
- provider alias
- region/country/city
- protocol
- sticky TTL
- credential presence boolean
- redaction proof

不得出现：

- proxy username/password
- provider API key
- AdsPower/BitBrowser login token
- cookies/session payload
- phone/email verification content

## 10. 官方/本机来源

- AdsPower download: `https://www.adspower.com/download`
- AdsPower official installer: `https://version.adspower.net/software/win64-global/8.6.3/AdsPower-Global-8.6.3-x64.exe`
- AdsPower Local API docs: `https://localapi-doc-en.adspower.com/`
- AdsPower RPA docs: `https://rpa-doc-en.adspower.com/`
- BitBrowser docs: `https://doc.bitbrowser.net/`
- BitBrowser Local API docs: `https://doc.bitbrowser.net/api-docs/local-service-guide-demo-download`
- BitBrowser browser profile API: `https://doc.bitbrowser.net/api-docs/browser-profiles`
- Existing comparison: `docs/47-personal-pilot-adspower-bitbrowser-benchmark.md`
- Local code evidence:
  - `backend/internal/browser/runtime_projection.go`
  - `backend/internal/browser/runtime_materialize.go`
  - `backend/internal/behavior/environment_injector.go`
  - `backend/internal/proxy/leak_probe.go`
  - `backend/internal/browser/proxy_launch.go`
  - `backend/internal/wininput/keyboard_windows.go`
  - `data/reports/three-browser-benchmark/proxy-preflight/proxy-preflight-summary-1783442692772.json`
