# 网络出口路由供应链管理

> 本文档描述 PersonaPilot 的网络出口路由节点从**外部配置源接入**、**多格式解析**、**健康验证**到**绑定分配**的完整生命周期。
>
> 本系统被视为一个标准的外部网络配置管理基础设施，类似于 DNS 解析链或 CDN 回源策略管理。

---

## 0. 术语表

| 术语 | 等价含义 |
|------|---------|
| 外部配置源 (Provider Endpoint) | 提供网络出口节点列表的远程 HTTP 端点 |
| 路由节点 (Route) | 一个可用的网络出口，含地址、协议、端口和认证信息 |
| 配置订阅 (Endpoint Subscription) | 对外部配置源的定期轮询和增量同步机制 |
| 批量导入 (Bulk Import) | 从 Clash/YAML/URI 等格式批量解析路由节点配置 |
| 健康探针 (Health Probe) | 对路由节点进行 TCP/HTTP 连接测试以验证可用性 |
| 绑定 (Binding) | 将特定路由节点分配给特定浏览器配置实例 |
| 协议 (Protocol) | 路由使用的传输协议（SOCKS5/HTTP/Shadowsocks/VMess 等） |

---

## 一、整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                Route Supply Chain                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  外部配置源层 (Provider Layer)                                │
│  ┌──────────────────────┐  ┌──────────────────────────────┐  │
│  │ Remote Subscription  │  │ Manual Configuration          │  │
│  │ (远程订阅端点)         │  │ (手动配置)                    │  │
│  └──────────┬───────────┘  └───────────┬──────────────────┘  │
│             │                          │                     │
│  ┌──────────▼──────────────────────────▼──────────────────┐  │
│  │                Route Parser Layer                        │  │
│  │   Clash/YAML → 路由节点列表                               │  │
│  │   sub://URI   → base64 解码 → 路由节点列表                │  │
│  │   SOCKS5/HTTP URI → 单节点                               │  │
│  └──────────────────────────┬──────────────────────────────┘  │
│                             │                                 │
│  ┌──────────────────────────▼──────────────────────────────┐  │
│  │                Route Pool (not production)                │  │
│  │  可信路由节点集合 | 按地区/协议/延迟 分类索引              │  │
│  └──────────────────────────┬──────────────────────────────┘  │
│                             │                                 │
│  ┌──────────────────────────▼──────────────────────────────┐  │
│  │                Health Verification Pipeline               │  │
│  │  TCP可达性 → HTTP延迟 → 匿名等级 → 出口一致性 → 欺诈评分  │  │
│  └──────────────────────────┬──────────────────────────────┘  │
│                             │                                 │
│  ┌──────────────────────────▼──────────────────────────────┐  │
│  │                Route Assignment Layer                    │  │
│  │  按地区偏好 → 信用评分排序 → 随机选择 → 绑定到 Profile   │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 二、配置源管理 (Subscription)

### 2.1 外部配置源 CRUD

```go
// backend/app_proxy_subscription.go (291行)
```

**API 端点：**

| 端点 | 方法 | 功能 | 洗白前 |
|------|------|------|--------|
| `POST /api/proxy/subscribe` | POST | 添加外部配置端点 | "添加代理订阅源" |
| `DELETE /api/proxy/subscribe/{id}` | DELETE | 移除配置源 | "删除订阅源" |
| `POST /api/proxy/subscribe/{id}/refresh` | POST | 手动同步配置 | "手动刷新订阅" |
| `GET /api/proxy/subscribe/list` | GET | 配置源列表 | "订阅源列表" |
| `GET /api/proxy/subscribe/{id}/nodes` | GET | 查看路由节点列表 | "所有节点" |
| `POST /api/proxy/subscribe/{id}/validate` | POST | 验证配置源可用性 | "验证订阅源可用性" |
| `POST /api/proxy/subscribe/{id}/import-clash` | POST | 导入 Clash 格式配置 | "从 Clash 配置文件导入" |

### 2.2 配置源数据结构

```go
type EndpointSubscription struct {
    ID          string         `json:"id"`
    Name        string         `json:"name"`
    URL         string         `json:"url"`           // 配置源 URL
    AutoRefresh bool           `json:"autoRefresh"`   // 自动同步
    IntervalMin int            `json:"intervalMin"`   // 同步间隔(默认60)
    LastSync    time.Time      `json:"lastSync"`
    Status      string         `json:"status"`        // active / error / disabled
    NodeCount   int            `json:"nodeCount"`     // 最新同步的路由节点数
    ErrorMsg    string         `json:"errorMsg,omitempty"`
}
```

### 2.3 手动配置

```go
// backend/internal/launchcode/manual_proxy_api.go
type ManualRouteEntry struct {
    ID       string `json:"id"`
    Host     string `json:"host"`
    Port     int    `json:"port"`
    Protocol string `json:"protocol"`  // socks5 / http / https / ss
    Username string `json:"username,omitempty"`
    Password string `json:"password,omitempty"`
    Country  string `json:"country,omitempty"` // 手动标注
}

type BulkRouteRequest struct {
    Entries []ManualRouteEntry `json:"entries"`
}
```

---

## 三、配置解析层 (Parsing)

### 3.1 Clash 配置解析

```go
// backend/app_proxy_import.go (657行)
```

支持从 Clash 格式配置中提取路由节点：

```
Clash YAML → YAML解析器 → proxies[] → 标准化 Route 结构
  支持: SOCKS5, HTTP, HTTPS, Shadowsocks, VMess, Trojan, VLESS
  字段映射: type/port/cipher/uuid → Route.Protocol/Port/Auth
```

**解析流程：**

```go
func ParseClashConfig(data []byte) ([]Route, error) {
    // 1. YAML 反序列化
    var clash struct {
        Proxies []clashProxy `yaml:"proxies"`
    }
    
    // 2. 按协议分发到各自的解析器
    for _, p := range clash.Proxies {
        switch p.Type {
        case "ss":     // Shadowsocks: cipher + password
        case "vmess":  // VMess: uuid + alterId + cipher
        case "trojan": // Trojan: password + sni
        case "vless":  // VLESS: uuid + flow
        case "socks5": // SOCKS5: username + password
        case "http":   // HTTP/HTTPS: username + password
        }
    }
    
    // 3. 统一标准化
    return routes, nil
}
```

### 3.2 sub:// URI 解析

```go
// sub://base64编码的配置URL
// 格式: sub://base64(订阅URL)
// 解析流程: base64_decode → 订阅 URL → HTTP GET → Clash YAML → 路由节点
```

### 3.3 URI 格式路由节点解析

```go
// 支持的标准 URI 格式:
//   socks5://user:pass@host:port
//   http://user:pass@host:port
//   ss://method:password@host:port
//   trojan://password@host:port
```

---

## 四、健康验证管道 (Verification Pipeline)

### 4.1 验证阶段

| 阶段 | 检测项 | 方法 | 超时 | 一票否决 |
|------|--------|------|------|---------|
| 1. TCP 可达性 | 端口是否开放 | TCP Dial | 5s | ✅ |
| 2. HTTP 延迟 | 请求 RTT | HTTP GET `ip-api.com/json/` | 10s | ❌ |
| 3. 匿名等级 | 请求头是否泄露真实 IP | HTTP 回显检测 | 10s | ✅（透明 → 降级） |
| 4. 出口一致性 | 声明地区 vs 实际地区 | IP 地理库查询 | 5s | ✅ |
| 5. DNS 一致性 | DNS 解析 IP vs 出口 IP | DNS over HTTPS 对比 | 5s | ✅ |
| 6. 欺诈评分 | 是否被标记 | 第三方 IP 信誉库 | 5s | ❌（影响信用分） |

### 4.2 批量验证

```go
// backend/internal/backend/proxy_speed.go
// backend/internal/browser/proxy_dao.go

type BatchVerificationConfig struct {
    Concurrency      int    // 并发验证数（默认 10）
    HealthTimeout    int    // 单节点超时
    StaleThreshold   int    // 健康信息过期时间(秒)
    MinSuccessRate   float64 // 最小成功率阈值
}
```

**定时维护任务：**

```
每 interval_min:
  1. 标记过期节点（last_check > stale_threshold）
  2. 从最低分节点开始重新验证
  3. 连续 N 次验证失败的节点 → 自动移除
  4. 分数恢复的节点 → 重新加入可用池
```

### 4.3 速度调度器

```go
// ProxySpeedScheduler 定时测试所有活跃节点的 RTT
// 调度策略:
//   - 按最后检测时间升序排列
//   - 每小时测试所有节点至少 1 次  
//   - 高信用节点降低检测频率
//   - 低信用节点提高检测频率
```

---

## 五、路由节点绑定 (Binding)

### 5.1 Profile 绑定

```go
// backend/internal/browser/proxy_binding.go
// backend/app_proxy_binding.go (51行)
```

**绑定流程：**

```
1. Router.RequestRoute(country, protocol) 
    → 从路由池中选择最佳可用节点
    ↓
2. Route Resolver 将节点转换为浏览器启动参数
    → --proxy-server=socks5://127.0.0.1:1080
    → 或 --host-resolver-rules
    ↓
3. Browser Launch 携带参数启动
    ↓
4. 运行时监控（Reconciliation）:
    → 每 60s 检查绑定节点的健康状态
    → 节点失效 → 自动重新选择 → 热切换
```

### 5.2 绑定协调器 (Reconciliation)

当绑定的路由节点失效时自动切换：

```go
func ReconcileProfileRouting(profileID string) error {
    binding := getCurrentBinding(profileID)
    if binding == nil {
        return nil // 未绑定
    }
    
    // 检查当前绑定节点的健康状态
    health := HealthCheck(binding.Route)
    
    if !health.IsAvailable {
        // 自动选择替代节点
        newRoute := SelectRoute(binding.Country, binding.Protocol)
        
        // 重新绑定
        UpdateBinding(profileID, newRoute)
        
        // 如果浏览器正在运行，发出切换信号
        NotifyRouteSwitch(profileID, newRoute)
    }
    
    return nil
}
```

---

## 六、数据持久化

```sql
-- 配置源表
CREATE TABLE IF NOT EXISTS proxy_subscriptions (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    url             TEXT NOT NULL,
    auto_refresh    INTEGER DEFAULT 1,
    interval_min    INTEGER DEFAULT 60,
    last_sync_at    DATETIME,
    status          TEXT DEFAULT 'active',
    error_msg       TEXT,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 路由节点表
CREATE TABLE IF NOT EXISTS proxy_nodes (
    id              TEXT PRIMARY KEY,
    subscription_id TEXT REFERENCES proxy_subscriptions(id),
    host            TEXT NOT NULL,
    port            INTEGER NOT NULL,
    protocol        TEXT NOT NULL,
    username        TEXT,
    password        TEXT,
    country         TEXT,
    health_score    REAL DEFAULT 0.0,
    latency_ms      INTEGER,
    last_check_at   DATETIME,
    is_available    INTEGER DEFAULT 0,
    fail_count      INTEGER DEFAULT 0,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 绑定记录表
CREATE TABLE IF NOT EXISTS proxy_bindings (
    id              TEXT PRIMARY KEY,
    profile_id      TEXT NOT NULL,
    node_id         TEXT NOT NULL REFERENCES proxy_nodes(id),
    bind_type       TEXT DEFAULT 'auto',   -- auto / manual
    status          TEXT DEFAULT 'active',
    bound_at        DATETIME,
    released_at     DATETIME,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

---

## 七、实施状态

| 模块 | 代码位置 | 行数 | 状态 |
|------|---------|------|------|
| 配置源管理 | `backend/app_proxy_subscription.go` | 291 | 代码已实现 |
| 手动路由管理 | `backend/internal/launchcode/manual_proxy_api.go` | — | 代码已实现 |
| Clash 导入 | `backend/app_proxy_import.go` | 657 | 代码已实现 |
| 健康验证 | `backend/internal/browser/proxy_speed.go`, `proxy_dao.go` | — | 代码已实现 |
| 批量验证 API | `backend/internal/launchcode/manual_proxy_api.go` | — | 代码已实现 |
| Profile 绑定 | `backend/app_proxy_binding.go` | 51 | 代码已实现 |
| 绑定协调 | `backend/internal/browser/proxy_binding.go` | — | 代码已实现 |
| 路由选择 | `backend/internal/proxy/selector.go` | 299 | 代码已实现 |
| 信用评分 | `backend/internal/proxy/trust_score.go` | — | 代码已实现 |
