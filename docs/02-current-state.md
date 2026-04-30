# antbrowser 当前状态主档

## 最后更新时间

- 日期：2026-04-30
- 维护目的：记录行为录制功能评估结果、非容器化实例工作台 MVP、外部窗口辅助控制、身份强度体检、代理池免费代理抓取与后续技术完善入口

## 整体状态摘要

当前仓库原有 Wails/Go 后端 + Vite/React/TypeScript 前端历史形态，同时已补齐根目录 Tauri 2 + Vite/React/TypeScript 基线入口；最新 Win11 Tauri 模板检查已通过。存量 Wails/Go 模块仍作为迁移期代码存在，本轮不做全量架构迁移。

行为录制功能已经完成 P0/P1 代码修复：点击语义去重、同 page target 刷新/导航重注入、session 目录初始化、CDP 回放响应确认、store nil guard、LLM 执行录制语义对齐、基础元数据、坐标缩放、滚动容器路径、速度扰动和自动录制写入错误处理均已落地。P2 已完成错误码语义、cleanup POST、统一 active recording 状态、前端 API 收口和事件监听释放；P3 已完成 summary list、详情事件分页、300ms 搜索 debounce、敏感输入脱敏、导入导出、复制模板、裁剪、回放进度、失败重试和真实指纹 Chromium E2E。本轮剩余问题已收口：same-page iframe 聚合与可连接 OOPIF/CDP iframe target 同步已完成；新 tab/多 page target 已改为浏览器级 `Target.targetCreated/targetInfoChanged` 事件驱动同步，并保留 100ms 轮询兜底；manifest/index 已完成，detail page 已走索引；EventMonitor 表格虚拟滚动、BrowserLogs 分页增量加载和 Table 虚拟滚动 warning 均已完成。当前不再把跨 target/iframe/manifest-index/长列表日志性能作为未完成主线，最新 `go test -count=1 ./backend/...` 已通过。

实例工作台已转向非容器化主线：浏览器保持外部真实顶层窗口运行，软件内提供实例可视化、筛选、批量 API 操作、截图预览、任务队列持久化、虚拟滚动和 Windows 外部窗口激活/排列。该方向避免 HWND re-parent、WebView2 替代和 headless screencast，优先保持指纹浏览器自然窗口形态。

专项技术指导见 [06-behavior-recording-technical-guidance.md](./06-behavior-recording-technical-guidance.md)。
实例工作台专项指导见 [07-instance-workbench-technical-guidance.md](./07-instance-workbench-technical-guidance.md)。

## 已完成功能

### 基础架构

- 前端存在行为录制页面入口，路由接入 `frontend/src/App.tsx`。
- 后端行为录制入口集中在 `backend/app_behavior.go`。
- 录制、回放、自动养号录制和存储逻辑位于 `backend/internal/behavior/`。

### 核心业务能力

- 支持对运行中的浏览器实例开始/停止录制。
- 支持保存录制 JSON、列表、详情、删除、重命名。
- 支持把保存的录制回放到运行中的实例。
- 支持快速养号录制和自然语言任务执行后保存录制。
- 支持实例工作台卡片化查看全部实例，运行中实例可读取当前 URL/标题。
- 支持工作台内批量启动、停止、打开 URL、刷新和截图预览。
- 支持按 PID 激活真实外部浏览器窗口，并对选中运行实例做平铺/主辅排列。
- 支持前端任务队列，默认最多 3 个任务并发执行，最近 200 条任务持久化到后端。
- 支持实例列表虚拟滚动，替代此前前 200 条截断显示。
- 支持本地 CDP 指纹体检和身份强度报告，覆盖 UA、Client Hints、Intl、screen、Canvas、Audio、WebGL、Fonts、storage、WebRTC 能力等 70+ 采集字段；只读运行态，不导航第三方检测站，不写 profile/cookie。
- 支持代理池从公开 raw 列表抓取免费 HTTP/HTTPS/SOCKS5 候选，后端并发直连检测出口 IP，仅导入检测通过的代理，并用 `last_ip_health_json` 覆盖式持久化最新 IP 检测结果。

### 维护性与工程能力

- `go test -count=1 ./backend/internal/behavior` 已通过。
- `go test -count=1 ./backend -run "TestLlm|TestBehavior"` 已通过。
- `go test -count=1 ./backend/...` 已通过。
- `npm --prefix frontend run build` 已通过。
- `npm run build` 已通过。
- `$env:ANT_RECORDING_E2E='1'; go test -count=1 ./backend/internal/behavior -run TestRealFingerprintBrowserRecordPlayback -v` 已通过，覆盖真实指纹 Chromium 录制/回放闭环。
- `git diff --check` 已通过。
- `powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot D:\SelfMadeTool\antbrowser` 最新已通过；Table 虚拟滚动 warning 已消除。
- 行为录制专项待办已沉淀到维护文档。
- 实例工作台改动已通过前端生产构建、后端顶层包测试和相关后端包测试。
- 窗口控制自动 E2E 已用 Notepad 验证 PID 顶层窗口查找、移动和激活路径。
- 身份强度体检已通过本地单测、全量后端测试、Tauri release 构建、真实两实例工作台验收和同一 CDP 探针的指纹迁移回归；验证 user-data-dir、fingerprint args、launch args、cookie marker 均保持一致。
- 代理池免费代理导入已通过 `go test -count=1 ./backend/...`、`npm run build`、`npm run tauri:build`、`git diff --check` 和 Win11 Tauri 基线脚本。

## 进行中事项

- 行为录制完善：P0/P1/P2 与 P3 基础性能/产品工具切片已落地并通过真实 E2E；iframe/OOPIF target 同步、新 tab/多 page target 事件驱动同步、manifest/index、EventMonitor 虚拟滚动、BrowserLogs 分页增量加载和 Table 虚拟滚动 warning 已收口；剩余边界是浏览器级 target discovery 不可用时的 100ms 轮询兜底提示和真实业务页回归样例。
- 实例工作台完善：补预览节流、多屏/最小化增强和真实浏览器手工验收。

## 已知阻塞与风险

- 真实指纹 Chromium 基础录制/回放 E2E 已通过；same-page iframe 聚合、可连接 OOPIF/CDP iframe target 同步与新 tab/多 page target 事件驱动同步已完成。warning：浏览器级 target discovery 不可用时会降级到 100ms 轮询兜底，强实时首击场景需保留提示；内部滚动容器还需要真实业务页面验收。
- manifest/index 已完成，detail page 已走索引；后续重点是用超大录制样本复核磁盘读取成本，不要退回只为 eventCount 或某一页事件读取全文件。
- 敏感输入默认脱敏后不会保存真实密码/验证码，因此这类录制不能回放真实敏感值，这是刻意的安全边界。
- Windows 可能因前台窗口限制拒绝 `SetForegroundWindow`，此时 API 操作、截图和排列不受影响，但“激活”会返回失败状态。
- Win11 Tauri 模板检查脚本最新已通过；EventMonitor、BrowserLogs 和共享 Table 的长列表/日志性能基线已收口，后续表格/长列表改动仍必须继续执行虚拟滚动或分页策略。

## 下一步 3-5 项

1. 用真实业务页验证浏览器级 target discovery 事件驱动同步；若目标浏览器不暴露 browser websocket，则保留 100ms 轮询兜底提示。
2. 用真实业务页面回归 iframe 聚合、跨 target、新 tab 和内部滚动容器样例。
3. 用超大录制样本复核 manifest/index 与 detail page 索引读取成本。
4. 发布前补 release build、冷启动/RSS/进程数基线测量。
5. 为实例工作台补预览节流、多屏布局和布局恢复。

## 与 README 或旧文档的不一致处

- README 面向用户介绍项目能力；本目录记录内部真实维护状态。行为录制当前可按“基础录制/回放已真实验收”描述，但不应按“完整生产级已完善”对外描述。

## 2026-04-30 身份强度体检与指纹维度扩展

- 已完成：新增 `IdentityReportProfile(profileId)`，在实例运行且 debugReady 后通过当前真实 fingerprint Chromium 的 CDP 只读采集，不改变浏览器启动链路、不进入 Tauri WebView、不重建或改写 profile。
- 已完成：`FingerprintSnapshot` 扩展到 70+ 字段，身份报告按指纹可见性、一致性、Profile 持久化、代理网络、自然行为、自动化安全 6 个分项评分。
- 已完成：工作台新增批量/单实例“身份体检”，卡片和右侧面板展示分数、等级、异常摘要和维度数量；原“指纹体检”继续保留为轻量检查。
- 已完成：事件注册表扩展到 466 个事件，覆盖 identity/fingerprint/cookie/profile/behavior/workbench/network/automation 域，并用测试锁住 350+ 事件目标。
- 已加固：`user-data-dir` 父级跳转检测改为按路径段 fail-closed，`data\profiles\..\other-profile` 这类边界会被标为身份风险。
- 已验证：`go test -count=1 -timeout 8m ./backend/...`、`npm run build`、`npm run tauri:build`、`cargo test`、Win11 Tauri baseline、`git diff --check`、真实两实例工作台验收、Tauri 指纹迁移回归均通过。
- 边界：本地体检不访问第三方指纹站，不代表外部风控通过率；它只证明当前实例的本机可见字段、启动链路和 profile/cookie 持久性没有被软件层破坏。

## 2026-04-30 代理池免费代理抓取与 IP 检测持久化

- 已完成：新增免费代理候选抓取与解析能力，默认读取少量公开 raw 代理列表，也支持用户在代理池弹窗中填写自定义 source URL。
- 已完成：新增 HTTP/HTTPS/SOCKS5 直连并发检测，只通过候选代理访问轻量 IP 信息接口，不走 Clash/Xray/sing-box 桥接，不启动浏览器或额外常驻服务。
- 已完成：检测通过的代理才进入正式代理池；每个导入代理同步写入最新 `ProxyIPHealthResult`，再次检测或保存代理列表时覆盖 `browser_proxies.last_ip_health_json`，不追加历史流水。
- 已完成：修复 `SaveBrowserProxies` 先清表再写入导致测速/IP 健康运行时字段丢失的问题；保存列表时会保留并写回已有 `last_latency_ms`、`last_test_*` 和 `last_ip_health_json`。
- 已验证：`go test -count=1 ./backend/internal/proxy ./backend/cmd/antbrowser-core ./backend`、`go test -count=1 ./backend/...`、`npm run build`、`npm run tauri:build`、`git diff --check`、`powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot D:\SelfMadeTool\antbrowser` 均通过。

## 2026-04-29 行为录制 P2/P3 后端切片

- 已完成：录制 HTTP API 错误语义补齐，业务错误可区分 400/404/409/503，未知内部错误仍按 500 并隐藏细节。
- 已完成：`/api/recording/sessions/cleanup` 支持 POST；GET 保留为兼容入口，并在响应中标记 `deprecated` 与兼容说明。
- 已完成：新增后端 active recording 状态结构与接口，HTTP 暴露 `/api/recording/status`，Wails/App 层暴露 `BehaviorRecordingStatus` / `ActiveRecordingStatus`。
- 已完成：新增录制 summary/meta 列表能力，HTTP `/api/recording/list` 默认返回 `RecordingSummary`，不携带完整 `events`；详情支持按需读取事件，Wails 使用 `BehaviorGetRecordingDetail(id, offset, limit)` 分页返回。
- 已验证：`go test -count=1 ./backend/internal/behavior ./backend/internal/launchcode`、`go test -count=1 ./backend -run "TestBehavior|TestRecording"`、`go test -count=1 ./backend/...` 已通过。
- 已完成：前端已优先使用 `BehaviorRecordingSummaryList`、`BehaviorRecordingStatus` 和 `BehaviorGetRecordingDetail`；旧 `BehaviorRecordingList` 保留为兼容入口。

## 2026-04-29 行为录制 P3 产品工具切片

- 已完成：新增 `BehaviorRecordingExport`、`BehaviorRecordingImport`、`BehaviorRecordingCopy`、`BehaviorRecordingTrim`，导入/复制/裁剪均另存为新录制。
- 已完成：回放引擎新增进度回调，Wails 事件 `automation:playback:progress` 会上报 `recordingId`、事件进度、百分比、耗时和状态；完成/失败事件补 `recordingId`。
- 已完成：前端录制面板新增 JSON 文本导入/导出、列表行复制模板、回放进度显示和失败重试；详情弹窗新增 1-based inclusive 事件范围裁剪。
- 已验证：`go test -count=1 ./backend -run "TestBehaviorRecording|TestBehaviorPlayback|TestBehaviorGetRecordingDetail"`、`go test -count=1 ./backend/internal/behavior -run "TestPlaybackProgress"`、`go test -count=1 ./backend/...`、`npm --prefix frontend run build`、真实指纹 Chromium E2E 已通过。
## 2026-04-29 实例工作台本地 API 闭环

- 已完成：LaunchServer 新增 `POST /api/instances/stop`，通过 App 层受控停止真实浏览器实例，停止成功后清理 active CDP profile。
- 已完成：LaunchServer 新增 `/api/workbench/navigate|refresh|screenshot|activate|arrange` 本地 HTTP 入口，复用现有 `Synchronizer*` 后端能力，方便外部自动化验收软件内 API 操作。
- 已完成：新增 `scripts/verify-workbench-two-instances.ps1`，默认使用 2 个未运行 profile 做真实启动、导航、刷新、截图、窗口排列、停止隔离和清理闭环。
- 已验证：`go test ./backend/...`、`npm --prefix frontend run build`、`wails build`、`wails build -skipbindings`、`git diff --check` 均通过。
- 已验证：2 实例脚本实际启动并停止两个指纹 Chromium 实例；停第一个后第二个仍可刷新和截图；脚本结束后 `personal-pilot` 进程和 `19876/api/health` 未残留。
- 说明：此前“用至少 2 个真实运行实例手工验收工作台导航/刷新/截图/排列/停止”的缺口，已由 `scripts/verify-workbench-two-instances.ps1` 自动验收覆盖；后续重点转为多屏、最小化、布局恢复和预览节流。
