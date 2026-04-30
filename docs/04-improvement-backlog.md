# antbrowser 长期改进建议池

本文件记录长期改进建议，不直接替代路线图。
状态词固定为：`Idea`、`Planned`、`In Progress`、`Done`、`Dropped`。

## 产品体验

| ID | 标题 | 现象/问题 | 建议方向 | 优先级 | 状态 | 备注/证据 |
| --- | --- | --- | --- | --- | --- | --- |
| UX-001 | 行为录制状态不同步 | 浏览器列表页和录制页曾各自维护录制中状态 | 后端暴露 active recording 状态，前端统一查询 | P2 | Done | `BehaviorRecordingStatus` / `fetchRecordingStatus()` 已接入 `BrowserListPage.tsx` 和 `RecordingPanel.tsx` |
| UX-002 | 长录制缺产品化工具 | 曾缺录制裁剪、导入导出、复制模板、回放进度、失败重试 | 保留最小入口，后续增强文件选择器、批量导入和更强预览 | P3 | Done | 已有详情弹窗、重命名、基础统计、JSON 文本导入/导出、复制模板、事件范围裁剪、回放进度和失败重试 |
| UX-003 | 自然语言任务完成态不完整 | 失败/取消态和监听释放曾不完整 | 补取消和失败态，并保持逐个 off 释放 | P2 | Done | `NaturalLanguageTask.tsx` 已经通过 API 层调用并逐个 off |
| UX-004 | 实例工作台真实窗口激活未完成 | 工作台已能 API 操作和预览，现已补 Windows-only 外部窗口激活/平铺/主辅排列 | 继续做多屏、最小化其他窗口和布局恢复增强 | P1 | Done | `SynchronizerPage.tsx`、`app_synchronizer.go`、`window_control_windows.go` |
| UX-005 | 工作台任务队列未持久化 | 当前任务队列是前端内存态，刷新页面会丢失历史 | 后端保存最近 200 条任务；后续再补分页筛选 | P2 | Done | `app_synchronizer.go`、`SynchronizerPage.tsx` |
| UX-006 | 代理池缺免费候选导入 | 只能手动导入或刷新 Clash 订阅，免费代理候选缺自动抓取、直连检测和最新 IP 结果持久化 | 后端抓取公开 raw 列表，HTTP/HTTPS/SOCKS5 并发直连检测，通过后入库，最新 IP 检测结果覆盖写入 | P2 | Done | `BrowserProxyImportFreeDirectProxies`、`free_proxy.go`、`ProxyPoolPage.tsx` |
| UX-007 | 工作台缺身份强度可见化 | 用户无法在软件内判断当前运行实例的指纹维度、一致性和 profile 安全状态 | 增加本地 CDP 指纹体检、身份强度报告、70+ 字段采集、466 个事件注册与迁移回归脚本同探针对比 | P1 | Done | `IdentityReportProfile`、`identity_report.go`、`fingerprint_verifier.go`、`SynchronizerPage.tsx` |

## 前端

| ID | 标题 | 现象/问题 | 建议方向 | 优先级 | 状态 | 备注/证据 |
| --- | --- | --- | --- | --- | --- | --- |
| FE-001 | 录制搜索无 debounce | 每次输入立即过滤 | 使用 250-400ms debounce | P3 | Done | `RecordingPanel.tsx` 使用 300ms debounce |
| FE-002 | 录制列表无虚拟滚动 | 大量录制时全量渲染 | 超过 200 条使用虚拟滚动或分页 | P3 | Done | `RecordingPanel.tsx` 使用 25/50/100 分页 |
| FE-003 | 详情事件列表只截取最近 50 条 | 长录制无法完整检查，也缺分页/虚拟化 | 详情按需加载事件，支持分页或虚拟列表 | P3 | Done | `RecordingDetailModal.tsx` 使用分页，`BehaviorGetRecordingDetail` 支持 offset/limit |
| FE-004 | 前端直接调用 Wails 绑定 | 部分行为页面未完全走统一 API 层 | 统一收口到 browser api/service | P2 | Done | 行为录制页、录制面板和自然语言任务已走 `frontend/src/modules/browser/api.ts` |
| FE-005 | 工作台超过 200 个实例只截断显示 | 为避免大列表全量渲染，当前仅显示前 200 个匹配项 | 已改为固定行高虚拟滚动，保留搜索 debounce | P1 | Done | `SynchronizerPage.tsx` |

## 后端

| ID | 标题 | 现象/问题 | 建议方向 | 优先级 | 状态 | 备注/证据 |
| --- | --- | --- | --- | --- | --- | --- |
| BE-001 | 点击重复回放 | 录制 down/up/click，回放也分别执行三类事件 | 统一事件语义，避免一次真实点击多次触发 | P0 | Done | 新录制保留 down/up，旧派生 click 保存前去重 |
| BE-002 | 跨页面录制丢事件 | 事件只保存在当前页面 window 变量 | 增加后端缓冲或导航后重注入，记录 tab/frame/url | P0 | Done | 已覆盖同 CDP page target 内刷新/导航、新 tab/多 page target 事件驱动同步、same-page iframe 聚合和可连接 OOPIF/CDP iframe target 同步；browser websocket 不可用时保留 100ms 轮询兜底 |
| BE-003 | 会话恢复未落地 | `recordingSessionDir` 未初始化 | 启动时初始化 `.sessions` 目录并补测试 | P0 | Done | 已初始化 `data/recordings/.sessions` 并补测试 |
| BE-004 | 回放不确认 CDP 响应 | 写 WebSocket 后不读取结果 | 增加 request/response 匹配和 CDP error 处理 | P0 | Done | CDP response/error/timeout 已处理 |
| BE-005 | 录制存储空指针风险 | store 初始化失败后 API 直接调用 | 所有 API 加 store 可用性检查 | P0 | Done | 行为录制 API 与 LLM 保存路径已走统一 guard |
| BE-006 | LLM 执行与录制语义不一致 | JS 直接改 DOM，录制脚本可能听不到 | 统一走 CDP 输入层或显式写入录制事件流 | P0 | Done | click/scroll/type 走 CDP Input 或显式录制事件 |
| BE-007 | 当前标签页选择不可靠 | CDP 连接第一个 page target | 增加 active tab 选择策略 | P1 | Done | 已改为 page target 评分选择 |
| BE-008 | 自动养号录制忽略写入错误 | 多处 `_ = conn.WriteJSON` | 错误应中断或记录失败事件 | P1 | Done | 写入错误已返回，不静默保存残缺录制 |
| BE-009 | 录制 API 错误码粗糙 | 业务错误多走 500 | 区分 400/404/409/503/500 | P2 | Done | `recordingHTTPStatus`、store typed errors 和 HTTP 错误码矩阵测试已落地 |
| BE-010 | cleanup 使用 GET 变更状态 | 清理会话是变更操作 | 改 POST 并保留兼容说明 | P2 | Done | POST 已支持，GET 保留兼容并返回 `deprecated` |
| BE-011 | 工作台截图依赖 CDP 当前页 | 运行中实例 debugPort 未就绪时无法预览 | 卡片状态提示并保留手动重试；后续可补窗口截图降级 | P2 | Planned | `app_synchronizer.go` |

## 稳定性与可维护性

| ID | 标题 | 现象/问题 | 建议方向 | 优先级 | 状态 | 备注/证据 |
| --- | --- | --- | --- | --- | --- | --- |
| MTN-001 | 全仓后端测试未通过 | `go test ./backend/...` 失败在 `backend/internal/apppath` Darwin 路径测试 | 修复路径测试或平台条件，恢复质量门禁 | P0 | Done | 已修复 Darwin HOME 模拟路径，`go test -count=1 ./backend/...` 通过 |
| MTN-002 | 缺真实浏览器 E2E 验收 | 行为包单测通过，但真实浏览器测试硬编码端口且可 skip | 建立可重复 E2E 或手工验收脚本 | P1 | Done | `ANT_RECORDING_E2E=1 go test -run TestRealFingerprintBrowserRecordPlayback` 已通过真实指纹 Chromium 录制/回放 |
| MTN-003 | 录制数据敏感信息风险 | 输入内容可能明文保存到 JSON | 默认屏蔽密码/验证码字段，必要时加密或用户确认 | P1 | Done | 敏感输入默认脱敏保存；真实敏感值不回放 |
| MTN-004 | 录制列表返回完整事件 | list API 和前端 state 都持有完整 events | 拆 metadata list/detail，详情按需加载 | P2 | Done | `BehaviorRecordingSummaryList` / HTTP list 返回 summary；详情走 `BehaviorGetRecordingDetail` 分页，manifest/index 已用于长录制索引读取 |
## 2026-04-29 收口记录

- MTN-005 / Done：实例工作台真实 2 实例自动验收入口已补齐。证据：`POST /api/instances/stop`、`/api/workbench/*`、`scripts/verify-workbench-two-instances.ps1`，以及脚本真实运行通过。
- 后续仍保留：多屏指定、最小化其他窗口、布局恢复、预览节流和 debugPort 未就绪时的窗口截图降级。

## 2026-04-29 Identity Asset Safety Gate V1 Backlog Entry

- MTN-006 / In Progress: Identity Asset Safety Gate V1 scripted acceptance.
- Problem: profile identity assets can only be trusted if release scripts prove user-data-dir uniqueness, Cookie persistence, path mismatch blocking, profile isolation, fingerprint stability, and no residual verification processes.
- Direction: keep the checks in `scripts/verify-workbench-two-instances.ps1` and `scripts/verify-tauri-fingerprint-regression.ps1`; do not move this into Go/Tauri UI until the acceptance surface is stable.
- Evidence required for Done: release script JSON showing duplicate/path mismatch blocked, Cookie marker persisted after restart, profile markers did not cross, stop-one-keep-one passed, no residual processes remained, and all listed fingerprint fields matched baseline/candidate.
