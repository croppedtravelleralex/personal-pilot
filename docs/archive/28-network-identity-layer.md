# 网络身份标识层架构设计

> 本文档描述 PersonaPilot 的统一网络身份标识层——整合**浏览器环境参数管理**、**网络出口路由配置**和**跨组件一致性校验**为核心的基础设施。
> 
> 本层负责确保每个浏览器运行时实例具有一致、可重复且符合目标地理/网络特征的**环境标识**。

---

## 0. 术语表

| 技术术语 | 等价含义 |
|---------|---------|
| 环境标识 (Environment Signature) | 浏览器运行时暴露给网站的客户端参数集合（含 navigator/WebGL/Canvas/Audio/字体等） |
| 环境标识族 (Signature Family) | 一组相关环境参数的分类集合，如 `canvas_rendering`、`webgl_gpu` |
| 控制点 (Control Point) | 一个可独立配置的环境参数，如 `navigator.hardwareConcurrency` |
| 第一优先族 (First Family) | 对兼容性影响最大的 50+ 核心控制点，跨所有组件的基线 |
| 消耗预算 (Consumption Budget) | 按性能负荷对控制点分类（L1=热路径/L2=普通/L3=后台） |
| 一致性校验 (Consistency Check) | 验证不同维度参数之间无矛盾（如时区必须匹配代理出口地区） |
| 网络出口路由 (Network Egress) | 将 HTTP 请求通过中间节点转发的传输机制 |
| 路由池 (Routing Pool) | 可用的网络出口节点集合，含健康状态和信用评分 |

---

## 一、层次架构

```
┌──────────────────────────────────────────────────────────────┐
│                    Application Layer                          │
│   Workflow Engine / Plugin System / Schedule Manager         │
└──────────────────────┬───────────────────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────────────────┐
│              Network Identity Layer (NIL)                     │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Pillar 1: Environment Signature Management               │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐  │  │
│  │  │ First Family │ │ Budget       │ │ Consistency      │  │  │
│  │  │ (50+核心参数) │ │ (L1/L2/L3)   │ │ (跨参数交叉校验)   │  │  │
│  │  └──────────────┘ └──────────────┘ └──────────────────┘  │  │
│  │  归属: src/network_identity/fingerprint_*.rs               │  │
│  │        backend/internal/browser/fingerprint_*.go          │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Pillar 2: Network Egress Management                     │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐  │  │
│  │  │ Routing Pool │ │ Health       │ │ Selection &      │  │  │
│  │  │ (节点池管理)  │ │ (健康评分)    │ │ Trust Scoring   │  │  │
│  │  └──────────────┘ └──────────────┘ └──────────────────┘  │  │
│  │  归属: src/network_identity/proxy_*.rs                     │  │
│  │        backend/internal/proxy/*.go                        │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Pillar 3: Cross-Layer Consistency                       │  │
│  │  ┌────────────────┐ ┌──────────────┐ ┌───────────────┐  │  │
│  │  │ Geo Mismatch   │ │ Identity     │ │ Coverage      │  │  │
│  │  │ (地区一致性校验) │ │ Guard (锁定)  │ │ Dashboard     │  │  │
│  │  └────────────────┘ └──────────────┘ └───────────────┘  │  │
│  │  归属: backend/internal/browser/identity_*.go             │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

---

## 二、Pillar 1: 环境标识参数管理

### 2.1 第一优先族 (First Family)

50+ 核心控制点，分 5 个类别。这些参数构成了每个浏览器实例的**基线环境标识**：

| 类别 | 控制点示例 | 参数数量 | 运行时来源 |
|------|-----------|---------|-----------|
| `BROWSER` | userAgent, appVersion, platform, vendor, languages | 12+ | CDP `Page.addScriptToEvaluateOnNewDocument` |
| `OS` | platform, oscpu (Firefox), navigator.oscpu | 8+ | 启动参数 + CDP 注入 |
| `DISPLAY` | screen.width, screen.height, colorDepth, pixelDepth, devicePixelRatio | 10+ | 启动参数 `--window-size` + CDP 注入 |
| `HARDWARE` | hardwareConcurrency, deviceMemory, navigator.gpu | 10+ | 启动参数 `--gpu-vendor-id` + CDP 注入 |
| `LOCALE` | language, languages, timezone, Intl.DateTimeFormat | 10+ | 启动参数 `--timezone-for-testing` + CDP 注入 |

**控制点优先级规则**（`fingerprint_policy.rs`）：

```
L1 (Coherence-Critical): 
   时区/语言/UA 三者必须一致，且匹配路由出口地区
   违反 → 实例被 Identity Guard 拒绝启动

L2 (Stability-Sensitive):
   WebGL vendor/renderer、Canvas 噪声种子、硬件并发数
   必须在实例生命周期内保持稳定

L3 (Non-Binding):
   字体列表、媒体设备枚举
   可在运行时变更（但应保持一致）
```

**选择算法**（`first_family.rs`）：

```
对于每个控制点:
  1. 检查用户配置（Profile Config）
  2. 未配置则从分类模板库选择（Variant Template）
  3. 无模板则按出口 IP 归属地自动推导
  4. 此时仍未确定 → 样本库随机抽样
```

### 2.2 消耗预算分级 (Consumption Budget)

按运行时性能影响分级：

| 预算级别 | 性能负荷 | 包含参数数 | 使用场景 |
|---------|---------|-----------|---------|
| `L1-HotPath` | 低（<5ms） | 26 个 | 每次页面加载都要读取的参数（navigator.*） |
| `L2-Normal` | 中（5-50ms） | 60+ | 按需读取（WebGL、Canvas、Audio） |
| `L3-Background` | 高（>50ms） | 120+ | 首次采集缓存后再使用（字体枚举、媒体设备） |

### 2.3 一致性校验 (Consistency Check)

跨参数交叉校验规则（`fingerprint_consistency.rs`）：

```
校验器矩阵:

  [Timezone] ↔ [Locale.language] ↔ [Accept-Language Header]
    不一致 → 协调器自动修正较弱的项

  [Hardware Concurrency] ↔ [Device Memory] ↔ [GPU Vendor]
    不一致 → 发出配置警告

  [Screen Resolution] ↔ [Window Size] ↔ [Device Pixel Ratio]
    不一致 → 修正 window size 或 viewport

  [WebGL Vendor] ↔ [GPU Driver Version] ↔ [OS Platform]
    全部从同一真实硬件样本选取
```

**严重度等级：**

| 等级 | 含义 | 处理方式 |
|------|------|---------|
| `Ok` | 所有校验通过 | 无操作 |
| `Warning` | 轻度不一致，不影响功能 | 记录日志，建议修正 |
| `Error` | 中度不一致，部分网站可能拒绝 | 阻止实例启动 |
| `Critical` | 严重不一致，几乎必被识别 | 阻止实例启动，要求重新配置 |

---

## 三、Pillar 2: 网络出口路由管理

### 3.1 路由池 (Routing Pool)

管理所有可用网络出口节点：

```rust
// src/network_identity/proxy_growth.rs
pub struct RoutingPoolGrowthPolicy {
    pub min_available_ratio: f64,      // 最小可用比例（默认 0.2）
    pub max_available_ratio: f64,      // 最大可用比例（默认 0.8）
    pub high_concurrency_threshold: u32, // 高并发阈值
}
```

**三种运行模式：**

| 模式 | 环境变量 | 路由池来源 | 用途 |
|------|---------|-----------|------|
| `prod_live` | `PROXY_RUNTIME_MODE_OVERRIDE=prod` | 远程订阅端点 | 生产环境 |
| `demo_public` | `PROXY_RUNTIME_MODE_OVERRIDE=demo` | 公共路由列表 | 演示/测试 |
| `local` | （默认） | 本地配置 | 开发调试 |

### 3.2 路由健康评分 (Health Scoring)

```rust
// src/network_identity/proxy_health.rs
pub struct RoutingHealthScores {
    pub fraud_score: f64,        // 被目标标记为恶意流量的概率 (0-1)
    pub identity_score: f64,     // 出口身份可信度 (0-1) 
    pub privacy_score: f64,      // 匿名保护等级 (0-1)
    pub mail_reputation: f64,    // 邮件声誉 (0-1)
}
```

**健康检测组件：**

| 检测项 | 检测方法 | 影响分数 |
|--------|---------|---------|
| 可达性 | TCP 连接测试 | 一票否决（不可达→分数归零） |
| 匿名等级 | IP 归属地查询 | `elite` +20, `transparent` -50 |
| 延迟 | HTTP 请求 RTT 测量 | <500ms 正常，>3000ms 降级 |
| 欺诈评分 | 第三方 IP 信誉数据库 | 低分值候选 |
| 出口一致性 | DNS 解析 vs 声明地区 | 不匹配立即降级 |
| 邮件声誉 | 反向 DNS/MX 记录检查 | 用于邮箱验证场景 |

### 3.3 路由选择与信用评分 (Selection & Trust Score)

```go
// backend/internal/proxy/selector.go
type SelectionTuning struct {
    StaleAfterSeconds    int     // 健康信息过期时间
    FailurePenalty       float64 // 失败惩罚乘数
    EliteBonus           float64 // 匿名等级加分
    TransparentPenalty   float64 // 透明代理减分
    CountryMatchBonus    float64 // 地区匹配加分
    RecentFailMultiplier float64 // 近期失败额外惩罚
}
```

**选择流程：**

```
1. 过滤出目标国家的可用节点
2. 按信任分数排序（信任分 = 
      匿名等级分 × 0.3 + 
      (1-欺诈评分) × 0.3 + 
      延迟分 × 0.2 + 
      历史成功率 × 0.2)
3. 从 Top 10% 中随机选择（防模式固定）
4. 绑定到目标 profile
```

### 3.4 路由池增长与采集

```rust
// src/network_identity/proxy_harvest.rs (1364行)
```

**采集管道：**

```
外部端点配置 → HTTP 请求 → 响应解析 → 节点提取 → 健康验证 → 加入路由池
     ↓             ↓           ↓           ↓            ↓          ↓
订阅URL      GET请求    base64解码   地址:端口      TCP连接     可用节点
+鉴权                  Clash/YAML  协议类型       HTTP延迟     +健康分数
                      解析                      匿名等级
```

**自动扩容策略：**

```
当 min_available_ratio > 阈值时:
  触发预取 → 从订阅源拉取更多节点
  验证通过 → 加入路由池
  验证失败 → 报告采集异常
  
当 max_available_ratio < 阈值时:
  清理低频节点
  降低预取频率  
```

---

## 四、Pillar 3: 跨层一致性校验

### 4.1 地区一致性校验 (Geo Mismatch)

```go
// backend/internal/browser/geo_mismatch.go
```

检测以下维度是否一致：

```
路由出口IP的归属地
    ↓ 需要一致
TimeZone 设置 (--timezone-for-testing)
    ↓ 需要一致
Accept-Language 请求头
    ↓ 需要一致
navigator.language / navigator.languages
    ↓ 需要一致
Intl.DateTimeFormat().resolvedOptions().timeZone
```

**不一致处理：**

| 场景 | 自动修正策略 |
|------|-------------|
| 路由 IP 更换为不同国家 | ⚠️ 发出警告，建议重新配置时区/语言 |
| 手动修改了时区但路由未变 | ✅ 自动回滚时区到与路由一致 |
| 新建 profile 未配置地区 | ✅ 按路由出口 IP 自动推导 |

### 4.2 身份锁 (Identity Guard)

```go
// backend/internal/browser/identity_guard.go (342行)
```

**功能：**

1. **防止同一 profile 被多次启动** — 基于文件锁 + PID 文件
2. **启动前完整性检查** — 校验所有必填控制点已配置
3. **一致性快照** — 启动时记录所有控制点值，运行时巡检

**锁定流程：**

```
Profile Start Request
    ↓
AcquireProfileStartLock()
    ├── 检查 profile 是否已被其他进程锁定
    ├── 检查锁文件是否过期（崩溃恢复）
    └── 创建新锁 → 返回锁定令牌
    ↓
ResolveCanonicalUserDataDir()
    ├── 确认 --user-data-dir 路径有效
    ├── 校验目录未被其他 profile 占用
    └── 返回规范化路径
    ↓
Profile Identity Binding
    ├── 将当前 profile 的标识绑定到锁
    └── 记录启动时间戳
    ↓
Browser Launch
```

---

## 五、已有资产映射

| 模块 | 已有代码文件 | 行数 | 文档状态 |
|------|------------|------|---------|
| 第一优先族 | `src/network_identity/first_family.rs` | 419 | ❌ 本文档 |
| 消耗预算 | `src/network_identity/fingerprint_consumption.rs` | 435 | ❌ 本文档 |
| 一致性校验 | `src/network_identity/fingerprint_consistency.rs` | ~200 | ❌ 本文档 |
| 策略定义 | `src/network_identity/fingerprint_policy.rs` | 174 | ❌ 本文档 |
| 路由池增长 | `src/network_identity/proxy_growth.rs` | 317 | ❌ 本文档 |
| 路由采集 | `src/network_identity/proxy_harvest.rs` | 1364 | ❌ 本文档 |
| 路由健康 | `src/network_identity/proxy_health.rs` | 740 | ❌ 本文档 |
| 路由选择 | `src/network_identity/proxy_selection.rs` | 753 | ❌ 本文档 |
| 身份锁 | `backend/internal/browser/identity_guard.go` | 342 | ❌ 本文档 |
| 地区一致性 | `backend/internal/browser/geo_mismatch.go` | — | ❌ 本文档 |
| 环境健康 | `backend/internal/browser/fingerprint_health.go` | — | ❌ 本文档 |
| 一致性 Go 实现 | `backend/internal/browser/fingerprint_consistency.go` | 312 | ❌ 本文档 |

**总计覆盖代码：~5,000+ 行**

---

## 六、实施状态

| 子模块 | 当前状态 | 下一步 |
|--------|---------|--------|
| First Family 控制点定义 | 代码已实现 | 无需变更 |
| 消耗预算分级 | 代码已实现 | 无需变更 |
| 一致性校验矩阵 | Partial（cross-field 校验未全实现） | 补全校验器 |
| 路由池管理 | 代码已实现 | 无需变更 |
| 路由健康评分 | 代码已实现 | 无需变更 |
| 路由选择逻辑 | 代码已实现 | 无需变更 |
| 身份锁 | 代码已实现 | 无需变更 |
| 地区一致性 | 代码已实现 | 无需变更 |
| 覆盖度仪表盘 | 待开发 | 增设可视化面板 |
