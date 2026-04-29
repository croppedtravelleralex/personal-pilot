# 应用内多实例真实浏览器窗口工作台技术指导与验收标准

最后更新：2026-04-29

## 1. 决策摘要

目标不是把网页放进 Tauri/WebView2，也不是用 headless 投屏模拟浏览器，而是把现有真实浏览器实例的原生窗口托管进应用主窗口。

推荐决策：

- 短期：当前 Wails/Go 项目只用于 HWND 托管 PoC，验证真实浏览器窗口 re-parent 后的指纹稳定性和交互可靠性。
- 中期：PoC 通过后迁移到 Tauri 2 + Vite + React + TypeScript。
- 长期：形成单窗口实例工作台，支持多实例标签切换、当前真实浏览器显示区、API 任务调度、状态监控和降级模式。

不推荐决策：

- 不把 `--headless` + CDP screencast 作为最终内嵌方案。
- 不用 Tauri/WebView2 替代目标浏览器内核。
- 不做多个内嵌 WebView 伪装多个指纹浏览器。

## 2. 目标和边界

### 2.1 核心目标

- 指纹稳定优先：浏览器内核、profile、代理、启动参数和核心环境尽量不因视觉内嵌而变化。
- 实现可靠优先：启动、attach、detach、切换、resize、focus、关闭和异常恢复可重复执行。
- 视觉上单窗口：用户在一个应用窗口内切换多个真实浏览器实例。
- API 操作统一：无论实例是否正在显示，都能通过授权本地 API/CDP 执行任务。

### 2.2 非目标

- 不做绕过平台风控或检测的指导。
- 不支持未授权账号、未授权数据或未授权自动化。
- 不承诺“零指纹变化”。窗口托管可能天然改变窗口尺寸、焦点、可见性和系统弹窗行为。
- 第一阶段不做多浏览器同时分屏托管。先做一个浏览器显示区，多实例标签切换。

## 3. 当前仓库事实

### 3.1 当前技术栈

- 当前仓库是 Wails v2 + Go + Vite + React + TypeScript。
- 前端位于 `frontend/`，后端主要位于 `backend/`。
- 目标基线要求是 Windows 11 only、Tauri 2 + Vite + React + TypeScript。

### 3.2 可复用能力

- `backend/app_instance.go` 已有实例启动链路：`user-data-dir`、`remote-debugging-port`、代理、指纹参数、PID/debugPort 状态。
- `backend/internal/browser/` 已有 profile、core、proxy、fingerprint consistency 等领域能力。
- `backend/internal/behavior/` 已有 CDP 录制、回放和脚本注入相关能力。
- `backend/internal/launchcode/` 已有本地 Launch API 基础。

### 3.3 不应继续扩展的能力

- `backend/app_embedded.go` 当前 `EmbeddedBrowserStart` 使用 `--headless` 和 `Page.startScreencast`。
- `frontend/src/modules/browser/pages/EmbeddedBrowserPage.tsx` 当前用 canvas 绘制 screencast frame。
- 这条链路可保留为“预览/实验”，但不能作为最终工作台方案。

## 4. 目标架构

```text
Tauri 主窗口
  src/pages/workbench
    只负责页面布局和交互编排
  src/features/workbench
    实例状态、标签切换、任务状态、selector
  src/services/desktop.ts
    唯一 Tauri invoke 出口，导出 typed functions
  src-tauri/src
    commands
    native/window_host
    browser/instance
    cdp/task_runner
    metrics/baseline
真实 Chromium/fingerprint-chromium 实例
  独立 user-data-dir
  独立代理
  独立 debugPort
  独立 PID/HWND
```

强制调用链：

```text
pages/components -> features/hooks/store -> services/desktop.ts -> tauri commands -> native service -> Win32/CDP/browser process
```

禁止反向依赖：

- `services` 不得 import `pages`、`components`、`features`。
- 页面组件不得直接调用 Tauri `invoke`。
- 页面组件不得直接处理文件系统、进程控制、窗口句柄或命令执行。

## 5. 核心模块设计

### 5.1 前端模块

建议目录：

```text
src/
  pages/
    WorkbenchPage.tsx
  features/
    workbench/
      store.ts
      hooks.ts
      types.ts
      selectors.ts
      components/
        InstanceTabs.tsx
        InstanceList.tsx
        BrowserHostPane.tsx
        ApiActionPanel.tsx
        TaskFeed.tsx
  services/
    desktop.ts
```

页面职责：

- `WorkbenchPage` 只组合布局，不承载业务规则。
- `BrowserHostPane` 只上报宿主区域的坐标和尺寸，不直接操作 HWND。
- `InstanceTabs` 触发切换意图，不直接 attach/detach。
- `ApiActionPanel` 触发任务，不直接拼 CDP 请求。

状态职责：

- 运行实例列表、当前 active instance、attach 状态、任务状态放入 `features/workbench/store.ts`。
- 大列表使用 selector 读取，避免全页重渲染。
- 搜索和筛选默认 250-400ms debounce。
- 异步请求必须使用 request id 或 abort 策略防 stale-result。

### 5.2 `src/services/desktop.ts`

这是唯一 native 调用出口。建议暴露 typed functions：

```ts
export type BrowserInstanceId = string

export interface BrowserInstanceSummary {
  profileId: BrowserInstanceId
  name: string
  running: boolean
  pid: number
  debugPort: number
  debugReady: boolean
  attached: boolean
  attachState: 'detached' | 'attaching' | 'attached' | 'detaching' | 'failed'
  lastError?: string
}

export interface HostRect {
  x: number
  y: number
  width: number
  height: number
  scaleFactor: number
}

export interface AttachBrowserRequest {
  profileId: BrowserInstanceId
  hostRect: HostRect
}

export interface AttachBrowserResult {
  profileId: BrowserInstanceId
  hwnd: string
  attached: boolean
}
```

必需函数：

- `listBrowserInstances()`
- `startBrowserInstance(profileId)`
- `stopBrowserInstance(profileId)`
- `attachBrowserWindow(request)`
- `detachBrowserWindow(profileId)`
- `switchActiveBrowser(profileId, hostRect)`
- `resizeAttachedBrowser(profileId, hostRect)`
- `focusAttachedBrowser(profileId)`
- `runBrowserTask(profileId, task)`
- `getBrowserHostHealth(profileId)`

错误规范：

```ts
export interface DesktopError {
  code: string
  message: string
  recoverable: boolean
  detail?: unknown
}
```

所有 native 错误必须归一化为稳定 `code`，禁止 UI 依赖原始系统错误字符串。

### 5.3 Tauri/Rust native 模块

建议目录：

```text
src-tauri/src/
  commands/
    browser.rs
    window_host.rs
    task.rs
  native/
    window_host.rs
    process.rs
    win32.rs
  browser/
    instance.rs
    launch_args.rs
    profile.rs
  cdp/
    client.rs
    task_runner.rs
  metrics/
    baseline.rs
```

Rust 只写 native-only 能力：

- 启动/停止浏览器进程。
- 分配和检测 debug port。
- 查找 PID 对应的 top-level browser HWND。
- attach/detach/resize/focus 原生窗口。
- 本地文件、配置、数据库和系统集成。
- CDP 本地连接和任务调度。

不应把大量 UI 业务规则放进 Rust。

## 6. 真实浏览器 HWND 托管方案

### 6.1 基本流程

```text
start profile
  -> launch real browser with existing args
  -> wait debugPort stable
  -> find browser top-level HWND by PID
  -> save original parent/style/placement
  -> SetParent(browserHwnd, hostHwnd)
  -> remove WS_POPUP, add WS_CHILD
  -> SetWindowPos to host rect
  -> show/focus
```

切换实例：

```text
current attached instance
  -> optionally hide or detach
target running instance
  -> attach to same host area
  -> resize to current host rect
  -> focus
```

关闭或退出：

```text
detach if attached
  -> restore original parent/style if available
  -> stop browser process only when user requested stop
  -> app quit must choose: quit app only / quit app and browsers
```

### 6.2 HWND 查找规则

查找过程应满足：

- 通过 PID 定位窗口，不能只靠标题。
- 使用 Windows top-level window 枚举。
- 用 `GetWindowThreadProcessId` 匹配 PID。
- 过滤不可见、尺寸为 0、owner window、tool window。
- Chromium 主窗口通常可出现 `Chrome_WidgetWin_1` 类名，但类名只能作为辅助，不作为唯一条件。
- 找不到窗口时可以短轮询，但必须有超时和明确错误。

### 6.3 样式和父子关系

attach 时需要：

- 保存原 parent、style、exStyle 和 placement。
- 设置父窗口为应用宿主窗口。
- 将目标窗口样式调整为 child window 形态。
- resize 到宿主区域。
- 避免改变浏览器启动参数来解决显示问题，除非该参数已纳入指纹对比验收。

detach 时需要：

- 恢复原 parent/style/exStyle/placement。
- 失败时记录 recoverable error，并允许用户切换到外部窗口模式。

### 6.4 Host 区域选择

React `div` 本身不能直接容纳外部原生窗口。推荐做法：

- 前端测量显示区域在主窗口客户区内的 `x/y/width/height/scaleFactor`。
- 通过 `desktop.ts` 调用 native service。
- native service 将真实浏览器窗口定位到该区域。
- 窗口 resize、侧栏展开、DPI 变化时重新发送 host rect。

### 6.5 降级模式

必须保留：

- 外部窗口模式：真实浏览器仍以普通窗口运行。
- 预览模式：可以使用截图或 screencast 作为只读预览。
- 重新 attach：托管失败后允许用户重新托管当前实例。

## 7. 浏览器启动约束

### 7.1 必须保持

- 每个实例独立 `user-data-dir`。
- 每个实例独立 debug port。
- 每个实例独立代理。
- 每个实例独立指纹参数。
- 已存在的 profile 数据和 cookie 不得跨实例共享。

### 7.2 禁止为最终工作台添加

- `--headless`
- 用 WebView2 加载目标站点。
- 共享 `user-data-dir`。
- 会覆盖用户 profile 配置的 `--remote-debugging-port`、`--user-data-dir`、`--proxy-server` 参数。
- 未经验收的窗口尺寸、DPI、GPU、WebGL、语言、时区相关启动参数。

### 7.3 可接受但必须登记

- `--new-window`：用于确保可见窗口，但需保持外部窗口和托管窗口两种模式一致。
- 固定初始 `--window-size`：会影响 viewport/screen 类结果，必须纳入指纹对比记录。

## 8. API 操作和任务调度

API 控制不应依赖当前实例是否可见。任务调度按 profile/debugPort/CDP 连接执行。

任务类型：

- 打开 URL。
- 查询 tabs。
- 执行授权脚本。
- 截图。
- 行为录制/回放。
- 获取页面标题/URL/状态。

调度规则：

- 每个 profile 默认串行执行会改变页面状态的任务。
- 查询类任务可并发，但必须限制并发数。
- 每个任务有 timeout、cancel、retry policy 和 structured error。
- 日志分页或增量加载，禁止一次性把大日志塞进全局 store。
- 任务结果绑定 request id，避免旧结果覆盖新状态。

## 9. 指纹稳定性验收

### 9.1 验收原则

验收目的不是规避检测，而是确认软件托管窗口没有意外改变用户已配置的浏览器环境。

对比对象：

- A：同一 profile 以普通外部窗口启动。
- B：同一 profile 以真实 HWND 托管方式显示。

测试条件：

- 同一浏览器内核。
- 同一 profile。
- 同一代理。
- 同一启动参数。
- 同一屏幕、DPI、系统语言、系统时区。
- 同一测试 URL 或内部测试脚本。

### 9.2 必须采集字段

必须稳定：

- User-Agent。
- Client Hints。
- 语言和 locale。
- 时区。
- WebGL vendor/renderer。
- Canvas hash。
- Audio hash。
- 字体集合摘要。
- cookie/localStorage/sessionStorage 是否为同一 profile 预期状态。
- WebRTC 策略行为。
- 代理出口和 DNS/连接路径预期结果。

允许登记为差异：

- `innerWidth/innerHeight`。
- `outerWidth/outerHeight`。
- `screenX/screenY`。
- focus/visibility。
- 与宿主窗口尺寸、DPI、显示器位置直接相关的字段。

### 9.3 通过标准

PoC 阶段通过标准：

- 核心稳定字段在 5 次冷启动、20 次 attach/detach、100 次实例切换后保持一致。
- 允许差异字段必须能解释为窗口托管导致，并写入验收记录。
- 不得出现 profile 混用、cookie 串号、代理串号、debugPort 串号。
- 浏览器崩溃或托管失败时可以恢复到外部窗口模式。

产品阶段通过标准：

- 10 个实例运行时，任意两个实例之间切换 200 次无状态串扰。
- 关闭应用但保留浏览器、关闭应用并关闭浏览器，两种路径都可控。
- 托管失败、浏览器退出、debugPort 断开、DPI 变化、窗口最大化/还原都有明确 UI 状态和恢复路径。

## 10. 交互验收

### 10.1 基础交互

- 鼠标点击、滚轮、拖拽、文本选择正常。
- 键盘输入、快捷键、复制粘贴正常。
- 中文输入法候选窗位置可接受。
- 浏览器右键菜单、地址栏、标签栏行为正常。
- 页面弹窗、权限弹窗、下载提示、文件选择框行为可预测。

### 10.2 切换体验

- 点击实例标签后 300ms 内开始反馈切换状态。
- 已运行实例切换不应重新启动浏览器。
- 切换时不应重置当前页面、tab 或滚动位置。
- 当前 attach 失败时不影响其他实例运行。

### 10.3 工作台 UI

- 实例列表超过 200 条必须虚拟滚动。
- 搜索默认 debounce 250-400ms。
- 当前实例状态、attach 状态、debugReady、任务状态必须可见。
- 错误提示必须说明“可重试/需外部窗口/需重启实例/需检查配置”。

## 11. 性能和资源验收

性能只能用 release 构建验收，不能用 dev 模式指标做结论。

应用基线：

- 冷启动 <= 2.0s。
- 应用空闲 RSS <= 220MB，不含外部浏览器实例和代理桥接进程。
- 应用自身进程数 <= 4，不含用户主动启动的浏览器实例和代理桥接进程。

工作台基线：

- 单实例 attach <= 1.5s。
- 已运行实例切换可见反馈 <= 300ms。
- host resize 不造成明显闪烁或持续 CPU 飙升。
- 10 个运行实例的状态刷新不造成主界面卡顿。

资源记录必须区分：

- 应用进程。
- 浏览器实例进程树。
- 代理桥接进程。
- 任务/CDP 连接。

## 12. 质量门槛

Tauri 目标完成前必须通过：

```powershell
npm run build
cargo test
cargo build --release
powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot <project-root>
```

当前 Wails PoC 阶段至少通过：

```powershell
go test ./...
Set-Location frontend; npm run build
```

完成声明必须包含：

- 结构合规结果。
- 类型检查结果。
- release build 结果。
- 指纹对比结果。
- attach/detach/switch 压测结果。
- 已知风险和降级方案。

## 13. 停止线

出现以下任一情况，不得继续产品化，必须回到方案评估：

- 托管后核心指纹字段发生不可解释变化。
- profile、cookie、代理或 debugPort 出现串号。
- 关闭应用后高概率残留不可控浏览器窗口或进程。
- 输入法、焦点、文件选择、权限弹窗无法达到可用标准。
- 为了托管窗口必须添加会改变浏览器环境的启动参数。
- Tauri 迁移导致长期引入 Go sidecar 或额外常驻后端进程，且没有明确预算豁免。

## 14. 分阶段实施切片

### Slice 1：当前 Wails 上的 HWND 托管 PoC

- 目标：验证真实窗口托管可行性。
- 范围：只做单显示区、2 个实例、attach/detach/switch/resize/focus。
- 禁止：不改现有 profile 数据模型，不扩展 headless screencast，不做多分屏。
- 验收：通过 PoC 指纹、交互和切换标准。

### Slice 2：Tauri 2 最小壳层

- 目标：建立符合基线的最终应用框架。
- 范围：`src/`、`src-tauri/`、`desktop.ts`、一个工作台页面、一个 native command。
- 禁止：不迁移全部业务，不做大而全重写。
- 验收：enforce 脚本、typecheck、release build 通过。

### Slice 3：实例管理迁移

- 目标：迁移启动、停止、状态查询和 debugPort 管理。
- 范围：profile 读取、启动参数、进程管理、错误归一化。
- 禁止：不改变 profile 隔离语义。
- 验收：现有实例能按原参数启动并被工作台识别。

### Slice 4：工作台产品化

- 目标：完成用户可用的多实例单窗口切换体验。
- 范围：标签、列表、host pane、任务面板、状态和错误恢复。
- 禁止：不做多分屏。
- 验收：10 实例、200 次切换、任务调度和降级模式通过。

### Slice 5：多实例 API 调度增强

- 目标：让用户可对多个已授权实例执行本地 API 操作。
- 范围：任务队列、并发限制、取消、重试、日志分页。
- 禁止：不支持未授权目标或绕过限制的自动化。
- 验收：任务状态可靠、日志不卡 UI、失败可恢复。

## 15. 实现前检查清单

- 是否确认当前任务属于 PoC、迁移、产品化还是验收？
- 是否保留真实浏览器内核，而不是 WebView2/headless？
- 是否确认启动参数不会改变指纹稳定性？
- 是否有外部窗口降级路径？
- 是否所有 native 调用都走统一 service 层？
- 是否有 attach session 状态，用于失败恢复？
- 是否有明确的指纹对比基线？
- 是否使用 release 构建做性能结论？

## 16. 交付验收清单

- 功能：启动、attach、detach、switch、resize、focus、stop 全部可用。
- 隔离：profile、cookie、代理、debugPort 无串号。
- 指纹：核心字段稳定，差异字段已登记。
- 交互：鼠标、键盘、输入法、弹窗、下载、文件选择可用。
- 性能：release 指标达标或有书面豁免和缓解方案。
- 结构：Tauri 目标项目满足全局目录和服务层规则。
- 质量：类型检查、测试、release build、enforce 脚本通过。
- 降级：托管失败可切外部窗口或预览模式。
- 文档：当前状态、路线图、backlog、验收记录已更新。
