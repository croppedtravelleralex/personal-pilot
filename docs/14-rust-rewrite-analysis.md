# 全 Rust 重写可行性分析

> 分析是否可以将 Personal Pilot 从 Go + Tauri sidecar 架构完全迁移到 Rust。
> 日期: 2026-05-17

---

## 一、当前架构全景

```
[Frontend: Tauri 2 + React 18 + Vite + TypeScript]
    ↕ Tauri IPC (invoke)
[Rust Thin Shell: ~600 lines lib.rs]
    ↕ HTTP/JSON (localhost)
[Go Backend Sidecar: ~57K 生产代码]
    ├── HTTP API 服务 (launchcode) — ~15 文件
    ├── 浏览器管理 (browser) — ~42 文件
    ├── 代理管理 (proxy) — ~22 文件
    ├── 行为引擎 (behavior) — ~21 文件
    ├── 调度器 (scheduler) — 6 文件
    ├── 规则引擎 (automation) — 3 文件
    ├── 临时邮箱 (email) — 5 文件
    ├── 备份加密 (backup) — 4 文件
    ├── 日志系统 (logger) — 12 文件
    ├── 事件系统 (events) — 8 文件
    ├── ... + 其他
    ↓
[外部进程]
    ├── Chromium (exec)
    ├── xray.exe / sing-box.exe
    └── (可选: Clash)
```

### 关键数据点

| 指标 | 值 |
|------|-----|
| Go 生产代码 | ~57,000 行 |
| Go 测试代码 | ~21,000 行 |
| Go 直接依赖 | 11 个 (不含间接) |
| 间接依赖 | ~80+ (主要来自 mihomo) |
| Rust Tauri shell | ~600 行 |
| 外部二进制管理 | xray, sing-box, Chromium |
| 无 CGo | ✅ (modernc/sqlite 纯 Go) |
| Wails 残留 | 3 文件引用，迁移中 |

---

## 二、Go vs Rust 在各模块的性能分析

### 2.1 热路径识别

真正需要极致性能的路径只有以下几条：

| 热路径 | 当前瓶颈 | Rust 能否改善 |
|--------|----------|-------------|
| **CDP WebSocket 通信** | 网络 I/O 绑定，Go 的 goroutine 已足够高效 | 收益微小，tokio 表现类似 |
| **代理测速(并发)** | 当前并发 5 个，HTTP 延迟是主要因素 | 收益微小，瓶颈在网络 |
| **sing-box/xray 桥接管理** | 外部进程启动时间(~200ms)，非 Go 可优化 | 无法优化，等待进程启动 |
| **浏览器启动** | Chromium 启动 1-3s，瓶颈在浏览器自身 | 无法优化 |
| **录制事件处理** | CDP 事件流处理，goroutine 模型已高效 | 收益微小 |
| **YAML/JSON 解析** | 毫秒级操作，非瓶颈 | 收益可忽略 |
| **SQLite 读写** | 单连接序列化，毫秒级 | rusqlite 性能相当 |

**结论：性能瓶颈几乎全在 I/O（网络/进程/磁盘），Rust 无法在这些场景比 Go 有明显优势。**

### 2.2 内存占用对比

| 场景 | Go 当前 | Rust 预期 | 节省 |
|------|---------|----------|------|
| 空载 (无浏览器运行) | ~20-30 MB | ~5-10 MB | 显著 |
| 单浏览器实例 | ~50-80 MB | ~35-60 MB | 中等 |
| 并行 5 个实例 | ~200-300 MB | ~150-250 MB | 中等 |
| 大量代理节点管理(200+) | ~10-20 MB | ~5-10 MB | 中等 |
| 录制事件缓冲 | 内存占用视录制时长 | 同等 | 无差异 |

**Rust 的内存优势主要在空载和大量并发连接场景。** 但此项目的主要内存消耗者实际上是 Chromium 进程（每个约 100-200MB），而非 Go sidecar 本身。

### 2.3 启动时间

| 场景 | Go | Rust | 差异 |
|------|-----|------|------|
| 冷启动(无缓存) | ~50ms | ~5ms | Rust 快 10x |
| 热启动 | ~10ms | ~2ms | 差异可忽略 |
| 完整应用启动(含浏览器) | 3-5s | 3-5s | **瓶颈在外进程，无差异** |

### 2.4 GC 暂停分析

Go 的 GC 在此项目场景中影响：

- **CDP WebSocket 路径**：毫秒级响应，GC 暂停 < 1ms，无感知
- **代理测速/健康检查**：秒级操作，GC 完全无影响
- **录制事件批处理**：批量处理，GC 暂停不可见
- **HTTP API 请求**：请求-响应模式，GC 在请求间隙完成

**结论：GC 不是此项目的性能瓶颈。** 项目中没有低延迟高频交易类的场景。最接近实时的是 CDP 事件监听，但 CDP 本身是异步推送，100μs 的 GC 暂停对用户完全不可感知。

---

## 三、mihomo 依赖分析（最大的替换障碍）

### 3.1 当前用途

`metacubex/mihomo`（Clash Meta 核心）只在一个地方使用：

**`backend/internal/proxy/speedtest.go:unifiedDelayTest()`**

```go
// 1. mihomo 解析代理配置
proxyInstance, err := adapter.ParseProxy(mapping)

// 2. 通过代理拨号建立 TCP 连接
conn, err := px.DialContext(ctx, &addr)

// 3. 在连接上发送 HTTP 请求测速
req, _ := http.NewRequestWithContext(ctx, http.MethodHead, testURL, nil)
resp, err := client.Do(req)
```

**核心价值**：`adapter.ParseProxy` 能解析 **vmess/vless/trojan/ss** 等协议的配置，不依赖外部进程就能建立代理连接。这避免了为测速启动 xray/sing-box 进程的开销。

### 3.2 替代方案评估

| 方案 | 工作量 | 代码量 | 协议覆盖 |
|------|--------|--------|----------|
| **直接调用 xray/sing-box 进程测速**（不走 mihomo） | 0 — 已有 fallback 路径 | 已有 | 全部 |
| **用 Rust 实现各协议拨号** | **极高** — 每个协议都需要实现/TLS/WS/mux | 5000+ 行 | 残缺 |
| **保留 Go mihomo，其余用 Rust** | 低 — Rust ↔ Go 通过子进程通信 | ~200 行胶水 | 全部 |

**推荐**：方案 1。`httpClientDelayTest()` 已经是 mihomo 路径的 fallback，且已经过生产验证。去掉 mihomo 后降级为：
- socks5/http 代理 → 直接 HTTP 测速
- vmess/vless/trojan/ss → 启动 xray/sing-box 桥接后再测速
- 额外代价：测速需等桥接就绪（~200ms），但对于秒级测速操作可忽略

### 3.3 mihomo 移除收益

| 指标 | 当前 | 移除后 |
|------|------|--------|
| Go 间接依赖数 | ~80+ (mihomo 及其传递依赖) | ~10 |
| 编译时间 | ~2-3 分钟 | ~30 秒 |
| 二进制大小 | ~25 MB (含 mihomo) | ~8 MB |
| 功能影响 | 无 | 测速多 200ms (桥接启动) |

---

## 四、模块级 Rust 迁移可行性

### 4.1 直接可迁移（Low Risk）

| 模块 | Go 行数 | Rust 难度 | Rust 生态支持 |
|------|---------|----------|-------------|
| **HTTP API 服务器** (launchcode) | ~2500 | 低 | **axum/actix-web** 成熟 |
| **SQLite DAO 层** | ~1000 | 低 | **rusqlite** / **sqlx** 成熟 |
| **YAML/JSON 解析** (config/parser) | ~600 | 低 | **serde** + **serde_yaml** |
| **日志系统** (logger) | ~800 | 低 | **tracing** / **log** |
| **事件系统** (events) | ~300 | 低 | 自定义 |
| **调度器** (scheduler) | ~500 | 低 | **tokio-cron-scheduler** |
| **规则引擎** (automation) | ~300 | 低 | 自定义，无复杂依赖 |
| **Webhook 发送** | ~50 | 低 | **reqwest** |
| **系统托盘** (tray) | ~50 | 低 | **tauri** 自带 |
| **系统路径** (apppath/fsutil) | ~100 | 低 | 标准库 |

**小计：~6200 行，可安全迁移，无技术风险。**

### 4.2 中等难度（Medium Risk）

| 模块 | Go 行数 | Rust 难点 | Rust 生态 |
|------|---------|-----------|----------|
| **浏览器管理** (browser) | ~4000 | CDP WebSocket + 进程管理 | **tokio-tungstenite** (WebSocket), 子进程管理用标准库 |
| **CDP 基础操作** (cdp_ops) | ~400 | WebSocket JSON-RPC | 同上 |
| **CDP 执行器** (cdp_executor) | ~1063 | 贝塞尔曲线/鼠标轨迹/键盘事件算法 | 纯算法，无依赖 |
| **浏览器指纹提取** (fingerprint_*) | ~800 | CDP Runtime.evaluate 调用 | 同上 |
| **窗口控制 Win32** (window_control) | ~150 | Win32 API | **windows-rs** 官方 crate |
| **原生输入 Win32** (wininput) | ~560 | SendInput/SendMessage | **windows-rs** + **enigo** |
| **代理 HTTP 客户端** (http_client) | ~140 | SOCKS5 dialer | **tokio-socks** |
| **临时邮箱** (email) | ~400 | REST API 客户端 | **reqwest** |
| **备份加密** (backup) | ~200 | AES-GCM | **aes-gcm** / **orion** |

**小计：~7700 行，有风险但可行，关键是 Rust 生态已成熟。**

### 4.3 高难度 + 低收益（不建议迁移）

| 模块 | Go 行数 | 不迁移原因 |
|------|---------|-----------|
| **行为录制引擎** (recorder.go) | 1000 | CDP 多域事件订阅 + JS 注入逻辑复杂，无 Go 特定问题 |
| **回放引擎** (playback.go) | 864 | 864 行复杂状态机，重写风险回归 ≤ 收益 |
| **人性化中间件** (humanize/) | ~800 | 贝塞尔曲线+打字计划+滚动算法，纯数学 |
| **Xray/sing-box 桥接管理** (xray.go/singbox.go) | ~600 | 纯粹的外部进程管理，Go 和 Rust 完全等价 |

**这些模块可以在 Rust 中用同样方式实现，但没有任何性能优势。** 迁移它们的主要价值是统一技术栈，而非性能。

### 4.4 需要保留为外部进程或 Go 模块的

| 组件 | 原因 | 方案 |
|------|------|------|
| **mihomo 测速** | 80+ 传递依赖，唯一使用处可替换 | 去掉，用 xray/sing-box 桥接 fallback |
| **xray / sing-box** | 外部二进制，跨平台 | 保留为外部进程，Rust 管理生命周期 |
| **Chromium / Lightpanda** | 浏览器核心 | 保留为外部进程 |

---

## 五、性能收益量化估算

### 如果全部用 Rust 重写

| 指标 | Go 当前 | Rust 重写 | 改善幅度 |
|------|---------|-----------|----------|
| **内存空载** | ~25 MB | ~8 MB | -68% |
| **内存满载(5实例)** | ~300 MB | ~280 MB | -7% (Chrome 是主要消耗者) |
| **API 响应时间** | ~5ms | ~3ms | -40% (但从用户角度无感知) |
| **启动时间(后端)** | ~50ms | ~5ms | -90% (但仍然被 Chrome 3s 启动掩盖) |
| **并发测速(100节点)** | ~60s (并发 5) | ~60s (瓶颈在代理延迟) | 0% |
| **二进制体积** | ~25 MB | ~15 MB (含 tokio 等) | -40% |
| **编译时间(增量)** | ~15s | ~30s (Rust 增量编译) | +100% (更慢) |
| **编译时间(全量)** | ~2min | ~10min | +400% (更慢) |

**真实世界可感知的改善只有空载内存和二进制体积。** API 响应从 5ms 降到 3ms 用户不可感知，启动时间从 50ms 降到 5ms 被 Chrome 3s 启动淹没。

### 如果只做混合架构（保留 Go sidecar + Rust 接管）

目前已经是混合架构（Rust Tauri shell + Go sidecar）。维持现状或仅增量迁移部分功能到 Rust 是最务实的选择。

---

## 六、全 Rust 重写成本估算

### 6.1 可行方案 A：渐进式替换（推荐）

**方案**：保持当前 Tauri + Go sidecar 架构，逐步用 Rust 重写 Go 侧功能。

**阶段划分：**

| 阶段 | 内容 | 代码量 | 工期 | 风险 |
|------|------|--------|------|------|
| 1 | 去掉 mihomo 依赖，测速走 fallback | ~100 行改 | 1 天 | 低 |
| 2 | 新建 Rust 核心 crate，实现 HTTP API + SQLite | ~5000 行 | 4 周 | 中 |
| 3 | Rust 直接替换 Go sidecar（不启动 Go 进程） | ~3000 行 | 3 周 | 中高 |
| 4 | 迁移浏览器管理 + CDP 操作 | ~4000 行 | 4 周 | 中 |
| 5 | 迁移窗口控制 + Win32 原生输入 | ~800 行 | 1 周 | 低 |
| 6 | 迁移行为引擎（录制/回放/人类化） | ~3000 行 | 4 周 | 高 |
| 7 | 清理 Go 残留代码 | ~500 行改 | 1 周 | 低 |
| **总计** | | | **~18 周** | |

**总工期：约 4-5 个月（单人全时）。**

### 6.2 方案 B：混合架构（务实推荐）

**方案**：保持 Go sidecar，仅将以下高价值部分用 Rust 实现：
- 保留 Tauri Rust shell（已有）
- 用 Rust 实现新的 APP API 层（替换 launchcode）
- Go sidecar 降级为纯业务引擎（browser/proxy/behavior）
- Rust ↔ Go 通过 stdin/stdout JSON-RPC 通信（现有模式的反向）

**工期：约 6-8 周，风险显著降低。**

### 6.3 方案 C：全量重写（不推荐）

一次性全部用 Rust 重写 57K Go 代码。

**工期：6-9 个月。风险：极高。** 行为引擎的录制/回放逻辑是 3000 行经过多轮修复的复杂状态机，重写必然引入大量回归 bug。

---

## 七、关键 Rust crate 选型

| 需求 | 推荐 crate | 成熟度 |
|------|-----------|--------|
| HTTP 框架 | **axum** | ★★★★★ 生产级 |
| 异步运行时 | **tokio** | ★★★★★ |
| SQLite | **rusqlite** + **sqlx** | ★★★★★ |
| WebSocket | **tokio-tungstenite** | ★★★★★ |
| YAML | **serde_yaml** | ★★★★★ |
| JSON | **serde_json** | ★★★★★ |
| 序列化 | **serde** | ★★★★★ |
| 日志追踪 | **tracing** + **tracing-subscriber** | ★★★★★ |
| 定时任务 | **tokio-cron-scheduler** | ★★★★☆ |
| Win32 API | **windows** (microsoft/windows-rs) | ★★★★★ 官方 |
| 鼠标/键盘模拟 | **enigo** | ★★★★☆ |
| SOCKS5 | **tokio-socks** | ★★★★☆ |
| AES 加密 | **aes-gcm** | ★★★★★ |
| UUID | **uuid** | ★★★★★ |
| CLI 参数 | **clap** | ★★★★★ |
| HTTP 客户端 | **reqwest** | ★★★★★ |
| CDP JSON-RPC | 自实现（200 行，基于 serde+tokio-tungstenite） | - |

### 新增依赖估算

| 范围 | 直接依赖数 | 间接依赖估算 | 编译时间 |
|------|-----------|-------------|----------|
| 最小 Rust 核心（API + DB + 配置） | ~15 | ~100+ | ~5min |
| 完整 Rust 重写 | ~25 | ~200+ | ~10min |
| 当前 Go 项目 | 11 直接 | ~100+ | ~2min (含 mihomo) |

去掉 mihomo 后 Go 间接依赖可降到 ~20，Rust 的依赖树不会比当前 Go 项目更复杂。

---

## 八、最终判断

### 不建议全 Rust 重写的理由

1. **性能收益有限** — 瓶颈在外部进程（Chrome/xray/sing-box）和网络 I/O，不在 Go runtime
2. **Go GC 不是问题** — 项目中没有低延迟场景
3. **57K 代码重写风险极高** — 行为引擎的录制回放经过多轮迭代，重写必然引入回归
4. **编译时间更长** — Rust 全量编译 10min vs Go 2min
5. **团队成本** — 如果只有你一人维护，同时熟悉 Go 和 Rust 的上下文切换成本高

### 推荐的务实做法

```
继续用 Go 完善现有功能（docs/12-module-split-and-backlog.md 中的 P0/P1 项）
  ↓
去掉 mihomo 依赖（降低构建复杂度，间接依赖从 80+ 降到 20）
  ↓
保持 Tauri Rust shell 现有架构（已工作良好）
  ↓
如果未来需要新模块（如 ML 推理/高性能代理检测），用 Rust 写独立子进程
  ↓
Go sidecar 作为稳定的核心引擎继续运行
```

### 唯一值得现在用 Rust 的部分

| 部分 | 原因 |
|------|------|
| **Tauri Rust shell**（已有） | 已用 Rust 写就，继续维护即可 |
| **ML 推理模块**（未来） | Rust + `ort` (ONNX Runtime) 比 Go 更成熟 |
| **高性能代理健康检测**（可选） | 如果需要每秒扫描数百代理端口，Rust + `tokio` + `mio` 有优势 |

**总结：全 Rust 重写性价比低。当前 Go + Tauri 混合架构是合理的。把精力花在补齐功能上比换语言更有价值。**
