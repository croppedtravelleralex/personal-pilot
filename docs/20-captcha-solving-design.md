# Personal Pilot — CAPTCHA Solving Module Design

> 验证码打码集成方案设计。覆盖视觉 CAPTCHA 识别、第三方打码服务、Turnstile 深度处理。
> Part of the "缺口补全" initiative (CAPTCHA + SMS + Email API exposure).

---

## 一、现状与缺口

| 维度 | 当前状态 | 缺口 |
|------|----------|------|
| Turnstile Token 提取 | ✅ 已实现 (`app_deepseek_register.go`) | 仅限隐藏 token，无交互式点击 |
| 事件检测 | ✅ 已实现 (4 个 captcha 事件) | 无实际的解决动作绑定 |
| 文本验证码提取 | ✅ 已实现 (email 模块 regex) | 仅限邮件场景 |
| **视觉 CAPTCHA 解决** | ❌ 无 | **OCR + 第三方打码服务全缺** |
| **打码服务集成** | ❌ 无 | 2Captcha/Capsolver/Anti-Captcha 均未接入 |
| **reCAPTCHA/hCaptcha/GeeTest** | ❌ 无 | 不识别不解决 |
| **CAPTCHA 难度预测** | ❌ 无 | 仅有文档提及 |
| **打码结果缓存** | ❌ 无 | 重复遇到同图浪费成本 |

---

## 二、整体架构

```
                    ┌──────────────────────────────┐
                    │    CAPTCHA Solver Manager     │
                    │    (internal/captcha/)        │
                    │    统一接口 + 自动降级          │
                    └──────┬───────────────┬───────┘
                           │               │
              ┌────────────┼───────┐       │
              ▼            ▼       ▼       ▼
     ┌────────────┐ ┌──────────┐ ┌────────────────┐
     │  ddddocr   │ │Capsolver│ │   2Captcha     │
     │ (本地OCR)   │ │ (AI)    │ │ (人工+AI混合)   │
     │  免费 0ms  │ │ $0.60/K │ │  $0.50-2.99/K │
     └─────┬──────┘ └────┬─────┘ └───────┬────────┘
           │             │               │
           ▼             ▼               ▼
     ┌────────────────────────────────────────┐
     │         CDP Integration Layer          │
     │  截图→提交→等待→填入 → 事件通知        │
     └────────────────────────────────────────┘
```

### 三层自动降级策略

```
Layer 1 (免费) : ddddocr 本地 OCR → 纯文本验证码
    ↓ 失败 / 非文本
Layer 2 (AI)   : Capsolver API → reCAPTCHA/Turnstile/hCaptcha
    ↓ 失败 / 超时
Layer 3 (兜底) : 2Captcha API → 人工辅助
```

---

## 三、模块设计：`internal/captcha/` (Go)

### 3.1 目录结构

```
backend/internal/captcha/
├── captcha.go             # 核心接口 + 管理器
├── solver_ddddocr.go      # ddddocr 本地 OCR 适配器
├── solver_capsolver.go    # Capsolver API 适配器
├── solver_2captcha.go     # 2Captcha API 适配器
├── solver_anticaptcha.go  # Anti-Captcha API 适配器（可选）
├── solver_turnstile.go    # Turnstile 深度处理（CDP 交互）
├── captcha_test.go        # 单元测试 + mock 服务
└── README.md              # 预留
```

### 3.2 核心接口

```go
package captcha

// Solver 是所有打码实现的统一接口
type Solver interface {
    Name() string
    Solve(ctx context.Context, req *SolveRequest) (*SolveResult, error)
    GetBalance(ctx context.Context) (float64, error)
    GetCost() float64 // 最近一次请求的花费美元
}

type CaptchaType string
const (
    CaptchaImage     CaptchaType = "image"      // 纯文本/数字图片
    CaptchaReCaptcha CaptchaType = "recaptcha"   // reCAPTCHA v2/v3
    CaptchaHCaptcha  CaptchaType = "hcaptcha"
    CaptchaTurnstile CaptchaType = "turnstile"   // Cloudflare Turnstile
    CaptchaGeeTest   CaptchaType = "geetest"
    CaptchaFun       CaptchaType = "funcaptcha"
    CaptchaKey       CaptchaType = "keycaptcha"
)

type SolveRequest struct {
    Type       CaptchaType
    ImageData  []byte            // 图片 CAPTCHA 的 PNG/JPEG 数据
    ImageURL   string            // 或图片 URL
    SiteKey    string            // reCAPTCHA/Turnstile 的 sitekey
    PageURL    string            // 出现验证码的页面 URL
    Proxy      string            // 可选：使用指定代理提交
    UserAgent  string            // 可选：浏览器 UA
    Options    map[string]any    // 各平台特有参数
    Timeout    time.Duration     // 单次求解超时
}

type SolveResult struct {
    Text       string            // 识别结果文本
    Token      string            // reCAPTCHA/Turnstile token
    SolvedAt   time.Time
    Cost       float64           // 本次花费
    Provider   string            // 实际使用的服务商
}
```

### 3.3 CAPTCHA Manager

```go
type Manager struct {
    solvers    []Solver          // 按优先级排列
    cache      *Cache            // 结果缓存（相同图片 hash 直接返回）
    metrics    *Metrics          // 成功率/成本统计
}

func NewManager(config *Config) *Manager

// Solve 自动选择最优 solver，失败自动降级
func (m *Manager) Solve(ctx context.Context, req *SolveRequest) (*SolveResult, error)

// SolveWithPriority 指定优先级层
func (m *Manager) SolveWithPriority(ctx context.Context, req *SolveRequest, layer int) (*SolveResult, error)

// ReportIncorrect 上报错误结果（部分服务商支持退款）
func (m *Manager) ReportIncorrect(id string) error
```

### 3.4 Cache 设计

```go
type Cache struct {
    store    sync.Map            // SHA256(image) → *SolveResult
    ttl      time.Duration       // 默认 60s
    maxSize  int                 // 最大缓存条目
}

func (c *Cache) Get(key string) (*SolveResult, bool)
func (c *Cache) Set(key string, result *SolveResult)
func (c *Cache) Invalidate(key string)
```

**缓存命中条件：** 相同图片 SHA256 hash + 相同 sitekey → 60s 内直接返回
**目的：** 同一批次中重复遇到相同验证码时省成本

---

## 四、各 Solver 实现细节

### 4.1 ddddocr 本地 OCR

| 项目 | 说明 |
|------|------|
| **部署** | 本机运行 Python 进程或 Go 绑定 (`github.com/86maid/ddddocr`) |
| **适用类型** | 纯文本/数字/字母图片验证码 |
| **成本** | 免费 (本地推理) |
| **延迟** | ~100-500ms |
| **准确率** | 简单验证码 ~95%, 复杂验证码 ~60% |
| **限制** | 不支持 reCAPTCHA/Turnstile/hCaptcha |

**Rust 端口用法:** (可选方案)
```go
// 通过 subprocess 调用 Python ddddocr
func (s *DDDDOcrSolver) Solve(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
    if req.Type != CaptchaImage {
        return nil, ErrUnsupportedType
    }
    result, err := runDDDDOcr(req.ImageData)
    return &SolveResult{Text: result, Provider: "ddddocr", Cost: 0}, err
}
```

### 4.2 Capsolver API

| 项目 | 说明 |
|------|------|
| **Base URL** | `https://api.capsolver.com` |
| **Auth** | `"clientKey": "your-api-key"` |
| **适用类型** | ImageToText, ReCaptchaV2/V3, HCaptcha, FunCaptcha, GeeTest, Turnstile |
| **价格** | Turnstile $0.60/1K, reCAPTCHA $1.00/1K, Image $0.80/1K |
| **延迟** | AI solver: 3-8s |
| **SDK** | 无官方 Go SDK，直接 REST+JSON |

```go
// Capsolver 适配器核心
func (s *CapsolverSolver) SolveTask(ctx context.Context, task map[string]any) (*SolveResult, error) {
    // Step 1: createTask
    taskPayload := map[string]any{
        "clientKey": s.apiKey,
        "task":      task,
    }
    body, _ := json.Marshal(taskPayload)
    // POST https://api.capsolver.com/createTask

    // 解析返回的 taskId
    var createResp struct {
        ErrorId int    `json:"errorId"`
        TaskId  string `json:"taskId"`
    }

    // Step 2: 轮询 getTaskResult (指数退避 1s-5s)
    pollPayload := map[string]any{
        "clientKey": s.apiKey,
        "taskId":    createResp.TaskId,
    }
    // POST https://api.capsolver.com/getTaskResult

    var resultResp struct {
        ErrorId int    `json:"errorId"`
        Status  string `json:"status"`  // "ready" / "processing"
        Solution map[string]any `json:"solution"`
    }
}
```

**Capsolver 各任务类型 Payload 参考：**

```json
// reCAPTCHA V2
{
  "type": "ReCaptchaV2Task",
  "websiteURL": "https://example.com",
  "websiteKey": "6Le-wxASAAAAAEHe5B0BB3...",
  "proxy": "http://user:pass@1.2.3.4:8080"
}

// Turnstile
{
  "type": "AntiTurnstileTaskProxyLess",
  "websiteURL": "https://example.com",
  "websiteKey": "0x4AAAAAAAFnTpB..."
}

// ImageToText
{
  "type": "ImageToTextTask",
  "body": "base64_encoded_image_data",
  "module": "common"  // or "queue"
}

// GeeTest
{
  "type": "GeeTestTask",
  "websiteURL": "https://example.com",
  "gt": "gt_key",
  "challenge": "challenge_value"
}
```

### 4.3 2Captcha API (兜底)

| 项目 | 说明 |
|------|------|
| **Base URL** | `https://2captcha.com` |
| **Flow** | `in.php` → 拿 `id` → `res.php?action=get&id=xxx` |
| **适用类型** | 30+ 类型（最全） |
| **价格** | $0.50-$2.99/1K |
| **延迟** | 人工辅助: 10-60s |
| **SDK** | `github.com/2captcha/2captcha-go` |

```go
import "github.com/2captcha/2captcha-go"

client := api2captcha.NewClient("YOUR_API_KEY")

// Normal Text CAPTCHA
cap := api2captcha.Normal{
    File: "/path/to/captcha.jpg",
}

// reCAPTCHA V2
cap := api2captcha.Recaptcha{
    SiteKey: "6Le-wxASAAAAAEHe5B0BB3...",
    Url:     "https://example.com",
}

code, err := client.Solve(cap.ToRequest())
```

### 4.4 Turnstile 深度处理

当前实现 (`app_deepseek_register.go:604-629`) 仅轮询 `[name="cf-turnstile-response"]` 隐藏输入。
对于需要点击的交互式 Turnstile，新设计如下：

```
CDP 流:
1. 检测到 Turnstile iframe
2. 通过 Capsolver/2Captcha 获取 Turnstile token
3. 通过 CDP 设置 document.querySelector('[name="cf-turnstile-response"]').value = token
4. 触发 change 事件
5. 验证表单可提交
```

---

## 五、CDP 集成层

### 5.1 检测与截图流

```go
// 在 behavior/cdp_executor.go 中新增：
type CaptchaDetector struct {
    client *cdp.Client
}

// DetectCaptcha 检测页面上是否存在验证码并返回类型 + 截图
func (d *CaptchaDetector) DetectCaptcha(ctx context.Context) (*CaptchaInfo, error) {
    // 1. 检查 Turnstile iframe
    // 2. 检查 reCAPTCHA iframe
    // 3. 检查 hCaptcha iframe
    // 4. 检查图片验证码元素
    // 5. 截图验证码区域
}

type CaptchaInfo struct {
    Detected   bool
    Type       CaptchaType
    SiteKey    string
    PageURL    string
    Screenshot []byte       // 验证码区域的截图
    BoundingBox *DOMRect    // 验证码元素的位置
}
```

### 5.2 自动填入

```go
// FillCaptcha 将解决结果填入页面
func (d *CaptchaDetector) FillCaptcha(ctx context.Context, info *CaptchaInfo, result *SolveResult) error {
    switch info.Type {
    case CaptchaImage:
        // 找到 input 框填入文本
    case CaptchaReCaptcha, CaptchaTurnstile, CaptchaHCaptcha:
        // 通过 CDP 设置 token 值 + 触发回调
    }
    return nil
}
```

### 5.3 事件集成

已有事件系统无需新增事件，直接对接现有事件：

| 事件 | 触发时机 | 新动作 |
|------|----------|--------|
| `risk:captcha:detected` | 检测到验证码 | → 自动调用 SolverManager.Solve() |
| `risk:captcha:failed` | 解决失败 | → 自动降级下一层 Solver / 切换代理 |
| `automation:guard:captcha-detected` | 自动化守卫发现验证码 | → 同上 |

---

## 六、REST API 设计

### 6.1 CAPTCHA 解决端点

| 端点 | 方法 | 功能 | 优先级 |
|------|------|------|--------|
| `/api/captcha/solve` | POST | 提交验证码图片/base64 并获取结果 | P1 |
| `/api/captcha/solve/token` | POST | 提交 reCAPTCHA/Turnstile 信息获取 token | P1 |
| `/api/captcha/balance` | GET | 查询打码服务余额 | P2 |
| `/api/captcha/config` | GET | 当前打码配置 | P2 |
| `/api/captcha/config` | PUT | 更新打码配置 | P2 |
| `/api/captcha/stats` | GET | 打码统计(成功率/花费/次数) | P2 |
| `/api/captcha/report-incorrect` | POST | 上报错误结果(部分服务退款) | P2 |

### 6.2 请求/响应体

```json
// POST /api/captcha/solve
{
  "type": "image",
  "imageData": "base64_encoded_png",
  "options": {
    "numeric": false,
    "minLength": 4,
    "maxLength": 6,
    "caseSensitive": false
  }
}
→ {
  "ok": true,
  "data": {
    "text": "ABC123",
    "cost": 0.002,
    "provider": "capsolver",
    "elapsedMs": 1234
  }
}

// POST /api/captcha/solve/token
{
  "type": "turnstile",
  "siteKey": "0x4AAAAAAAFnTpB...",
  "pageUrl": "https://example.com/login",
  "proxy": "http://user:pass@1.2.3.4:8080"
}
→ {
  "ok": true,
  "data": {
    "token": "0.XXXXX...",
    "cost": 0.0006,
    "provider": "capsolver"
  }
}
```

### 6.3 自动化集成

新增自动化规则预置模板：

| 规则名 | 触发事件 | 动作 | 说明 |
|--------|----------|------|------|
| `captcha-auto-solve` | `risk:captcha:detected` | 自动解决并填入 | 遇到验证码自动处理 |
| `captcha-retry-fallback` | `risk:captcha:failed` | 降级 solver + 切换代理 | 一次失败自动换方案 |
| `captcha-difficulty-warning` | `risk:captcha:detected` | + 日志告警 | 记录 CAPTCHA 频率 |

---

## 七、数据库变更

```sql
-- 打码配置表
CREATE TABLE IF NOT EXISTS captcha_config (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    provider      TEXT NOT NULL DEFAULT 'capsolver',     -- 首选服务商
    api_key       TEXT NOT NULL,                         -- API 密钥(加密存储)
    fallback      TEXT DEFAULT '2captcha',               -- 降级服务商
    fallback_key  TEXT,                                  -- 降级密钥
    use_local_ocr INTEGER DEFAULT 1,                     -- 启用本地 OCR
    cache_ttl_sec INTEGER DEFAULT 60,                    -- 缓存有效期
    max_retries   INTEGER DEFAULT 3,
    timeout_sec   INTEGER DEFAULT 120,
    created_at    DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at    DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 打码统计表
CREATE TABLE IF NOT EXISTS captcha_stats (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    provider      TEXT NOT NULL,
    captcha_type  TEXT NOT NULL,
    success       INTEGER DEFAULT 0,
    failure       INTEGER DEFAULT 0,
    total_cost    REAL DEFAULT 0.0,
    total_time_ms INTEGER DEFAULT 0,
    date          DATE NOT NULL
);
```

---

## 八、配置项

```yaml
# config.yaml 新增
captcha:
  primary_provider: capsolver          # 首选: capsolver / 2captcha / anticaptcha
  primary_api_key: ${CAPSOLVER_API_KEY}
  fallback_provider: 2captcha          # 降级
  fallback_api_key: ${TWOCAPTCHA_API_KEY}
  use_local_ocr: true                  # 开启本地 ddddocr
  cache_ttl: 60s                       # 结果缓存
  max_retries: 3                       # 单次请求最大重试
  timeout: 120s                        # 总体超时
  # 自动降级阈值
  auto_degrade_after_failures: 2       # 连续失败 N 次自动降级
```

环境变量:

| 变量 | 用途 |
|------|------|
| `CAPSOLVER_API_KEY` | Capsolver API 密钥 |
| `TWOCAPTCHA_API_KEY` | 2Captcha API 密钥 |
| `ANTICAPTCHA_API_KEY` | Anti-Captcha API 密钥 |
| `CAPTCHA_CONFIG_PATH` | 打码配置文件路径 |

---

## 九、集成路径（与自动化引擎协同）

```
注册/登录流程遇到 CAPTCHA:

1. CDP 执行 detect → 发现验证码
   ↓
2. 发射 risk:captcha:detected 事件
   ↓
3. captcha-auto-solve 规则触发
   ↓
4. SolverManager.Solve():
   4a. 检查缓存 → cache hit → 直接返回
   4b. cache miss → ddddocr (如果适用) → 成功? 返回
   4c. Capsolver API → 成功? 缓存+返回
   4d. 2Captcha API → 成功? 缓存+返回
   4e. 全部失败 → 返回 error
   ↓
5. FillCaptcha() → CDP 填入结果
   ↓
6. 发射 captcha:solved 事件
   ↓
7. 自动化流程继续
```

---

## 十、实施计划

| 阶段 | 内容 | 工时 | 优先级 |
|------|------|------|--------|
| **Phase 1** | Capsolver 适配器 + Turnstile 深度处理 + CDP 截图检测 | 3d | P1 |
| **Phase 2** | 2Captcha 适配器(兜底) + 缓存 + 配置API | 2d | P1 |
| **Phase 3** | 自动化规则集成 + 事件绑定 + 管理 API | 2d | P2 |
| **Phase 4** | ddddocr 本地 OCR 集成 + 统计看板 | 2d | P2 |
| **Phase 5** | CAPTCHA 难度预测 + Anti-Captcha 适配器 | 3d | P2 |

**总计: ~12d** (其中核心 5d 即可上线可用版本)

## 十一、风险与缓解

| 风险 | 影响 | 概率 | 缓解 |
|------|------|------|------|
| Capsolver API 在大陆不可用 | 无法使用 AI solver | 中 | 内置 2Captcha 作为 fallback，配置自选代理 |
| 打码成本超出预期 | 运营成本高 | 低 | 缓存 + 本地 OCR 优先，API 成本监控告警 |
| 目标站打码成功率低 | 自动化流程卡住 | 中 | 自动降级+切换代理重试，超时后人工介入事件通知 |
| ddddocr 在 Windows 部署复杂 | 本地 OCR 不可用 | 低 | Python 子进程模式，无 Python 时自动跳过 |
