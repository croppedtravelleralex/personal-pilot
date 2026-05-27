# Personal Pilot — Email API Exposure & Automation Design

> `internal/email` 模块已从内部调用推进到部分 REST API 暴露。本文记录当前已落地能力与剩余自动化注册流水线缺口。
> Part of the "缺口补全" initiative (CAPTCHA + SMS + Email API exposure).

---

## 一、现状

### 已有实现 (`backend/internal/email/`)

| 文件 | 功能 | 行数 |
|------|------|------|
| `cloudflare_temp.go` | Cloudflare Worker 临时邮箱客户端 + mail.tm fallback | 605 |
| `mailtm.go` | mail.tm 直接 API 客户端 | 337 |
| `names.go` | 人类化邮箱名/密码生成器 | 116 |
| `credstore.go` | AES-GCM 加密凭证存储 | 134 |
| `provider.go` | Provider 接口抽象 | 已新增 |
| `service.go` | EmailService + session store | 已新增 |
| `email_test.go` | 综合测试套 | 1046 |

### 当前能力

- ✅ 创建临时邮箱 (Cloudflare Worker / mail.tm 双通道)
- ✅ 轮询收件箱 (指数退避, 默认 5min 超时)
- ✅ 智能验证码提取 (中英文 regex, 4-8 位数字)
- ✅ 邮件过滤 (发件人后缀/主题包含/验证码正则)
- ✅ HTML 清洗 + 多优先级正则匹配
- ✅ 人类化名称 (firstname.lastnameNN@domain)
- ✅ 凭证加密存储
- ✅ REST API 部分暴露：创建/查询/释放 inbox、等待验证码
- ⚠️ 邮箱会话表 `email_sessions` 已由 `EmailService` 初始化；当前持久化失败只记录日志并继续返回，可靠性仍需加固

### 缺口

- ⚠️ **REST API 未完全覆盖** — 邮件列表、单封邮件详情、配置读写、统计仍未落地
- ❌ **无通用自动化注册流水线** — 邮箱创建→验证码轮询→自动填充 未作为通用服务暴露
- ⚠️ **持久化邮箱会话部分落地** — session store 已有，保存失败目前是 best-effort 日志，不会阻止创建响应；仍缺完整消息缓存/复用策略
- ⚠️ **多平台适配器接口已落地** — 当前 provider 抽象已存在，仍需更多 provider 实现与配置面

---

## 二、架构设计

```
                    ┌──────────────────────────────┐
                    │    Email Service              │
                    │  (internal/email/)            │
                    │  已有实现 + REST API 层        │
                    └──────┬───────────────┬───────┘
                           │               │
              ┌────────────┼───────────┐   │
              ▼            ▼           ▼   ▼
     ┌────────────┐ ┌──────────┐ ┌────────────────┐
     │Cloudflare  │ │ mail.tm  │ │  预留: 其他     │
     │ Worker API │ │ REST API │ │  临时邮箱服务    │
     └────────────┘ └──────────┘ └────────────────┘
                           │
                           ▼
               ┌──────────────────────┐
               │  CDP 集成层          │
               │  自动填入表单         │
               │  轮询验证码           │
               │  完成注册/验证        │
               └──────────────────────┘
```

---

## 三、新增代码

### 3.1 目录/文件新增

```
backend/internal/email/
├── service.go              # [DONE] Email Service (统一入口 + 会话管理)
├── provider.go             # [DONE] Provider 接口抽象

backend/internal/launchcode/
├── email_api.go            # [DONE] REST API handler (inbox + wait-code 核心端点)
```

### 3.2 Provider 接口 (解耦具体服务)

```go
// provider.go — 新增
type Provider interface {
    Name() string
    CreateAddress(ctx context.Context) (*Address, error)
    WaitForCode(ctx context.Context, filter *MailFilter, timeout time.Duration) (string, error)
    WaitForMail(ctx context.Context, filter *MailFilter, timeout time.Duration) (*MailMessage, error)
    FetchMails(ctx context.Context, limit, offset int) ([]*MailMessage, error)
}

type Address struct {
    Email    string
    Password string    // 部分服务需要密码
    Provider string
    Token    string
    ExpiresAt time.Time
}

type MailFilter struct {
    FromSuffix     string
    SubjectContains string
    CodePattern    string    // 可选: 自定义验证码正则
}
```

### 3.3 Email Service (统一入口)

```go
// service.go — 新增
type EmailService struct {
    primary    Provider
    fallback   Provider
    store      *SessionStore   // 邮箱会话持久化；当前 CreateInbox 保存失败为 best-effort 日志
    credStore  *CredStore      // 已有凭证存储
}

func NewEmailService(config *Config) *EmailService

// CreateInbox 创建临时收件箱 (primary → fallback)
func (s *EmailService) CreateInbox(ctx context.Context) (*Session, error)

// WaitForCode 等待验证码 (自动识别邮箱来源)
func (s *EmailService) WaitForCode(ctx context.Context, sessionID string, timeout time.Duration) (string, error)

// ListInboxes 列出当前活跃的收件箱
func (s *EmailService) ListInboxes(ctx context.Context) ([]*Session, error)

// ReleaseInbox 释放/丢弃收件箱
func (s *EmailService) ReleaseInbox(ctx context.Context, sessionID string) error

// GetCodeFromMail 从指定邮件提取验证码
func (s *EmailService) GetCodeFromMail(ctx context.Context, sessionID, mailID string) (string, error)

type Session struct {
    ID          string
    Email       string
    Provider    string
    Status      string      // active / released / expired
    Messages    int
    CreatedAt   time.Time
    ExpiresAt   time.Time
}
```

### 3.4 Session Store (邮箱会话持久化)

```go
// service.go
type SessionStore struct {
    db *sql.DB
}

func (s *SessionStore) Save(session *Session) error
func (s *SessionStore) Get(id string) (*Session, error)
func (s *SessionStore) List() ([]*Session, error)
func (s *SessionStore) UpdateStatus(id, status string) error
func (s *SessionStore) Delete(id string) error
```

---

## 四、REST API 设计

### 4.1 端点一览

| 端点 | 方法 | 功能 | 优先级 |
|------|------|------|--------|
| `/api/email/inbox` | POST | 创建临时收件箱 | 已落地 |
| `/api/email/inbox/{id}` | GET | 收件箱详情 | 已落地 |
| `/api/email/inbox/{id}` | DELETE | 释放/删除收件箱 | 已落地 |
| `/api/email/inbox/{id}/wait-code` | POST | 等待验证码(阻塞轮询) | 已落地 |
| `/api/email/inbox/{id}/mails` | GET | 获取收件箱邮件列表 | 未落地 |
| `/api/email/inbox/{id}/mails/{mailId}` | GET | 获取单封邮件详情 | 未落地 |
| `/api/email/config` | GET | 邮件服务配置 | 未落地 |
| `/api/email/config` | PUT | 更新邮件服务配置 | 未落地 |
| `/api/email/stats` | GET | 使用统计 | 未落地 |

### 4.2 请求/响应体

```json
// POST /api/email/inbox
{}   // 或 { "humanName": true, "domain": "custom.domain.com" }
→ {
  "ok": true,
  "data": {
    "id": "inbox-abc123",
    "email": "james.smith84@deltajohnsons.com",
    "provider": "mailtm",
    "status": "active",
    "createdAt": "2026-05-22T08:00:00Z",
    "expiresAt": "2026-05-22T13:00:00Z"
  }
}

// GET /api/email/inbox/inbox-abc123
→ {
  "ok": true,
  "data": {
    "id": "inbox-abc123",
    "email": "james.smith84@deltajohnsons.com",
    "status": "active",
    "messages": [
      {
        "id": "mail-001",
        "from": "noreply@deepseek.com",
        "subject": "Your verification code",
        "receivedAt": "2026-05-22T08:02:30Z",
        "hasCode": true
      }
    ]
  }
}

// POST /api/email/inbox/inbox-abc123/wait-code
{
  "fromSuffix": "deepseek.com",
  "subjectContains": "verification",
  "timeout": 300
}
→ {
  "ok": true,
  "data": {
    "code": "873291",
    "fromMailId": "mail-001",
    "elapsedMs": 12345
  }
}
```

---

## 五、自动化注册流水线

### 5.1 通用注册流程 (与 CAPTCHA/SMS 协同)

```
┌─────────────────────────────────────────────────────────┐
│                 Account Registration Pipeline            │
├─────────────────────────────────────────────────────────┤
│ 1. 创建临时邮箱 (POST /api/email/inbox)                  │
│ 2. 购买通知网关号码 (POST /api/sms/number)  // 如果需要      │
│ 3. CDP: 导航到注册页                                     │
│ 4. CDP: 填入邮箱 + 手机号                               │
│ 5. CDP: 处理 Turnstile/reCAPTCHA (human verification handler)       │
│ 6. CDP: 点击发送验证码                                   │
│ 7. 并行等待:                                            │
│    ├─ Email 验证码 (POST /api/email/inbox/{id}/wait-code)│
│    └─ SMS 验证码 (POLL /api/sms/number/{id}/status)     │
│ 8. CDP: 填入验证码                                      │
│ 9. CDP: 提交注册                                        │
│ 10. 保存凭证 (POST /api/system/credentials)             │
│ 11. 释放资源 (邮箱 + 号码)                               │
└─────────────────────────────────────────────────────────┘
```

### 5.2 API 级注册端点

为简化调用方使用，新增高级注册端点：

| 端点 | 方法 | 功能 | 优先级 |
|------|------|------|--------|
| `/api/registration/start` | POST | 启动注册流程(返回阶段状态) | P2 |
| `/api/registration/{id}/status` | GET | 注册流程状态查询 | P2 |

```json
// POST /api/registration/start
{
  "target": "deepseek.com",
  "profileId": "profile-xxx",
  "useEmail": true,
  "useSms": false,
  "humanize": true
}
→ {
  "ok": true,
  "data": {
    "id": "reg-001",
    "status": "in_progress",
    "currentStep": "creating_email",
    "steps": [
      {"name": "creating_email", "status": "running"},
      {"name": "navigating", "status": "pending"},
      {"name": "filling_form", "status": "pending"},
      {"name": "captcha", "status": "pending"},
      {"name": "sending_code", "status": "pending"},
      {"name": "waiting_code", "status": "pending"},
      {"name": "submitting", "status": "pending"},
      {"name": "saving_credentials", "status": "pending"}
    ]
  }
}
```

---

## 六、数据库变更

```sql
-- 邮箱会话表
CREATE TABLE IF NOT EXISTS email_sessions (
    id              TEXT PRIMARY KEY,
    email           TEXT NOT NULL,
    provider        TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'active',
    password        TEXT,
    token           TEXT,
    message_count   INTEGER DEFAULT 0,
    profile_id      TEXT,
    task_id         TEXT,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at      DATETIME,
    released_at     DATETIME
);

-- 邮件缓存表
CREATE TABLE IF NOT EXISTS email_messages (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL REFERENCES email_sessions(id),
    from_addr       TEXT NOT NULL,
    subject         TEXT,
    text_body       TEXT,
    html_body       TEXT,
    has_code        INTEGER DEFAULT 0,
    extracted_code  TEXT,
    received_at     DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 注册流程记录表 (可选)
CREATE TABLE IF NOT EXISTS registration_tasks (
    id              TEXT PRIMARY KEY,
    target_site     TEXT NOT NULL,
    profile_id      TEXT,
    email_session_id TEXT,
    sms_order_id    TEXT,
    status          TEXT NOT NULL,
    current_step    TEXT,
    result          TEXT,           -- success / failed
    error_message   TEXT,
    credential_id   TEXT,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    completed_at    DATETIME
);
```

---

## 七、配置项

```yaml
# config.yaml 新增
email:
  primary_provider: "cloudflare_worker"         # cloudflare_worker / mailtm
  cloudflare_api_base: "https://temp-email-api.chihuolingrang.de5.net"
  human_names: true                              # 使用人类化名称
  poll_interval: 3s                              # 轮询间隔
  poll_timeout: 300s                             # 轮询超时(5min)
  session_ttl: 24h                               # 邮箱会话保留时间
  auto_release: true                             # 用完自动释放
```

---

## 八、实施计划

| 阶段 | 内容 | 工时 | 优先级 |
|------|------|------|--------|
| **Phase 1** | Provider 接口提取 + EmailService + Session 持久化 | 2d | P1 |
| **Phase 2** | REST API: 7 个端点 (inbox CRUD + wait-code) | 1d | P1 |
| **Phase 3** | 注册流水线端点 + CDP 自动填入集成 | 2d | P2 |
| **Phase 4** | 统计看板 + 过期清理 + 多配置支持 | 1d | P2 |

**总计: ~6d** (其中核心 3d 即可上线 API)

---

## 九、与现有代码的关系

| 现有代码 | 关系 | 改动 |
|----------|------|------|
| `cloudflare_temp.go` | 保留 | 实现 Provider 接口 |
| `mailtm.go` | 保留 | 实现 Provider 接口 |
| `names.go` | 保留 | 保持 |
| `credstore.go` | 保留 | EmailService 内部调用 |
| `email_test.go` | 保留 + 新增 | 增加 Service/API 测试 |
| `app_deepseek_register.go` | 保留 | 后续可迁移到通用注册流水线 |
