# Personal Pilot — 自动化原语与集成缺口

> 首次全量盘点浏览器自动化基础原语缺失 + 外部集成缺口 + UX/管理缺口。
> 配合 docs/20-captcha-solving-design.md, docs/20-sms-verification-design.md, docs/20-email-api-exposure.md 一起阅读；这三项当前是“后端/API 部分落地，自动化闭环待验证”，不是全缺。

---

## 一、总览

本次审计在已部分落地的 CAPTCHA/SMS/Email handler/route 与服务代码边界之外，新发现 **10 项缺口**，涵盖:

| 类别 | 数量 | 优先级分布 |
|------|------|-----------|
| 浏览器自动化原语缺失 | 7 | P0: 3, P1: 3, P2: 1 |
| 外部集成缺口 | 1 | P2: 1 |
| UX/管理缺口 | 2 | P1: 1, P2: 1 |

---

## 二、浏览器自动化原语 (P0-P2)

### 2.1 🔴 文件下载处理 (P0)

**当前状态:** 完全未实现。

**场景:** 自动化中高频场景 (导出报表、下载附件、保存凭证)。缺失阻塞大量 real task。

**CDP 能力:** `Browser.setDownloadBehavior` + `Page.downloadProgress` 事件。

**设计方案:**

```
┌─────────────────────────────────────────────────────┐
│                File Download Manager                 │
│               internal/browser/download.go           │
├─────────────────────────────────────────────────────┤
│                                                      │
│  1. SetDownloadBehavior (behavior: allow,            │
│     downloadPath: "...")                             │
│                                                      │
│  2. 监听 Page.downloadProgress → 跟踪进度            │
│                                                      │
│  3. 下载完成 → 校验文件 (大小/类型/MD5)              │
│                                                      │
│  4. 事件: browser:file:downloaded                     │
│     payload: {profileId, url, path, size, mimeType}  │
│                                                      │
│  5. 超时处理: downloadTimeout 后取消 + 清理           │
└─────────────────────────────────────────────────────┘
```

**API 端点:**

| 端点 | 方法 | 功能 | 优先级 |
|------|------|------|--------|
| `/api/workbench/download/config` | POST | 配置下载行为 (路径/超时) | P0 |
| `/api/workbench/download/list` | GET | 已下载文件列表 | P0 |
| `/api/workbench/download/{id}` | GET | 下载详情 + 文件路径 | P1 |

**工作量:** 中 (2-3 天)

---

### 2.2 🔴 Alert/Prompt/Confirm 弹框处理 (P0)

**当前状态:** 完全未实现。

**场景:** 页面 `alert()`, `confirm()`, `prompt()` 让自动化流程断裂。无法自动确认/取消。

**CDP 能力:** `Page.javascriptDialogOpening` 事件 + `Page.handleJavaScriptDialog` 命令。

**设计方案:**

```go
type DialogHandler struct {
    policy DialogPolicy // accept / dismiss / prompt_text
}

func (h *DialogHandler) Handle(ctx context.Context, event *DialogEvent) error {
    switch h.policy {
    case DialogAccept:
        return handleDialog(ctx, true, "")
    case DialogDismiss:
        return handleDialog(ctx, false, "")
    case DialogPromptText:
        return handleDialog(ctx, true, h.promptText)
    }
}
```

**自动化规则预置:**

| 规则名 | 触发事件 | 动作 |
|--------|----------|------|
| `dialog-auto-accept` | `page:javascriptDialogOpening` | 自动确认 |
| `dialog-auto-dismiss` | `page:javascriptDialogOpening` | 自动取消 |
| `dialog-fill-prompt` | `page:javascriptDialogOpening` | 填入预设文本 |

**工作量:** 低 (1 天)

---

### 2.3 🔴 页面等待策略 (P0)

**当前状态:** 只有基础的 `wait` (固定超时)，无智能等待。

**场景:** 回放不确定性 — "等 3 秒" 在网络慢时不够，快时浪费 3 秒。

**缺失原语:**
- `waitForSelector(selector, timeout)` — 等待元素出现
- `waitForNavigation()` — 等待页面加载完成
- `waitForNetworkIdle(timeout)` — 等待网络请求空闲
- `waitForFunction(js, timeout)` — 等待自定义 JS 条件
- `waitForTimeout(ms)` — 固定等待 (兜底)

**设计方案:**

```go
type WaitStrategy string
const (
    WaitSelector    WaitStrategy = "selector"
    WaitNavigation  WaitStrategy = "navigation"
    WaitNetworkIdle WaitStrategy = "network_idle"
    WaitFunction    WaitStrategy = "function"
    WaitTimeout     WaitStrategy = "timeout"
)

type WaitAction struct {
    Strategy WaitStrategy
    Selector string         // waitForSelector 用
    JS       string         // waitForFunction 用
    Timeout  time.Duration  // 超时 (默认 30s)
}
```

**工作量:** 中 (2-3 天)

---

### 2.4 🟠 iframe/Shadow DOM 穿透 (P1)

**当前状态:** 完全未实现。

**场景:** 大量 SPA 使用 iframe 嵌入内容 (支付 iframe、客服聊天、OAuth 登录窗)。无此能力自动化覆盖率受限。

**CDP 能力:** `DOM.getFrameOwner`, `DOM.querySelector` 支持 `nodeId` 参数穿透。

**设计方案:**

```go
type FrameSelector struct {
    Frames []string  // 从顶级 frame 到目标 frame 的路径
    // 示例: ["#main-iframe", "#payment-iframe"]
    // 空 = 顶级 frame
}

func (e *CDPExecutor) FindInFrame(ctx context.Context, frameSelector *FrameSelector, css string) (*Element, error) {
    // 1. 根据 frameSelector 路径找到目标 executionContextId
    // 2. 在该上下文中执行 querySelector
}
```

**工作量:** 中 (2-3 天)

---

### 2.5 🟠 多 Tab/弹出窗口协调 (P1)

**当前状态:** 完全未实现。

**场景:** OAuth 登录在新 Tab 打开、支付跳转新窗口、链接 `target="_blank"`。无法处理弹出窗口。

**CDP 能力:** `Target.createTarget`, `Target.attachToTarget`, `Target.closeTarget`, `Page.windowOpen` 事件。

**设计方案:**

```go
type TabManager struct {
    targets map[string]*TargetInfo // targetId → info
}

func (m *TabManager) OnNewTab(ctx context.Context, callback func(target *TargetInfo)) {
    // 监听 Target.targetCreated 事件
    // 自动切换到新 tab 执行操作
    // 完成后关闭 tab 回到主 tab
}

// 动作序列:
// 1. 点击 "使用 Google 登录" → 新 tab 弹出
// 2. TabManager 自动切换到新 tab
// 3. 填写账号密码
// 4. 关闭 tab 回到原始页
```

**工作量:** 中 (3 天)

---

### 2.6 🟠 Select 下拉框处理 (P1)

**当前状态:** 完全未实现。

**场景:** 注册/表单页面必用 select 下拉框 (国家选择、生日、选项)。无此能力在表单自动化时失败。

**CDP 能力:** `DOM.querySelector` 定位 `select` 元素 + `Runtime.evaluate` 执行 `element.value = X` 并触发 change 事件。

**设计方案:**

```go
type SelectAction struct {
    Selector string
    By       SelectBy  // value / label / index
    Value    string
}

func (e *CDPExecutor) ExecuteSelect(ctx context.Context, action *SelectAction) error {
    js := fmt.Sprintf(`
        (() => {
            const el = document.querySelector('%s');
            if (!el) throw new Error('select element not found');
            el.value = '%s';
            el.dispatchEvent(new Event('change', { bubbles: true }));
        })()
    `, escapeJS(action.Selector), action.Value)
    _, err := e.EvaluateJS(js)
    return err
}
```

**工作量:** 低 (0.5 天)

---

### 2.7 🟡 文件上传 (input[type=file]) (P2)

**当前状态:** 完全未实现。

**场景:** 上传身份证明、头像、附件等。缺失阻塞身份验证流程。

**CDP 能力:** `DOM.setFileInputFiles` 可以直接设置文件路径到 input[type=file] 元素。

**设计方案:**

```go
type FileUploadAction struct {
    Selector string   // input[type=file] 的 CSS 选择器
    Files    []string // 本地文件路径列表
}

func (e *CDPExecutor) ExecuteFileUpload(ctx context.Context, action *FileUploadAction) error {
    // 1. 查找 input[type=file] 元素
    // 2. 调用 DOM.setFileInputFiles 设置文件
    // 3. 等待上传完成
}
```

**工作量:** 低 (0.5 天)

---

## 三、外部集成缺口 (P2)

### 3.1 Notion/Google Sheets/Airtable 导出 (P2)

**当前状态:** 完全未实现。

**场景:** 数据采集结果 (爬虫/监控/报表) 无法导出到协作平台。

**设计方案:**
- 适配器模式: `Exporter` 接口 → NotionExporter / SheetsExporter / AirtableExporter
- 复用 `/api/scraper/task` 和 `/api/scraper/export` 端点
- 新增配置端点: `POST /api/integration/notion` / `POST /api/integration/sheets`

```go
type Exporter interface {
    Name() string
    Export(ctx context.Context, data []Record, config *ExportConfig) error
}
```

**工作量:** 低 (每个适配器 0.5-1 天)

---

## 四、UX/管理缺口

### 4.1 🟠 使用配额 & 速率限制 per-profile (P1)

**当前状态:** 无 per-profile 限制。

**场景:** 自动化行为异常 (死循环/频率过高) 导致目标站封禁或 API 滥用。

**设计方案:**

```go
type ProfileQuota struct {
    ProfileID       string
    RequestsPerMin  int
    RequestsPerHour int
    ConcurrentMax   int
    CooldownAfter   int  // 连续失败 N 次后冷却 N 秒
}

type RateLimiter struct {
    store map[string]*tokenbucket.Bucket
    mu    sync.Mutex
}

func (rl *RateLimiter) Allow(profileID string) bool {
    // per-profile token bucket
    // 超出配额 → 发射事件 + 暂停任务
}
```

**已有基础设施:** 项目已有全局 rate limiter (`backend/internal/launchcode/ratelimit.go`)，扩展为 per-profile。

**新增 API:**

| 端点 | 方法 | 功能 | 优先级 |
|------|------|------|--------|
| `/api/profiles/{id}/quota` | GET | 查看 profile 配额 | P1 |
| `/api/profiles/{id}/quota` | PUT | 更新 profile 配额 | P1 |
| `/api/profiles/{id}/quota/usage` | GET | 当前使用量 | P1 |

**工作量:** 中 (2 天)

---

### 4.2 🟡 历史失败模式数据库 (P2)

**当前状态:** 无系统性失败聚合。

**场景:** 每次排查问题时从零开始，没有"这个配置昨天在这个平台失败了 5 次"的历史视图。

**设计方案:**

```sql
CREATE TABLE IF NOT EXISTS failure_patterns (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id    TEXT NOT NULL,
    target_site   TEXT NOT NULL,
    failure_type  TEXT NOT NULL,  -- captcha / timeout / login_failed / proxy_banned
    error_message TEXT,
    proxy_id      TEXT,
    fingerprint_id TEXT,
    count         INTEGER DEFAULT 1,
    first_seen    DATETIME,
    last_seen     DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 更新模式: 相同 (profile_id, target_site, failure_type) 的 count++ 并更新 last_seen
INSERT INTO failure_patterns (profile_id, target_site, failure_type, first_seen)
VALUES (?, ?, ?, CURRENT_TIMESTAMP)
ON CONFLICT(profile_id, target_site, failure_type)
DO UPDATE SET count = count + 1, last_seen = CURRENT_TIMESTAMP;
```

**API 端点:**

| 端点 | 方法 | 功能 | 优先级 |
|------|------|------|--------|
| `/api/analytics/failures` | GET | 失败模式聚合 | P2 |
| `/api/analytics/failures/{profileId}` | GET | 特定配置的失败历史 | P2 |
| `/api/analytics/failures/sites` | GET | 各站点失败率排名 | P2 |

**工作量:** 中 (2 天)

---

## 五、优先级速览

| 优先级 | 项目 | 工时 | 是否已在主 TODO |
|--------|------|------|----------------|
| **P0** | 文件下载处理 | 2-3d | ❌ 全新 |
| **P0** | Alert/Prompt 弹框 | 1d | ❌ 全新 |
| **P0** | 页面等待策略 | 2-3d | ❌ 全新 |
| **P1** | iframe/Shadow DOM | 2-3d | ❌ 全新 |
| **P1** | 多 Tab 协调 | 3d | ❌ 全新 |
| **P1** | Select 下拉框 | 0.5d | ❌ 全新 |
| **P1** | 使用配额 & 限流 | 2d | ⚠️ 部分 (全局限流已有) |
| **P2** | 文件上传 | 0.5d | ❌ 全新 |
| **P2** | 外部平台导出 | 1-2d | ❌ 全新 |
| **P2** | 失败模式数据库 | 2d | ❌ 全新 |
| | **总计** | **16-20d** | |
