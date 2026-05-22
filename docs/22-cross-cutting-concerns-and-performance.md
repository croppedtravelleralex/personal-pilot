# Personal Pilot — 横切关注点与性能审计

> 覆盖错误处理、日志、性能、并发安全、可观测性等横切面问题。
> 配合 docs/21-architecture-audit-and-optimization.md 一起阅读。

---

## 一、错误处理

### 1.1 🔴 启动配置加载失败静默降级

`backend/app.go:135-139`:
```go
cfg, err := LoadConfig(a.resolveAppPath("config.yaml"))
if err != nil {
    cfg = config.DefaultConfig()     // 静默使用默认值
}
```

**影响:** config.yaml 损坏或格式错误时，应用正常启动但行为异常，用户无感知。

**修复:**
```go
cfg, err := LoadConfig(a.resolveAppPath("config.yaml"))
if err != nil {
    log.Fatalf("FATAL: failed to load config: %v", err)  // fail-fast
}
```

### 1.2 🔴 Rust Heartbeat 错误静默丢弃

`src/runner/engine.rs:3013` (如 Rust 保留):
```rust
let _ = sqlx::query(...).execute(&state.db).await;
```

**影响:** 心跳写入失败时静默丢弃，任务显示"活着"直到 `reclaim_stale_running_tasks` 回收，浪费 worker 槽位。

**修复:** 至少 log warning，重试一次后标记任务异常退出。

### 1.3 🟠 CDP Executor `default: return nil`

`backend/internal/behavior/cdp_executor.go:602`:
```go
default:
    return nil
```

未识别的操作类型返回 nil，调用方无法区分"操作成功执行"和"操作被静默跳过"。

### 1.4 🟡 `now_ts_string()` 时钟错误返回 "0"

Rust 代码中:
```rust
fn now_ts_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);  // ← 时钟问题返回 "0"
    secs.to_string()
}
```

系统时钟异常时时间戳返回 epoch "0"，导致代理陈旧度计算错误。

---

## 二、日志

### 2.1 🔴 Rust 无结构化日志

Rust 二进制用 `println!` 和 `eprintln!` 输出，而 Go 后端有完善的日志系统:

| 维度 | Go (internal/logger) | Rust (src/) |
|------|---------------------|-------------|
| 级别 | Debug/Info/Warn/Error/Fatal | 无 |
| 格式 | JSON + text, 可配置 | 纯文本 |
| 轮转 | 按大小+时间轮转 | 无 |
| 写入器 | 文件+stdout+内存 buffer | stdout/stderr |
| 拦截器 | 事件总线桥接 | 无 |

**修复 (假设 Rust 保留):** Go 日志系统开 HTTP 端口，Rust 通过 HTTP 写入。

### 2.2 🟠 日志轮转失败时 `fmt.Printf` 兜底

`backend/internal/logger/logger.go:149,176,456`:
```go
fmt.Printf("log rotate error: %v", err)
```

如果 stdout 被重定向到 `/dev/null` (daemon 模式)，这些错误完全消失。

**修复:** 日志轮转失败时应写入 stderr 或触发事件告警。

---

## 三、性能

### 3.1 🔴 Rust N+1 查询 (proxy selection)

`src/runner/engine.rs:1529-1629`:
```
fetch_ranked_auto_selection_candidates()  → 1 次查询获取 16 候选
  └── compute_proxy_selection_explain()   → 每候选 3 次 SQL 查询
      └── provider risk                   → 1 次
      └── provider region cluster         → 1 次
      └── trust score subquery            → 1 次
```

**16 候选 = 1 + 48 = 49 次 SQL 往返。**

**修复:** 将三个子查询 JOIN 进主查询，一次获取全部。

### 3.2 🟠 超大 `Value` 克隆

`src/runner/engine.rs:1804,3549,3638+`:

`payload.clone()` 在 `serde_json::Value` 上频繁调用，payload 包含 `network_policy_json`, `fingerprint_profile_json`, `behavior_policy_json` 等大型嵌套字段。

**影响:** 16 并发 worker × ~500KB payload = 8+ MB 不必要的堆分配/周期。

**修复:** 用 `Arc<Value>` 共享引用，或用 `serde_json::RawValue` 延迟解析。

### 3.3 🟠 代理信任视图同步刷新

`src/runner/engine.rs:2598-2604`:

每次任务执行后同步调用 `refresh_proxy_trust_views_for_scope()`，高吞吐时造成写竞争。

**修复:** 加去抖 (debounce) — 每 N 秒最多刷新一次，用 `tokio::sync::Notify` 触发。

### 3.4 🟡 优先级任务排序无索引

`src/runner/engine.rs:2709-2722`:
```sql
WHERE status = ?
ORDER BY priority DESC, COALESCE(queued_at, created_at) ASC, created_at ASC
```

高频查询路径但可能全表扫描。

**修复:** 加复合索引 `(status, priority, queued_at)`。

---

## 四、并发安全

### 4.1 🟠 Go App 后台 goroutine 无管控

`backend/app.go:783`:
```go
go a.browserMgr.DownloadAndExtractCore(a.ctx, coreName, url, proxyConfig)
```

- 无错误传播回调用方
- 无 Context 取消感知
- 无 panic recovery
- 无并发限制

**修复:** 用 `errgroup` + semaphore:
```go
g, ctx := errgroup.WithContext(a.ctx)
g.SetLimit(3) // 最多 3 并发下载
g.Go(func() error {
    return a.browserMgr.DownloadAndExtractCore(ctx, ...)
})
```

### 4.2 🟠 Lock Order 风险

`backend/app_shutdown.go:47-52`:
```
stopRuntimeServices()
  → a.behaviorEnginesMu.Lock()
      → engine.Stop()
          → (可能) 回调 App 其他方法
              → 获取 bridgeMu → DEADLOCK
```

当前未触发但脆弱。

**修复:** 在 shutdown 路径上统一锁获取顺序: `bridgeMu → behaviorEnginesMu`，添加锁顺序检查工具 (`go-deadlock`)。

### 4.3 🟡 无 `-race` 定期执行

Go 后端无并发安全性测试的 CI 流程。

**修复:** CI 中加 `go test -race ./...` 步骤。

---

## 五、可观测性

### 5.1 🔴 缺少健康检查端点

当前 `/api/health` 只返回 200 OK，无组件级状态。

**需要的健康端点:**
```json
GET /api/health/detailed
{
  "status": "ok",
  "components": {
    "cdp": { "status": "ok", "connected_instances": 5 },
    "database": { "status": "ok", "size_mb": 128 },
    "proxy_pool": { "status": "degraded", "active": 42, "total": 100 },
    "disk": { "status": "ok", "free_gb": 45 },
    "memory": { "status": "warning", "usage_percent": 78 }
  }
}
```

### 5.2 🟠 无 Prometheus 指标

生产运行无法回答以下问题:
- 当前有多少活跃实例?
- 代理池大小趋势?
- API 请求延迟 P50/P95/P99?
- 任务成功率趋势?

**推荐指标 (最少):**
| 指标 | 类型 | 标签 |
|------|------|------|
| `app_instances_active` | Gauge | profile_id, kernel |
| `app_tasks_total` | Counter | status, type |
| `app_tasks_duration_seconds` | Histogram | type |
| `app_proxy_pool_size` | Gauge | status, provider, region |
| `app_api_requests_total` | Counter | endpoint, status |
| `app_api_duration_seconds` | Histogram | endpoint |

### 5.3 🟠 代理凭证明文泄露

**位置:**
- `RunnerProxySelection` 结构体带 `username` + `password` 明文
- 序列化到结果 JSON (`engine.rs:586-656`)
- 写入 `insert_log` 格式字符串 (`engine.rs:525-530`)
- 存储到 `proxy_session_bindings` 表

**修复:**
```go
type ProxyCredentials struct {
    Username string `json:"-"`                    // 绝不序列化
    Password string `json:"-"`                    // 绝不序列化
}

func (c *ProxyCredentials) LogSafe() string {
    if c.Username == "" {
        return "<anonymous>"
    }
    return c.Username[:2] + "***" + c.Username[len(c.Username)-1:]
}
```

DB 层: AES-GCM 加密存储凭证字段。

---

## 六、Go 后台循环治理

### 6.1 无优雅关闭

`tokio::spawn` 的 3 个后台循环 (proxy replenish, harvest, health) 无取消机制。

**Go 端同类问题:** `backend/app.go` 中 `go func()` 启动的后台任务未使用 Context 传播取消信号。

**统一修复:**

```go
type BackgroundLoop struct {
    ctx    context.Context
    cancel context.CancelFunc
    wg     sync.WaitGroup
}

func NewBackgroundLoop(parent context.Context) *BackgroundLoop {
    ctx, cancel := context.WithCancel(parent)
    return &BackgroundLoop{ctx: ctx, cancel: cancel}
}

func (bl *BackgroundLoop) Start(name string, fn func(ctx context.Context) error) {
    bl.wg.Add(1)
    go func() {
        defer bl.wg.Done()
        defer recoverPanic(name)
        if err := fn(bl.ctx); err != nil && err != context.Canceled {
            log.Errorf("background loop %s exited: %v", name, err)
        }
    }()
}

func (bl *BackgroundLoop) Shutdown() {
    bl.cancel()
    bl.wg.Wait() // 等待所有循环退出
}
```

---

## 七、依赖审计

| 依赖 | Go | Rust | 必要性 | 建议 |
|------|----|------|--------|------|
| `metacubex/mihomo` | v1.19.20 | — | ❌ 1 处使用 | **移除** |
| `wailsapp/wails/v2` | v2.11.0 | — | ❌ Tauri 已替代 | **移除** |
| `gorilla/websocket` | v1.5.3 | tokio-tungstenite | ✅ CDP | 保留 |
| `gopkg.in/yaml.v3` | v3.0.1 | — | ✅ config | 保留 |
| `mattn/go-sqlite3` | v1.14.22 | sqlx + rusqlite | ✅ DB | 保留 |
| 前端 (pnpm) | 241 包 | — | ⚠️ | 可 review |

---

## 八、优化优先级矩阵

```
                      影响大 ←—————————→ 影响小
                      ┌──────────────────────┐
          高  ┌────────┼──────────────────────┤
   实       │  P0:   │  N+1 查询             │
   现       │ 配置静默降级 │  代理信任视图同步刷新     │
   难       │ 无优雅关闭 │  超大 Value 克隆         │
   度       │ 凭证明文  │                      │
              │        │                      │
          低  │  P1:   │  P2:                 │
              │ CDP fallthrough             │  crashTimestamps 无上限  │
              │ Heartbeat 静默              │  now_ts_string() 时钟    │
              │ Lock order 风险             │  无 -race 测试           │
              │ 日志轮转兜底                  │  双层 JSON 键名          │
              └────────┴──────────────────────┘
```

---

## 九、实施路线 (与架构优化协同)

```
Phase 0 (当天)
├── CDP Executor default → return error
├── crashTimestamps 加上限
└── 确认 `-race` 可运行

Phase 1 (第 1 周)
├── 配置加载 fail-fast
├── 代理凭证明文脱敏 (log + JSON + DB)
├── Heartbeat 失败告警
└── Lock order 统一 + deadlock 检测工具

Phase 2 (第 2-3 周)
├── 后台循环统一治理 (BackgroundLoop 模式)
├── 健康检查端点增强
├── now_ts_string() 时钟异常检测
└── 优先级任务查询加索引

Phase 3 (第 4-6 周)
├── Rust N+1 查询优化 (如保留 Rust)
├── 超大 Value → Arc<Value>
├── 代理信任视图去抖
└── Prometheus 指标接入
```
