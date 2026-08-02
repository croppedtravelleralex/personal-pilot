# 行为录制技术指导

## 结论

当前行为录制已经完成 P0/P1 可靠性修复、P2 接口/状态修复，以及 P3 的列表/详情性能基础改造和产品化工具入口，并已通过真实指纹 Chromium 录制/回放 E2E。本轮剩余问题已收口：same-page iframe 聚合与可连接 OOPIF/CDP iframe target 同步已完成；新 tab/多 page target 已改为浏览器级 `Target.targetCreated/targetInfoChanged` 事件驱动同步，并保留 100ms 轮询兜底；manifest/index 已完成，detail page 已走索引；EventMonitor 表格虚拟滚动、BrowserLogs 分页增量加载和 Table 虚拟滚动 warning 均已收口。当前不再把跨 target/iframe/manifest-index/长列表日志性能作为未完成主线，最新 `go test -count=1 ./backend/...` 已通过。

核心问题集中在三类：

- 录制与回放语义不一致，真实点击、输入、滚动容易被重复、漏记或偏移。
- 会话、页面、标签页和上下文管理不完整，长流程和跨页面流程不稳。
- 性能、隐私、测试和质量门禁还没有达到长期维护标准。

## 事实来源

- 后端入口：`backend/app_behavior.go`
- 录制注入：`backend/internal/behavior/inject_script.go`
- 录制器：`backend/internal/behavior/recorder.go`
- 回放引擎：`backend/internal/behavior/playback.go`
- 自动养号录制：`backend/internal/behavior/auto_recorder.go`
- 录制存储：`backend/internal/behavior/recording_store.go`
- 自然语言任务：历史上曾存在 `backend/app_llm.go` 设想；当前仓库不再提供该入口
- 前端入口：`frontend/src/modules/browser/pages/BehaviorRecordingPage.tsx`
- 前端控制面板：`frontend/src/modules/browser/components/RecordingPanel.tsx`
- 前端自然语言任务：`frontend/src/modules/browser/components/NaturalLanguageTask.tsx`

本轮已验证命令：

- `go test -count=1 ./backend/internal/behavior`：通过
- `go test -count=1 ./backend -run "TestLlm|TestBehavior"`：通过
- `go test -count=1 ./backend/internal/behavior/humanize`：通过
- `go test -count=1 ./backend/...`：通过
- `npm --prefix frontend run build`：通过
- `npm run build`：通过
- `$env:PERSONAL_PILOT_RECORDING_E2E='1'; go test -count=1 ./backend/internal/behavior -run TestRealFingerprintBrowserRecordPlayback -v`：通过；覆盖真实指纹 Chromium 启动、录制、点击/输入/滚动、刷新后继续点击、停止、回放和重复点击断言
- `git diff --check`：通过
- `powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot D:\SelfMadeTool\personal-pilot`：通过；Table 虚拟滚动 warning 已消除。

## 2026-04-29 P0/P1 执行结果

| ID | 阶段 | 状态 | 本轮落地 |
| --- | --- | --- | --- |
| BR-P0-001 | P0 | Done | 新录制不再保存派生 `click`，保留 `down/up`；旧录制保存前做 click 去重，避免一次点击回放多次触发 |
| BR-P0-002 | P0 | Done | 刷新/同一 CDP page target 内导航后重注入录制脚本并累计事件；新标签/跨 target 仍列为后续边界 |
| BR-P0-003 | P0 | Done | 启动初始化 `data/recordings/.sessions`，session 文件名安全编码，补保存/恢复/清理测试 |
| BR-P0-004 | P0 | Done | 自然语言任务不再对外冒充独立 LLM 入口；录制相关动作仍只通过 CDP Input / 显式录制事件流处理 |
| BR-P0-005 | P0 | Done | CDP 回放命令增加 request/response 匹配、超时和 CDP error 处理 |
| BR-P0-006 | P0 | Done | 行为录制 API 统一走 `requireRecordingStore`，避免 store nil panic；旧 LLM 保存路径已移出当前主线 |
| BR-P1-001 | P1 | Done | CDP page target 选择改为评分策略，优先可用的当前/活动 page |
| BR-P1-002 | P1 | Done | Recording 增加 URL、标题、DPR、scale、viewport 等元数据 |
| BR-P1-003 | P1 | Done | 回放按录制 viewport 与当前 viewport 做坐标比例映射 |
| BR-P1-004 | P1 | Done | 增加 input/change/paste/composition 类录制策略，敏感字段默认脱敏 |
| BR-P1-005 | P1 | Done | wheel 录制记录滚动容器 `targetPath`，回放优先定位容器执行滚动，失败时回退坐标 wheel |
| BR-P1-006 | P1 | Done | `speedVariation` 已进入回放时间调度 |
| BR-P1-007 | P1 | Done | jitter 改为有界正负扰动，避免长流程只变慢 |
| BR-P1-008 | P1 | Done | 自动养号录制的 CDP `WriteJSON` 错误改为返回错误，不再静默保存残缺录制 |

## 完善标准

行为录制达到可交付状态前，至少满足：

1. 一次真实点击只回放一次，不重复触发业务操作。
2. 跨页面、刷新、当前标签页切换、常见 iframe 场景不会静默丢事件。
3. 输入、滚动、点击三类核心动作可录制、可回放、可验证。
4. 录制列表只加载元数据，详情按需加载事件，长录制不会拖慢页面。
5. 敏感输入有默认保护策略，不明文保存密码、验证码等高风险内容。
6. 后端 API 对空存储、无运行实例、CDP 未就绪、重复录制、重复回放都有明确错误。
7. 至少有一条真实浏览器端到端录制回放测试或可重复的手工验收脚本。
8. 通过结构检查、类型检查、行为包测试、前端生产构建和全仓后端测试。

## 2026-04-29 剩余问题收口状态

- same-page iframe 聚合和可连接 OOPIF/CDP iframe target 同步已完成，后续不再把 iframe 覆盖作为主线未完成项；若新增复杂嵌套 iframe 场景，应按回归样例补验收。
- 新 tab/多 page target 事件驱动同步已完成；通过浏览器级 `Target.setDiscoverTargets` 监听 `Target.targetCreated/targetInfoChanged`，target 创建后立即同步注入。若浏览器级 CDP 不可用，则降级为 100ms 轮询兜底并保留提示。
- manifest/index 已完成，detail page 已走索引；长录制详情不应退回一次性全量 JSON 读取。
- EventMonitor live/history 已使用共享 Table 虚拟滚动/分页，BrowserLogs 已改为 `GetAppLogsPage` 分页过滤、300ms 搜索 debounce、page-size 控制和跟随最新页增量刷新。
- Table 虚拟滚动 warning 已消除；后续新增超过 200 条可见记录的列表/表格，仍按虚拟滚动或分页规则处理。
- 最新全量后端测试、前端构建、真实浏览器 E2E、Win11 Tauri baseline 和 diff 检查均已通过。

说明：下面 P0-P3 表格是 2026-04-29 初始评估基线，用于保留问题来源和当时的指导方向；当前真实状态以本文的执行结果、P2/P3 收口核验和本节“剩余问题收口状态”为准。

## 必修问题

| ID | 优先级 | 问题 | 风险 | 指导方向 |
| --- | --- | --- | --- | --- |
| BR-P0-001 | P0 | 录制 `mousedown`、`mouseup`、`click`，回放又分别执行三类事件 | 一次点击可能回放成两次点击 | 统一事件语义；推荐保存 `down/up` 原子事件，或只保存 `click` 派生事件，不能两套同时回放 |
| BR-P0-002 | P0 | 事件只存在当前页面 `window.__antRecordedEvents` | 页面刷新、跳转、多标签后丢录制 | 建立后端会话缓冲或持久队列；导航后自动重注入；记录 URL/tab/frame 上下文 |
| BR-P0-003 | P0 | `recordingSessionDir` 未初始化 | 会话恢复和卡死清理基本不生效 | 在启动阶段初始化 session 目录，如 `data/recordings/.sessions`，并补保存/恢复测试 |
| BR-P0-004 | P0 | 自然语言任务用 JS 直接改 DOM，录制脚本只听 DOM 输入/鼠标/滚轮事件 | 执行成功但录制结果稀疏或不一致 | LLM 执行动作应走同一套 CDP 输入层，或显式写入录制事件流 |
| BR-P0-005 | P0 | 回放只写 CDP WebSocket，不读取命令响应 | 回放失败可能被误报为成功 | 增加带超时的 CDP request/response 匹配，处理 CDP error |
| BR-P0-006 | P0 | `recordingStore` 初始化失败后各 API 直接调用 | 运行时 nil panic | 所有行为录制 API 先检查 store，可返回 503/明确错误 |

## 功能正确性问题

| ID | 优先级 | 问题 | 指导方向 |
| --- | --- | --- | --- |
| BR-P1-001 | P1 | `ConnectPageCDP` 取第一个 page target，不一定是当前活动标签 | 按 profile 当前 tab 或最近活动 page target 选择；必要时提供 tabId |
| BR-P1-002 | P1 | Recording 缺起始 URL、页面标题、tabId、frameId、DPR、缩放、滚动容器信息 | 扩展录制元数据，回放前校验上下文 |
| BR-P1-003 | P1 | 坐标未适配视口、DPR、缩放和窗口变化 | 增加坐标归一化和视口比例映射 |
| BR-P1-004 | P1 | 键盘只记 `keydown`，中文输入、粘贴、组合输入不可靠 | 增加 `input/change/paste/composition` 策略；敏感字段可屏蔽 |
| BR-P1-005 | P1 | 滚动回放主要操作 `window.scrollBy`，不支持内部滚动容器 | 记录滚动目标元素路径和滚动容器；回放时定位容器 |
| BR-P1-006 | P1 | `speedVariation` 有 UI 和类型但后端未使用 | 在时间调度和鼠标轨迹中真正应用速度变化 |
| BR-P1-007 | P1 | 时间抖动只正向增加，长流程会整体变慢 | 改为有界正负 jitter，并保持总体时长接近原始录制 |
| BR-P1-008 | P1 | 自动养号录制大量忽略 `WriteJSON` 错误 | 写入错误应中断或记录失败事件，不能静默保存残缺录制 |

## 架构与接口问题

| ID | 优先级 | 问题 | 指导方向 |
| --- | --- | --- | --- |
| BR-P2-001 | P2 | 录制 API 错误码粗糙，业务错误多走 500 | 区分 400、404、409、503、500 |
| BR-P2-002 | P2 | `/api/recording/sessions/cleanup` 是变更操作却用 GET | 改为 POST，同时保留兼容期或文档说明 |
| BR-P2-003 | P2 | 前端部分页面直接调用 Wails 绑定 | 统一收口到 `frontend/src/modules/browser/api.ts` 或服务层 |
| BR-P2-004 | P2 | 浏览器列表页和录制页各自维护录制状态 | 后端暴露 active recording 状态，前端订阅统一事件或查询统一状态 |
| BR-P2-005 | P2 | `EventsOff(eventName)` 会移除同事件全部监听 | 使用 `EventsOn` 返回的 off 函数逐个释放 |

## 性能、体验与安全问题

| ID | 优先级 | 问题 | 指导方向 |
| --- | --- | --- | --- |
| BR-P3-001 | P3 | 列表接口返回完整 `events`，前端全量保存 | 拆成 metadata list/detail；详情按需拉取事件 |
| BR-P3-002 | P3 | 搜索无 debounce，列表无虚拟滚动 | 搜索默认 250-400ms debounce；超过 200 条列表虚拟化 |
| BR-P3-003 | P3 | 录制 JSON 明文保存输入内容 | 默认屏蔽密码/验证码字段；提供用户确认和加密/脱敏策略 |
| BR-P3-004 | P3 | 缺录制裁剪、预览、导入导出、复制模板、回放进度、失败重试 | 已补最小产品化入口；后续可继续做文件选择器、批量导入和更强预览 |
| BR-P3-005 | P3 | 真实浏览器测试依赖硬编码端口且可 skip | 建立可重复 E2E 或手工验收脚本，输出固定证据 |

## 2026-04-29 P2/P3 收口核验

本节记录 P2/P3 主线落地状态。真实浏览器基础验收已由 `TestRealFingerprintBrowserRecordPlayback` 跑通；后续收口已完成 iframe/OOPIF target 同步、新 tab/多 page target 事件驱动同步、manifest/index、EventMonitor 虚拟滚动、BrowserLogs 分页增量加载与 Table 虚拟滚动 warning 清理，剩余边界是 target discovery 降级提示和真实业务页样例。

| ID | 阶段 | 收口状态 | 证据 | 剩余口径 |
| --- | --- | --- | --- | --- |
| BR-P2-001 | P2 | Done | `recordingHTTPStatus` 支持 400/404/409/503/500 映射，App 层业务错误暴露 `HTTPStatusCode()` | 未知内部错误仍统一按 500 隐藏细节 |
| BR-P2-002 | P2 | Done | `/api/recording/sessions/cleanup` 支持 POST；GET 保留兼容并返回 `deprecated` / `compatibility` | 后续可在版本切换时移除 GET 兼容 |
| BR-P2-003 | P2 | Done | `BehaviorRecordingPage.tsx`、`RecordingPanel.tsx`、`NaturalLanguageTask.tsx` 行为录制调用收口到 `frontend/src/modules/browser/api.ts` | 其他非行为录制页面的历史 Wails 调用不在本切片范围 |
| BR-P2-004 | P2 | Done | 后端暴露 `ActiveRecordingStatus` / `BehaviorRecordingStatus` / HTTP `/api/recording/status`；`BrowserListPage.tsx` 与 `RecordingPanel.tsx` 均从 `fetchRecordingStatus()` 同步录制中 profile | 当前用查询/轮询同步，不依赖前端本地状态作为权威来源 |
| BR-P2-005 | P2 | Done | `RecordingPanel.tsx` 播放事件通过 `api.ts` 中的订阅函数释放 `EventsOn` 返回的 off 函数；`NaturalLanguageTask.tsx` 现在只做本地预览，不再对外宣称可执行 LLM 事件 | `CoreManagementPage` 等非录制页如有历史 `EventsOff`，另行治理；旧实验页已移除 |
| BR-P3-001 | P3 | Done | 新增 `RecordingSummary`、`RecordingDetailPage`、`BehaviorRecordingSummaryList()`、`BehaviorGetRecordingDetail(id, offset, limit)`；HTTP list 默认返回 metadata，Wails 详情按页返回 events；manifest/index 已完成，detail page 已走索引 | 后续用超大录制样本复核读取成本，不应退回一次性全量 JSON 读取 |
| BR-P3-002 | P3 | Done | `RecordingPanel.tsx` 搜索使用 300ms debounce；录制列表使用 25/50/100 分页；`RecordingDetailModal.tsx` 事件列表使用 50/100/200 分页 | 未新增虚拟滚动库；本切片选择分页满足大量数据渲染约束 |
| BR-P3-003 | P3 | Done | `inject_script.go` 识别 password/otp/captcha/token 等敏感字段并标记 `sensitive`；`recorder_model_test.go` 验证敏感 text 被清空 | 真实敏感值不会回放，这是安全边界 |
| BR-P3-004 | P3 | Done | 后端新增 `BehaviorRecordingExport/Import/Copy/Trim` 与 `RecordingExportBundle`；前端新增 JSON 文本导入/导出、复制模板、详情裁剪、回放进度条和失败重试 | 当前是最小可用入口；文件选择器、批量导入和更强预览可作为后续增强 |
| BR-P3-005 | P3 | Done | 新增 `TestRealFingerprintBrowserRecordPlayback`，在 `PERSONAL_PILOT_RECORDING_E2E=1` 时启动真实指纹 Chromium 并验证录制/回放闭环 | 基础 E2E 不单独覆盖所有复杂上下文；后续收口已补 iframe 聚合与新 tab/多 page target 事件驱动同步，browser websocket 不可用时保留 100ms 轮询兜底 |

## 推荐执行顺序

### 第 1 阶段：可靠性止血

目标：让基础录制回放不误触、不假成功。

1. 修点击重复回放。
2. 增加 `recordingStore` nil guard。
3. 初始化 `recordingSessionDir` 并让清理/恢复可用。
4. 回放 CDP 命令增加响应确认。
5. 补最小单元测试覆盖上述行为。

验收：

- 行为包测试通过。
- 前端构建通过。
- 手工录制一次点击、一次输入、一次滚动，回放不重复触发。

### 第 2 阶段：跨页面与状态一致

目标：长流程不丢事件，前端状态可信。

1. 增加录制元数据：URL、title、tabId、viewport、DPR。
2. 导航/刷新后重注入或后端持久缓冲。
3. 统一浏览器列表页和行为录制页的 active recording 状态。
4. 修复事件监听释放方式。

验收：

- 录制开始后跳转页面仍能继续采集。
- 停止录制后列表页和录制页状态一致。

### 第 3 阶段：自然语言任务收口

目标：不再把自然语言任务伪装成可执行 LLM 编排入口。

1. 保留本地预览，但不对外承诺执行。
2. 取消对不存在 LLM RPC 的调用。
3. 明确在文档和 UI 中标注当前只支持提示，不支持执行。

验收：

- 点击执行按钮只给出明确不支持提示，不再走不存在的后端入口。

### 第 4 阶段：性能与产品化

目标：长录制和大量录制可维护。

1. 拆 list/detail 接口。
2. 搜索 debounce。
3. 录制列表虚拟滚动或分页。
4. 详情页事件分页/虚拟化。
5. 增加导入导出、裁剪、回放进度等体验能力。

验收：

- 200 条以上录制列表仍流畅。
- 单条长录制详情不会一次性渲染全部事件。

### 第 5 阶段：质量门禁

目标：从可试用推进到可交付。

1. 保持全仓 `go test ./backend/...` 通过，新增改动必须先修复回归。
2. 增加真实浏览器 E2E 或固定手工验收脚本。
3. 跑前端生产构建。
4. 如有发布动作，再跑 release build 与基线检查。

验收：

- `go test ./backend/internal/behavior`
- `go test ./backend/...`
- `npm --prefix frontend run build`
- 真实浏览器录制回放验收通过

## AI 执行评估

本轮已经证明 AI 可以全量执行 P0-P3 主线切片，并可完成代码、真实浏览器 E2E、测试、构建和文档回写。后续继续推进时，不再重复 P0-P3 基础修复；iframe/OOPIF target 同步、新 tab/多 page target 事件驱动同步、manifest/index、EventMonitor 虚拟滚动、BrowserLogs 分页增量加载和 Table 虚拟滚动 warning 已收口，优先处理 target discovery 降级提示与真实业务页回归。

可直接执行：

- target discovery 降级到 100ms 轮询时的提示、补偿或验收策略
- 真实业务页面回归样例：iframe 聚合、跨 target、新 tab 和内部滚动容器
- 超大录制样本复核 manifest/index 与 detail page 索引读取成本

需要用户配合或环境确认：

- 敏感输入策略需要用户确认默认行为：完全不录、脱敏录、还是按字段可配置。
- 若要做 release build 和性能基线，需要确认当前项目发布命令和 Windows 11 目标打包流程。

推荐后续 AI 执行方式：

1. 先用真实业务页面回归 iframe 聚合、跨 target、新 tab 和内部滚动容器样例。
2. 再用超大录制样本复核 manifest/index、detail page 索引读取和性能基线。
3. 如进入发布前性能验收，再跑 release build、冷启动/RSS/进程数基线。
4. 每阶段结束都更新本文件、`02-current-state.md`、`03-roadmap.md` 和当月日志。

## 2026-04-29 P2/P3 后端切片落地

本次先落地后端接口语义与数据形状，随后补齐前端状态同步与详情分页接入。

| ID | 状态 | 落地内容 | 验证 |
| --- | --- | --- | --- |
| BR-P2-001 | Done | `recordingHTTPStatus` 支持业务错误映射，store 层新增 `ErrInvalidRecordingID` / `ErrRecordingNotFound`，App 层继续用 `HTTPStatusCode()` 暴露 409/503 等语义 | `TestRecordingHTTPBusinessErrorStatusCodes` |
| BR-P2-002 | Done | cleanup 变更操作支持 POST，GET 保留兼容并返回 `deprecated` / `compatibility` | `TestRecordingHTTPCleanupSupportsPostAndDeprecatedGet` |
| BR-P2-004 | Done | 新增 `ActiveRecordingStatus`，HTTP `/api/recording/status` 与 App/Wails 方法暴露 active/profileIds/recoverableProfileIds | `TestRecordingHTTPStatusEndpoint`、`TestBehaviorRecordingStatusExposesRecoverableActiveSession` |
| BR-P3-001 | Done with scale note | 新增 `RecordingSummary`、`RecordingDetailPage` 与 `ListSummaries()`；HTTP list 默认返回 metadata；Wails 详情支持 offset/limit 分页事件 | `TestFileRecordingStore_ListSummariesOmitsEventsAndKeepsEventCount`、`TestRecordingHTTPListReturnsSummariesAndDetailReturnsEvents`、`TestBehaviorGetRecordingDetailPaginatesEvents` |

兼容边界：现有 Wails `BehaviorRecordingList` 暂保持返回完整 `Recording`，避免破坏旧调用；新前端已优先使用 `BehaviorRecordingSummaryList` / `BehaviorRecordingStatus` / `BehaviorGetRecordingDetail`。

## 2026-04-29 P2/P3 前端与真实 E2E 落地

| ID | 状态 | 落地内容 | 验证 |
| --- | --- | --- | --- |
| BR-P2-003 | Done | 行为录制页面和录制面板调用收口到 `frontend/src/modules/browser/api.ts`；自然语言任务已降级为本地预览壳 | `npm --prefix frontend run build` |
| BR-P2-004 | Done | `BrowserListPage.tsx` 和 `RecordingPanel.tsx` 通过 `fetchRecordingStatus()` 同步后端 active recording 状态 | `npm --prefix frontend run build` |
| BR-P2-005 | Done | 录制/播放事件监听使用 `EventsOn` 返回的 off 函数逐个释放；自然语言任务不再监听不存在的 LLM 事件 | `npm --prefix frontend run build` |
| BR-P3-002 | Done | 录制搜索 300ms debounce，列表和详情事件分页 | `npm --prefix frontend run build` |
| BR-P3-004 | Done | JSON 文本导入/导出、复制模板、详情裁剪、回放进度显示和失败重试已接入 `RecordingPanel.tsx` / `RecordingDetailModal.tsx`，Wails 绑定同步更新 | `go test -count=1 ./backend -run "TestBehaviorRecording|TestBehaviorPlayback|TestBehaviorGetRecordingDetail"`、`npm --prefix frontend run build` |
| BR-P3-005 | Done | 真实指纹 Chromium 录制/回放 E2E 通过，事件计数包含 `change/down/input/key/move/scroll/up` | `PERSONAL_PILOT_RECORDING_E2E=1 go test -count=1 ./backend/internal/behavior -run TestRealFingerprintBrowserRecordPlayback -v` |

验证命令：

- `go test -count=1 ./backend/internal/behavior ./backend/internal/launchcode`
- `go test -count=1 ./backend -run "TestBehavior|TestRecording"`
- `go test -count=1 ./backend/...`
