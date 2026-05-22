# Personal Pilot — SMS Verification (接码) Module Design

> SMS 验证码接收平台集成方案设计。覆盖 5sim / SMSPool / HeroSMS 三大平台，号码生命周期管理，OTP 提取与自动化填充。
> Part of the "缺口补全" initiative (CAPTCHA + SMS + Email API exposure).

---

## 一、现状与缺口

| 维度 | 当前状态 | 缺口 |
|------|----------|------|
| 邮箱验证码 | ✅ 已实现 (`internal/email/`) | 完整可用 |
| 验证码提取 | ✅ 已实现 (smartExtractCode) | 仅限邮件场景 |
| **SMS 接码** | ❌ 无 | **全部需要新建** |
| **号码池管理** | ❌ 无 | 需设计号码生命周期 |
| **OTP 提取** | ⚠️ 部分 | 可复用 email regex，需适配 SMS 场景 |
| **接码平台集成** | ❌ 无 | 5sim/SMSPool/HeroSMS 均未接入 |

---

## 二、整体架构

```
                    ┌──────────────────────────────┐
                    │     SMS Manager               │
                    │  (internal/sms/)              │
                    │  统一接口 + Provider 轮换       │
                    └──────┬───────────────┬───────┘
                           │               │
              ┌────────────┼───────────┐   │
              ▼            ▼           ▼   ▼
     ┌────────────┐ ┌──────────┐ ┌───────────────┐
     │   5sim     │ │ SMSPool  │ │   HeroSMS     │
     │ 最低成本   │ │ 最高成功率│ │ sms-activate  │
     │ $0.008起   │ │ $0.05起  │ │ 兼容替代      │
     └─────┬──────┘ └────┬─────┘ └───────┬───────┘
           │             │               │
           └─────────────┼───────────────┘
                         ▼
               ┌────────────────────┐
               │    CDP 集成层       │
               │ 号码购买→填号→等待→│
               │ 提取OTP→填入→完成  │
               └────────────────────┘
```

### Provider 选用策略

```
主用: 5sim (成本最低, 覆盖 180+ 国家)
 │
 ├── 5sim 余额不足 / 号码售罄
 │   └── SMSPool (成功率最高, non-VoIP)
 │
 ├── 特定服务 5sim 无库存
 │   └── HeroSMS (sms-activate 生态)
 │
 └── 全部不可用
     └── 上报错误 + 人工介入
```

---

## 三、模块设计: `internal/sms/` (Go)

### 3.1 目录结构

```
backend/internal/sms/
├── sms.go                     # 核心接口 + SMS Manager
├── provider_5sim.go           # 5sim.net 适配器
├── provider_smspool.go        # SMSPool 适配器
├── provider_herosms.go        # HeroSMS (sms-activate 兼容) 适配器
├── number_pool.go             # 号码池管理 (预取/缓存/黑名单)
├── otp_extractor.go           # OTP 提取器 (多语言 regex)
├── sms_test.go                # 测试
└── README.md                  # 预留
```

### 3.2 核心接口

```go
package sms

// Provider 是所有接码平台的统一接口
type Provider interface {
    Name() string

    // BuyNumber 购买号码
    BuyNumber(ctx context.Context, req *BuyRequest) (*Number, error)

    // CheckSMS 检查号码收到的 SMS（轮询用）
    CheckSMS(ctx context.Context, orderID string) (*SMSResult, error)

    // Cancel 取消订单/释放号码（退款）
    Cancel(ctx context.Context, orderID string) error

    // Finish 确认收到验证码（标记完成）
    Finish(ctx context.Context, orderID string) error

    // GetBalance 查询余额
    GetBalance(ctx context.Context) (float64, error)

    // GetPrices 查询指定国家和服务的价格
    GetPrices(ctx context.Context, country, service string) (float64, error)
}

type BuyRequest struct {
    Country     string            // 国家代号: usa, china, russia, england...
    Service     string            // 目标服务: google, telegram, whatsapp, facebook...
    Operator    string            // 运营商偏好: virtual8, any 等
    MaxPrice    float64           // 最高接受价格(美元)
}

type Number struct {
    ID          string            // 订单/号码 ID
    Phone       string            // 完整号码 (含国家码)
    PhoneCC     string            // 国家码
    PhoneNum    string            // 去国家码的号码
    Country     string
    Service     string
    Operator    string
    Price       float64
    Status      NumberStatus      // PENDING / RECEIVED / TIMEOUT / CANCELED
    ExpiresAt   time.Time
    CreatedAt   time.Time
}

type NumberStatus string
const (
    StatusPending    NumberStatus = "PENDING"     // 等待 SMS
    StatusReceived   NumberStatus = "RECEIVED"    // 已收到 SMS
    StatusTimeout    NumberStatus = "TIMEOUT"     // 超时未收到
    StatusCanceled   NumberStatus = "CANCELED"    // 已取消
)

type SMSResult struct {
    Status      NumberStatus
    Code        string            // 提取的验证码
    Text        string            // 原始短信文本
    Sender      string            // 发送方
    ReceivedAt  time.Time
}
```

### 3.3 SMS Manager

```go
type Manager struct {
    providers   []Provider          // 按优先级排列
    blacklist   *Blacklist          // 号码黑名单
    metrics     *Metrics            // 成功率/成本追踪
    prewarm     *NumberPool         // 预取号码池
    config      *Config
}

func NewManager(config *Config) *Manager

// AcquireNumber 获取号码 (主 provider → 备 provider)
func (m *Manager) AcquireNumber(ctx context.Context, req *BuyRequest) (*Number, error)

// WaitForCode 轮询等待验证码
func (m *Manager) WaitForCode(ctx context.Context, number *Number, timeout time.Duration) (*SMSResult, error)

// ReleaseNumber 释放号码
func (m *Manager) ReleaseNumber(ctx context.Context, number *Number) error
```

### 3.4 Number Pool (预取管理)

```go
type NumberPool struct {
    lock        sync.Mutex
    pool        map[string][]*Number     // service → []*Number
    provider    Provider
    config      *PoolConfig
}

type PoolConfig struct {
    PrefetchCount   int           // 每服务预取数量 (默认 3)
    PrefetchTimeout time.Duration // 预取号码保留时间 (默认 10min)
    MinBalance      float64       // 余额低于此值停止预取
}

// Prefetch 预取号码到池中
func (p *NumberPool) Prefetch(ctx context.Context, services []string) error

// Pop 从池中取出一个号码
func (p *NumberPool) Pop(service string) *Number

// Release 将号码放回池中或释放
func (p *NumberPool) Release(number *Number)
```

**号码生命周期:**
```
PREWARM ──→ READY ──→ IN_USE ──→ COMPLETED
             │           │
             │           ├── TIMEOUT ──→ CANCEL (退款)
             │           │
             │           └── BANNED ──→ BLACKLIST
             │
             └── EXPIRED ──→ RELEASE
```

---

## 四、各 Provider 实现

### 4.1 5sim

| 项目 | 说明 |
|------|------|
| **Base URL** | `https://5sim.net/v1` |
| **Auth** | `Authorization: Bearer <API_KEY>` |
| **API 风格** | REST JSON |
| **价格** | $0.008 起 (virtual8) |
| **国家** | 180+ |
| **服务** | 1158+ |
| **余额检查** | `GET /user/profile` → `balance` 字段 |

```go
func (p *FiveSimProvider) BuyNumber(ctx context.Context, req *BuyRequest) (*Number, error) {
    url := fmt.Sprintf("%s/v1/user/buy/activation/%s/%s/%s",
        p.baseURL, req.Country, req.Operator, req.Service)
    httpReq, _ := http.NewRequestWithContext(ctx, "GET", url, nil)
    httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

    resp, err := p.client.Do(httpReq)
    if err != nil {
        return nil, fmt.Errorf("5sim buy: %w", err)
    }
    defer resp.Body.Close()

    var result struct {
        ID       int    `json:"id"`
        Phone    string `json:"phone"`
        Price    int    `json:"price"`     // 单位: 分
        Status   string `json:"status"`
        Expires  string `json:"expires"`
    }
    json.NewDecoder(resp.Body).Decode(&result)

    return &Number{
        ID:        strconv.Itoa(result.ID),
        Phone:     result.Phone,
        Price:     float64(result.Price) / 100,
        Status:    parseStatus(result.Status),
        ExpiresAt: parseTime(result.Expires),
    }, nil
}

func (p *FiveSimProvider) CheckSMS(ctx context.Context, orderID string) (*SMSResult, error) {
    url := fmt.Sprintf("%s/v1/user/check/%s", p.baseURL, orderID)
    httpReq, _ := http.NewRequestWithContext(ctx, "GET", url, nil)
    httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

    resp, err := p.client.Do(httpReq)
    // ...
    // 5sim 在 sms[0].code 中直接返回验证码
}
```

### 4.2 SMSPool

| 项目 | 说明 |
|------|------|
| **Base URL** | `https://api.smspool.net` |
| **Auth** | `Authorization: Bearer <API_KEY>` |
| **价格** | $0.02 起 |
| **特点** | non-VoIP 实卡，成功率最高 |
| **退款** | 号码不可用时自动退款 |

```go
func (p *SMSPoolProvider) BuyNumber(ctx context.Context, req *BuyRequest) (*Number, error) {
    url := fmt.Sprintf("%s/sms/order/%s/%s", p.baseURL, req.Service, req.Country)
    httpReq, _ := http.NewRequestWithContext(ctx, "GET", url, nil)
    httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)
    // ...
}
```

### 4.3 HeroSMS (sms-activate 兼容)

| 项目 | 说明 |
|------|------|
| **Base URL** | `https://hero-sms.com/api/stubs/handler_api.php` |
| **Auth** | `api_key` 作为参数 |
| **价格** | $0.01-$0.05 |
| **兼容** | sms-activate.org API 格式 |
| **注意** | 仅加密货币支付 |

```go
func (p *HeroSMSProvider) BuyNumber(ctx context.Context, req *BuyRequest) (*Number, error) {
    // 使用 sms-activate 兼容 API
    // action=getNumber&service=tg&country=12&api_key=xxx
    url := fmt.Sprintf("%s?action=getNumber&service=%s&country=%d&api_key=%s",
        p.baseURL, req.Service, p.countryCode(req.Country), p.apiKey)
    // ...
}
```

---

## 五、OTP 提取器

复用 `internal/email/cloudflare_temp.go` 的 `smartExtractCode()`，补充 SMS 专属模式：

```go
type OTPExtractor struct {
    patterns []*regexp.Regexp
}

func NewOTPExtractor() *OTPExtractor {
    return &OTPExtractor{
        patterns: []*regexp.Regexp{
            // 带上下文: "Your verification code is 123456"
            regexp.MustCompile(`(?i)(?:code|otp|pin|код|验证码|确认码|认証)[^:]*?[:\s]+(\d{4,8})`),
            // 纯数字: "123456"
            regexp.MustCompile(`\b(\d{6})\b`),
            // 4 位: "1234"
            regexp.MustCompile(`\b(\d{4})\b`),
            // 字母数字: "ABC123"
            regexp.MustCompile(`\b([A-Z0-9]{4,8})\b`),
        },
    }
}

func (e *OTPExtractor) Extract(text string) string {
    for _, p := range e.patterns {
        if m := p.FindStringSubmatch(text); len(m) > 1 {
            return m[1]
        }
    }
    return ""
}
```

---

## 六、CDP 集成层

```go
// 在 behavior 包中新增:
type SMSFiller struct {
    smsManager *sms.Manager
}

// FillPhoneNumber 在注册页填入手机号
func (f *SMSFiller) FillPhoneNumber(ctx context.Context, number *sms.Number) error

// WaitAndFillOTP 等待 SMS 并填入验证码
func (f *SMSFiller) WaitAndFillOTP(ctx context.Context, number *sms.Number, timeout time.Duration) error {
    result, err := f.smsManager.WaitForCode(ctx, number, timeout)
    if err != nil {
        return err
    }
    // CDP 查找验证码输入框并填入
    return fillOTPInput(ctx, result.Code)
}
```

---

## 七、REST API 设计

| 端点 | 方法 | 功能 | 优先级 |
|------|------|------|--------|
| `/api/sms/number` | POST | 购买号码 | P1 |
| `/api/sms/number/{id}/status` | GET | 查询号码状态 | P1 |
| `/api/sms/number/{id}/cancel` | POST | 取消/释放号码 | P1 |
| `/api/sms/number/{id}/finish` | POST | 确认完成 | P1 |
| `/api/sms/balance` | GET | 查询接码余额 | P2 |
| `/api/sms/prices` | GET | 查询各服务价格 | P2 |
| `/api/sms/config` | GET | 接码配置 | P2 |
| `/api/sms/config` | PUT | 更新接码配置 | P2 |
| `/api/sms/stats` | GET | 接码统计(成功率/花费) | P2 |

### 请求/响应体

```json
// POST /api/sms/number
{
  "service": "google",
  "country": "usa",
  "operator": "virtual8",
  "maxPrice": 0.05
}
→ {
  "ok": true,
  "data": {
    "id": "11631253",
    "phone": "+15627231715",
    "price": 0.04,
    "status": "PENDING",
    "expiresAt": "2026-05-22T08:28:38Z"
  }
}

// GET /api/sms/number/11631253/status
→ {
  "ok": true,
  "data": {
    "id": "11631253",
    "phone": "+15627231715",
    "status": "RECEIVED",
    "sms": {
      "code": "644794",
      "text": "Your Google verification code is 644794",
      "sender": "Google",
      "receivedAt": "2026-05-22T08:20:38Z"
    }
  }
}

// GET /api/sms/prices?service=google&country=usa
→ {
  "ok": true,
  "data": {
    "5sim": 0.01,
    "smspool": 0.05,
    "herosms": 0.03
  }
}
```

---

## 八、数据库变更

```sql
-- 接码配置表
CREATE TABLE IF NOT EXISTS sms_config (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    primary_provider  TEXT NOT NULL DEFAULT '5sim',
    primary_api_key   TEXT NOT NULL,         -- 加密存储
    fallback_provider TEXT DEFAULT 'smspool',
    fallback_api_key  TEXT,
    min_balance       REAL DEFAULT 2.0,       -- 余额低于此值告警
    poll_interval_sec INTEGER DEFAULT 3,
    poll_timeout_sec  INTEGER DEFAULT 180,
    prefetch_count    INTEGER DEFAULT 3,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 接码号码记录表
CREATE TABLE IF NOT EXISTS sms_orders (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id        TEXT NOT NULL,           -- 平台订单号
    provider        TEXT NOT NULL,
    phone           TEXT NOT NULL,
    country         TEXT NOT NULL,
    service         TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'PENDING',
    price           REAL DEFAULT 0.0,
    sms_code        TEXT,
    sms_text        TEXT,
    profile_id      TEXT,                    -- 关联的配置ID
    task_id         TEXT,                    -- 关联的任务ID
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 接码统计表
CREATE TABLE IF NOT EXISTS sms_stats (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    provider        TEXT NOT NULL,
    country         TEXT NOT NULL,
    service         TEXT NOT NULL,
    success         INTEGER DEFAULT 0,
    failure         INTEGER DEFAULT 0,
    total_cost      REAL DEFAULT 0.0,
    avg_time_sec    REAL DEFAULT 0.0,
    date            DATE NOT NULL
);
```

---

## 九、配置项

```yaml
# config.yaml 新增
sms:
  primary_provider: "5sim"               # 首选接码平台
  primary_api_key: "${FIVESIM_API_KEY}"  # 环境变量注入
  fallback_provider: "smspool"
  fallback_api_key: "${SMSPOOL_API_KEY}"
  min_balance: 2.0                       # 余额告警阈值(USD)
  poll_interval: 3s                      # 轮询间隔
  poll_timeout: 180s                     # 单次等待超时
  prefetch:
    enabled: true
    count: 3                             # 每服务预取数量
    per_service: ["google", "telegram", "whatsapp"]
```

环境变量:

| 变量 | 用途 |
|------|------|
| `FIVESIM_API_KEY` | 5sim API 密钥 |
| `SMSPOOL_API_KEY` | SMSPool API 密钥 |
| `HEROSMS_API_KEY` | HeroSMS API 密钥 |

---

## 十、与现有模块协同

### 注册自动化流程

```
1. 创建邮箱 (internal/email)
       ↓
2. 购买号码 (sms.AcquireNumber)
       ↓
3. CDP: 导航到注册页
       ↓
4. CDP: 填入邮箱 + 手机号 (SMSFiller.FillPhoneNumber)
       ↓
5. CDP: 发送验证码 (点击按钮)
       ↓
6. 等待 SMS (sms.WaitForCode) ← 并行进行
   等待 Email (email.WaitForCode) ←
       ↓
7. CDP: 填入 SMS OTP + Email OTP
       ↓
8. CDP: 提交注册表单
       ↓
9. 释放号码 (sms.ReleaseNumber)
10. 保存凭证 (email/credstore)
```

### 事件集成

| 事件 | 触发时机 | 新动作 |
|------|----------|--------|
| `account:login:verify` | 检测到二次验证 | 自动购买号码/SMS 轮询 |
| `automation:guard:two-factor-detected` | 自动化守卫发现双因素 | 同上 |

---

## 十一、实施计划

| 阶段 | 内容 | 工时 | 优先级 |
|------|------|------|--------|
| **Phase 1** | 核心接口 + 5sim 适配器 + OTP 提取器 | 2d | P1 |
| **Phase 2** | Number Pool 预取 + SMS Manager + 配置API | 2d | P1 |
| **Phase 3** | SMSPool 适配器(兜底) + 统计 | 1d | P2 |
| **Phase 4** | CDP 集成 + 注册自动化流程闭环 | 2d | P2 |
| **Phase 5** | HeroSMS 适配器 + 余额告警 + 看板 | 1d | P2 |

**总计: ~8d** (其中核心 4d 即可上线可用版本)

## 十二、风险与缓解

| 风险 | 影响 | 概率 | 缓解 |
|------|------|------|------|
| 5sim 关键号码缺货 | 无法购买 | 中 | 多 provider 自动切换，支持服务预取 |
| 目标平台识别虚拟号码 | 注册失败 | 高 | 优先 SMSPool (non-VoIP)，提高单次成功率 |
| 接码成本超出预算 | 运营成本 | 中 | 成本追踪告警 + 国家/服务级别限价 |
| 号码黑名单 | 号码被风控 | 高 | 内置黑名单去重 + 失败自动更换号码 |
