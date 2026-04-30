# Tauri 迁移实现说明

## 目标边界

本迁移只替换桌面应用壳层，不改变指纹浏览器启动与运行链路。

- Tauri 2 负责单窗口管理 UI、单实例、窗口关闭事件、启动/停止 Go sidecar。
- Go sidecar 继续运行原 `backend.App`、`LaunchServer`、profile、user-data-dir、fingerprint args、proxy bridge、debugPort、CDP、Win32 外部窗口控制。
- 目标网站仍运行在真实外部 fingerprint Chromium 顶层窗口中，不进入 Tauri WebView/WebView2。

## 关键实现

- `src-tauri/`：Tauri 2 壳层，Rust 只提供薄命令：
  - `core_start`
  - `core_status`
  - `core_rpc`
  - `core_stop`
  - `app_window_hide/show/minimize`
  - `app_quit`
- `backend/cmd/personal-pilot-core/`：Go sidecar 入口，启动原后端并暴露本地 bridge：
  - `/health`
  - `/rpc`
  - `/events`
  - `/shutdown`
- `src/services/desktop.ts`：唯一 Tauri API 入口，集中 `invoke/listen`。
- `src/services/tauriWailsBridge.ts`：兼容旧 Wails 前端绑定，避免一次性重写 100+ 个前端 API 调用。
- `backend/internal/events/event_bridge.go`：事件输出改为可插拔 emitter；Wails 模式仍走 Wails runtime，sidecar 模式走本地事件桥。

## 指纹影响

推荐链路下，网站可见指纹不应变化：

- Chromium 内核目录不变。
- profile/user-data-dir 策略不变。
- fingerprint args 注入仍由 Go 后端完成。
- proxy/debugPort/CDP 仍由 Go 后端完成。
- 浏览器窗口仍是外部真实顶层窗口。

需要记录的变化：

- Chrome 父进程可能从 Wails exe 变为 `personal-pilot-core.exe` 或 Tauri 启动的 sidecar。
- Tauri 壳层启动 sidecar 后，启动时序和焦点轨迹需要真实 profile 回归。

## 验收记录

已通过：

- `go test ./backend/...`
- `npm run build`
- `npm run tauri:build`
- Win11 Tauri baseline enforcement
- `git diff --check`
- sidecar ready/shutdown smoke test

未完全通过：

- `scripts/verify-workbench-two-instances.ps1` 已确认 Tauri release exe 能启动 sidecar 和 LaunchServer，但安全默认验收中止：当前没有至少 2 个非运行 profile。未使用 `-StopPreExisting` 强停已有实例。

## 后续验收

当存在 2 个可安全启动的非运行 profile 后，执行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-workbench-two-instances.ps1 `
  -AppPath "D:\SelfMadeTool\personal-pilot\src-tauri\target\release\personal-pilot-tauri.exe" `
  -ReadyTimeoutSec 90
```

若必须使用正在运行的 profile，显式传入 `-ProfileIds` 并加 `-StopPreExisting`，避免误停用户正在使用的实例。
