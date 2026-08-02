# 原生输入适配器设计

> 本文档描述基于 Win32 `SendMessage`/`SendInput` API 的原生输入适配器——允许通过操作系统级消息注入在 Chromium 窗口中产生 `isTrusted: true` 的鼠标和键盘事件。

---

## 0. 术语表
| 术语 | 含义 |
|------|------|
| 原生输入 | 通过操作系统 API 发送的、浏览器无法区分真伪的输入事件 |
| 窗口句柄 (HWND) | Windows 操作系统分配给每个窗口的唯一标识符 |
| 视口坐标 | 浏览器视口内的 CSS 像素坐标 |
| 客户端坐标 | Windows 窗口客户区内的物理像素坐标 |
| DPI 缩放 | 操作系统缩放比例，CSS 像素到物理像素的转换系数 |

## 一、架构

```
CDP Page Actions
    ↓ (Fallback Path)
┌──────────────────────────────────────────┐
│          Native Input Adapter             │
│  (backend/internal/wininput/)            │
│                                          │
│  ┌──────────┐ ┌────────────┐ ┌────────┐ │
│  │ Mouse    │ │ Keyboard   │ │ Coord  │ │
│  │ Sender   │ │ Sender     │ │ Helper │ │
│  └──────────┘ └────────────┘ └────────┘ │
│       ↓              ↓                    │
│  ┌──────────────────────────────────┐    │
│  │     Win32 API (user32.dll)       │    │
│  │  SendMessage / SendInput         │    │
│  └──────────────────────────────────┘    │
└──────────────────────────────────────────┘
```

## 二、坐标转换

```go
// 视口 CSS 坐标 → 客户区物理坐标:
//   clientX = viewportX * dpiScale
//   clientY = toolbarHeight + viewportY * dpiScale
//
// 客户区坐标 → 屏幕坐标:
//   ClientToScreen(hwnd, clientX, clientY)
//
// 窗口查找:
//   EnumWindows → 匹配 PID → 可见顶层窗口
```

## 三、鼠标适配器

```go
// MouseSender 通过 SendMessage 注入鼠标事件
// 产生的 MouseEvent.isTrusted = true（Chromium 无法区分原生 vs 注入）

// MoveTo 带贝塞尔插值的鼠标移动
//   → PostMessage WM_MOUSEMOVE 序列
//   → 每步间隔 8-15ms，模拟真实运动节奏
//
// Click 左键单击
//   → PostMessage WM_LBUTTONDOWN → 等待 30-70ms → WM_LBUTTONUP
//
// RightClick 右键
//   → PostMessage WM_RBUTTONDOWN → WM_RBUTTONUP
//
// ScrollWheel 滚轮
//   → PostMessage WM_MOUSEWHEEL (delta=±120)
```

## 四、键盘适配器

```go
// KeyboardSender 通过 SendMessage 注入键盘事件
// 产生的 KeyboardEvent.isTrusted = true

// SendKey 发送单个按键
//   → PostMessage WM_SETFOCUS → WM_KEYDOWN → WM_CHAR → WM_KEYUP
//   → 带人机化间隔（多指击键节奏）
//
// SendText 逐字符输入文本
//   → 遍历字符 → 每字符 SendKey
//   → 支持退格（WM_KEYDOWN VK_BACK）
```

## 五、代码资产

| 文件 | 职责 | 行数 |
|------|------|------|
| `coord_windows.go` | 坐标转换、窗口查找、几何测量 | 138 |
| `mouse_windows.go` | 鼠标事件注入 (MoveTo/Click/RightClick/ScrollWheel) | 175 |
| `keyboard_windows.go` | 键盘事件注入 (SendKey/SendText) | — |

## 六、使用场景

| 场景 | 推荐方式 | 原因 |
|------|---------|------|
| 纯 CDP 控制 | CDP Executor | 跨平台兼容 |
| 需要极高保真度 | **原生输入适配器** | `isTrusted: true`, 无法被 JS 检测 |
| 混合模式 | CDP 导航 + 原生输入填表 | 导航用 CDP, 表单用原生输入 |
