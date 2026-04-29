# 非容器化实例工作台技术指导

最后更新：2026-04-29

## 目标

实例工作台的主线是：浏览器保持真实外部顶层窗口运行，软件内只做实例可视化管理、API 操作、截图预览和自动化任务编排。

这条路线优先保证指纹浏览器的自然窗口形态，不使用 WebView2 替代浏览器内核，不使用 headless screencast 作为主体验，不做 HWND re-parent 容器化。

## 当前已落地

- 路由：`/browser/synchronizer`
- 页面：`frontend/src/modules/synchronizer/SynchronizerPage.tsx`
- 前端 API：`frontend/src/modules/synchronizer/api.ts`
- 后端 API：`backend/app_synchronizer.go`
- 导航入口：`frontend/src/config/project.config.ts` 中的“实例工作台”

已支持：

- 查看全部实例卡片。
- 搜索、状态筛选、分组筛选。
- 运行中实例读取当前 URL、标题、PID、debugPort。
- 批量启动、停止、打开 URL、刷新、截图。
- 单实例启动、停止、打开 URL、刷新、截图。
- Windows-only 外部窗口激活：按运行实例 PID 查找真实可见顶层窗口并置前。
- Windows-only 外部窗口排列：对选中运行实例执行平铺或主辅布局，不做 HWND re-parent。
- 前端任务队列，默认 3 并发。
- 任务队列最近 200 条持久化到后端 `data/workbench/tasks.json`，应用重启后可恢复。
- 实例列表已改为前端虚拟滚动，不再用前 200 条截断代替大列表渲染。
- 操作记录和截图预览。

## 不做什么

- 不把浏览器嵌入应用主窗口。
- 不使用 `--headless` 作为工作台主流程。
- 不改变 profile、代理、指纹参数、浏览器内核和 debugPort 策略。
- 不把任务队列做成绕过授权或平台限制的自动化。
- 不把外部窗口排列升级成内嵌容器；窗口仍属于真实浏览器进程。

## 后端规则

- 单实例 API 必须通过 profileId 找到运行中实例。
- 只有 `Running && DebugReady && DebugPort > 0` 时才允许 CDP 操作。
- URL 输入统一归一化：无协议时默认补 `https://`。
- CDP 请求必须匹配 response id，跳过事件消息。
- 截图返回 data URL，前端只作为预览使用，不长期落库。
- 窗口激活/排列只依赖运行实例 PID，不要求 debugPort ready；失败只返回窗口任务失败，不影响 CDP API 操作。
- Windows 前台限制可能导致激活失败；排列可独立成功，不能把这类失败当作实例崩溃。

## 前端规则

- 页面只负责工作台编排，不直接拼接底层 CDP。
- 搜索使用 300ms debounce。
- 实例列表使用固定行高虚拟滚动，避免 200 条以上全量渲染。
- 任务队列状态固定为 `pending/running/success/error`。
- 任务队列保存最近 200 条，保存失败不阻断当前操作。
- 批量任务默认最多 3 并发，避免同时压垮本机和代理桥接进程。
- 激活只对单个实例执行；排列只对用户选中的运行实例执行，避免无意移动全部窗口。

## 验收标准

最小验收：

- `npm --prefix frontend run build` 通过。
- `go test ./backend ./backend/internal/behavior ./backend/internal/browser ./backend/internal/launchcode ./backend/test/launchcode` 通过。
- 打开实例工作台后能加载实例列表。
- 对一个运行中实例执行打开 URL、刷新、截图均有明确成功或失败状态。
- 批量启动/停止不会导致前端卡死。
- 对一个运行中实例执行激活有明确成功或失败状态。
- 选中多个运行实例执行平铺/主辅排列有明确成功、部分失败或失败状态。
- 后端任务持久化单测通过，页面刷新/应用重启后最近任务可重新读取。
- 虚拟滚动下 200 条以上实例不再全量渲染卡片。

真实浏览器验收：

- 至少 2 个实例同时运行。
- 选中 2 个实例批量打开同一 URL，页面状态可在工作台更新。
- 选中 2 个实例批量截图，卡片预览更新。
- 选中 2 个实例执行平铺/主辅排列，真实外部窗口位置变化；失败时工作台不崩溃。
- 停止其中一个实例，另一个实例任务不受影响。

## 后续切片

1. 多屏/最小化增强：支持指定屏幕、最小化其他窗口和恢复布局。
2. 任务队列分页/筛选：当前保存最近 200 条，后续可分页读取和按状态筛选。
3. 预览节流：按可见区域和用户悬停刷新截图。
4. 窗口截图降级：debugPort 未就绪时可选使用窗口截图，但必须标记为降级预览。
## 2026-04-29 自动化验收增强

- 新增本地受控停止接口：`POST /api/instances/stop`，请求体为 `{"profileId":"..."}`，复用 `App.BrowserInstanceStop` 的 CDP 优先关闭路径，失败时返回明确 HTTP 状态，不再依赖脚本强杀浏览器进程。
- 新增本地 workbench HTTP 验收入口：`/api/workbench/navigate`、`/api/workbench/refresh`、`/api/workbench/screenshot`、`/api/workbench/activate`、`/api/workbench/arrange`。这些接口只桥接到既有 `Synchronizer*` 后端方法，不改变 profile、指纹参数、debugPort 或浏览器顶层窗口身份。
- 新增脚本：`scripts/verify-workbench-two-instances.ps1`。默认启动 `build/bin/personal-pilot.exe`，挑选 2 个未运行 profile，执行启动、导航、刷新、截图、真实窗口排列、停止第一个后验证第二个仍可控、最后停止两个实例并关闭脚本启动的 App。
- 安全边界：脚本默认不停止运行前已存在的实例；如确需使用已运行实例，必须显式传入 `-ProfileIds` 并加 `-StopPreExisting`。
- 已验证命令：`powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-workbench-two-instances.ps1` 通过，实际启动并停止 2 个指纹 Chromium 实例，截图长度分别为 42847 和 21483，窗口排列返回 2 条 placement。
