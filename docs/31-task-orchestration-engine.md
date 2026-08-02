# 任务编排引擎设计

> 本文档描述 PersonaPilot 的任务编排引擎——整合**定时任务调度器**、**事件驱动规则引擎**和 **CDP 任务执行器** 的统一框架。
> 
> 本系统提供: 基于 cron/interval/event 的工作触发、条件式规则匹配、浏览器任务执行和结果跟踪。

---

## 0. 术语表

| 术语 | 等价含义 |
|------|---------|
| 任务定义 (Task Definition) | 一个可执行的浏览器操作单元，包含操作类型、参数和超时 |
| 调度器 (Scheduler) | 按时间计划（cron/interval/一次性）触发任务执行的组件 |
| 规则引擎 (Rule Engine) | 根据事件条件匹配并触发预定义操作的判定系统 |
| CDP 任务执行器 (CDP Task Runner) | 通过 Chrome DevTools Protocol 执行浏览器任务的组件 |
| 事件总线 (Event Bus) | 系统组件间的异步消息传递通道 |
| 任务队列 (Task Queue) | 待执行任务的缓冲队列，支持优先级排序 |
| 冷却期 (Cooldown) | 同一规则/任务两次执行之间的最小间隔 |

---

## 一、整体架构

```
┌──────────────────────────────────────────────────────────────┐
│                     Task Orchestration Engine                  │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────────────┐    ┌───────────────────────────┐   │
│  │   Scheduler Module   │    │   Rule Engine Module       │   │
│  │  (定时触发器)         │    │  (事件触发器)              │   │
│  │                      │    │                           │   │
│  │  cron/interval/once  │    │  Event → Condition → Action│   │
│  │  → 排队到 Task Queue │    │  → 排队到 Task Queue       │   │
│  └──────────┬───────────┘    └───────────┬───────────────┘   │
│             │                            │                   │
│  ┌──────────▼────────────────────────────▼───────────────┐   │
│  │                  Task Queue (任务队列)                  │   │
│  │  优先级队列 | 去重 | 依赖检查 | 结果回调                │   │
│  └──────────────────────────┬────────────────────────────┘   │
│                             │                                 │
│  ┌──────────────────────────▼────────────────────────────┐   │
│  │               CDP Task Runner (CDP 执行器)              │   │
│  │  LaunchCode 解析 → 浏览器操作 → 结果收集 → 状态上报    │   │
│  └──────────────────────────┬────────────────────────────┘   │
│                             │                                 │
│  ┌──────────────────────────▼────────────────────────────┐   │
│  │               Event Bus (事件总线)                      │   │
│  │  task:created / task:started / task:completed / task:failed│
│  └──────────────────────────────────────────────────────────┘  │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

---

## 二、调度器模块 (Scheduler)

### 2.1 触发类型

```go
// backend/internal/scheduler/scheduler.go (378行)
```

| 类型 | 配置方式 | 示例 | 用途 |
|------|---------|------|------|
| `cron` | 标准 cron 表达式 | `0 */6 * * *` | 固定周期任务 |
| `interval` | 固定间隔(分钟) | `interval: 120` | 定期维护任务 |
| `once` | 指定时间点 | `at: 2026-06-01T00:00:00Z` | 一次性定时任务 |
| `event` | 事件触发(规则引擎) | `on: task:completed` | 事件驱动任务 |
| `chain` | 前序任务完成 | `depends_on: task_abc` | 依赖链任务 |

### 2.2 调度器设计

```go
type Scheduler struct {
    tasks     map[string]*ScheduledTask  // 注册任务列表
    cronTab   *cron.Cron                 // cron 调度器
    queue     *TaskQueue                 // 任务队列
    eventBus  *EventBus                  // 事件总线
}

type ScheduledTask struct {
    ID          string          `json:"id"`
    Name        string          `json:"name"`
    Type        string           `json:"type"`         // cron / interval / once
    Schedule    string           `json:"schedule"`     // cron 表达式或间隔分钟
    TaskDef     *TaskDefinition   `json:"taskDef"`      // 任务定义引用
    Enabled     bool              `json:"enabled"`
    CooldownSec int               `json:"cooldownSec"`  // 冷却期(秒)
    LastRun     time.Time         `json:"lastRun"`
    NextRun     time.Time         `json:"nextRun"`
    CreatedAt   time.Time         `json:"createdAt"`
}

// 调度循环
func (s *Scheduler) Start(ctx context.Context) {
    // 1. 恢复持久化任务
    for _, task := range s.loadTasks() {
        if task.Enabled {
            s.scheduleTask(task)
        }
    }
    
    // 2. 监听事件总线
    //    task:completed → 检查是否有依赖 chain 任务
    //    task:failed    → 记录失败次数，达到阈值时停用
    
    // 3. 等待 ctx.Done()
}
```

### 2.3 任务定义

```go
type TaskDefinition struct {
    ID          string            `json:"id"`
    Name        string            `json:"name"`
    
    // 执行内容
    Action      string            `json:"action"`     // navigate / click / type / script / workflow
    Payload     map[string]any    `json:"payload"`
    
    // 目标浏览器
    ProfileID   string            `json:"profileId,omitempty"`    // 指定 profile
    ProfileTags []string          `json:"profileTags,omitempty"`  // 或按标签选择
    
    // 执行配置
    TimeoutSec  int               `json:"timeoutSec"`
    RetryCount  int               `json:"retryCount"`
    Priority    int               `json:"priority"`    // 0-100, 越高越优先
    
    // 结果处理
    OnSuccess   *ResultAction     `json:"onSuccess,omitempty"`
    OnFailure   *ResultAction     `json:"onFailure,omitempty"`
}

type ResultAction struct {
    Type       string            `json:"type"`       // webhook / event / log / noop
    WebhookURL string            `json:"webhookURL,omitempty"`
    EventName  string            `json:"eventName,omitempty"`
    Message    string            `json:"message,omitempty"`
}
```

---

## 三、规则引擎模块 (Rule Engine)

### 3.1 规则结构

```go
// backend/internal/automation/engine.go (269行)
// backend/internal/automation/types.go
// backend/internal/automation/rule_dao.go
```

```go
type AutoRule struct {
    ID          string            `json:"id"`
    Name        string            `json:"name"`
    Enabled     bool              `json:"enabled"`
    
    // 触发条件
    Trigger     RuleTrigger       `json:"trigger"`
    
    // 条件检查
    Conditions  []RuleCondition   `json:"conditions,omitempty"`  // AND 逻辑
    
    // 动作
    Actions     []RuleAction      `json:"actions"`
    
    // 约束
    CooldownSec int              `json:"cooldownSec"`   // 冷却期
    MaxExec     int              `json:"maxExec"`       // 最大执行次数(0=不限)
    
    // 统计
    ExecCount   int              `json:"execCount"`
    LastExecAt  time.Time        `json:"lastExecAt"`
    CreatedAt   time.Time        `json:"createdAt"`
}

type RuleTrigger struct {
    Type        string            `json:"type"`        // event / schedule / manual
    EventNames  []string          `json:"eventNames,omitempty"`  // 事件名称列表
    Schedule    string            `json:"schedule,omitempty"`    // cron 表达式
}

type RuleCondition struct {
    Field       string            `json:"field"`       // event.data.field
    Operator    string            `json:"operator"`    // eq / neq / gt / lt / contains / exists
    Value       any               `json:"value"`
}

type RuleAction struct {
    Type        string            `json:"type"`       // emit_event / run_task / notify / webhook / noop
    EventName   string            `json:"eventName,omitempty"`
    TaskDefID   string            `json:"taskDefId,omitempty"`
    WebhookURL  string            `json:"webhookURL,omitempty"`
    Message     string            `json:"message,omitempty"`
}
```

### 3.2 规则评估循环

```go
func (e *RuleEngine) Evaluate(ctx context.Context, event Event) {
    for _, rule := range e.rules {
        if !rule.Enabled {
            continue
        }
        
        // 1. 检查冷却期
        if time.Since(rule.LastExecAt) < time.Duration(rule.CooldownSec)*time.Second {
            continue
        }
        
        // 2. 检查最大执行次数
        if rule.MaxExec > 0 && rule.ExecCount >= rule.MaxExec {
            continue
        }
        
        // 3. 检查触发条件
        if !e.matchTrigger(rule.Trigger, event) {
            continue
        }
        
        // 4. 检查附加条件
        if !e.evaluateConditions(rule.Conditions, event.Data) {
            continue
        }
        
        // 5. 执行动作
        for _, action := range rule.Actions {
            e.executeAction(ctx, action, event)
        }
        
        // 6. 更新统计
        rule.ExecCount++
        rule.LastExecAt = time.Now()
        
        // 7. 持久化状态
        e.dao.UpdateRule(rule)
    }
}
```

### 3.3 内置规则模板

| 规则 | 触发事件 | 条件 | 动作 | 用途 |
|------|---------|------|------|------|
| `captcha-auto-solve` | `risk:captcha:detected` | type != unknown | 启动人机验证处理 | 遇到验证码自动解决 |
| `captcha-fallback` | `risk:captcha:failed` | attempt < 3 | 切换验证服务商 | 失败自动降级 |
| `proxy-health-watch` | `proxy:health:degraded` | score < 0.3 | 切换路由节点 | 路由不可用时切换 |
| `session-auto-save` | `task:completed` | profile != nil | 保存 SessionBundle | 任务完成自动保存 |
| `nurture-daily` | 定时 (cron) | 每天 10:00 | 执行养号工作流 | 每日活跃度维护 |
| `storage-cleanup` | 定时 (interval) | 每 24h | 清理过期 profile | 维护磁盘空间 |

---

## 四、CDP 任务执行器 (CDP Task Runner)

### 4.1 执行流程

```go
// backend/internal/scheduler/cdp_runner.go (338行)
```

```
TaskEnqueue(TaskDefinition)
    ↓
TaskQueue.Pop() → 获取最高优先级任务
    ↓
CDPRunner.Execute(task)
    ├── 1. 解析 LaunchCode 或 Action 定义
    ├── 2. 获取或启动浏览器实例
    ├── 3. 获取或创建 CDP 连接
    ├── 4. 注入环境标识参数（Fingerprint Injector）
    ├── 5. 执行浏览器操作
    │   ├── navigate → Page.navigate
    │   ├── click → Input.dispatchMouseEvent (带人机交互)
    │   ├── type → Input.dispatchKeyEvent (带敲击节奏)
    │   ├── extract → Runtime.evaluate
    │   └── script → Runtime.evaluate (自定义JS)
    ├── 6. 收集执行结果
    ├── 7. 发射完成事件 (task:completed / task:failed)
    └── 8. 释放浏览器实例（按配置决定保持或关闭）
```

### 4.2 任务队列

```go
// 基于优先级的任务队列
type TaskQueue struct {
    mu       sync.Mutex
    items    []*QueuedTask
    priority int  // 0-100
}

type QueuedTask struct {
    Definition *TaskDefinition
    EnqueuedAt time.Time
    Priority   int              // 动态优先级 = 声明优先级 + 等待时间加成
    Status     string           // queued / running / completed / failed
    Attempt    int              // 当前尝试次数
    MaxAttempt int
}

// 排序规则:
// 1. 声明优先级高 → 优先
// 2. 等待时间加成: 每等待 1 分钟，优先级 +1
// 3. 同优先级按入队时间排序
```

### 4.3 任务执行上下文

```go
// 每个任务执行时携带的上下文
type TaskContext struct {
    TaskID       string
    Definition   *TaskDefinition
    Profile      *Profile
    Browser      *BrowserInstance
    CDP          *CDPExecutor
    Humanizer    *humanize.Config   // 行为保真度配置
    Variables    map[string]any     // 任务级变量
    StartTime    time.Time
    Logger       *TaskLogger
}

// 生命周期钩子
type TaskHooks struct {
    BeforeTask   func(ctx *TaskContext) error
    AfterTask    func(ctx *TaskContext, result *TaskResult) error
    OnFailure    func(ctx *TaskContext, err error) error
    OnRetry      func(ctx *TaskContext, attempt int) error
}

// 清理策略
type CleanupPolicy string
const (
    CleanupKeep   CleanupPolicy = "keep"    // 保持浏览器运行
    CleanupClose  CleanupPolicy = "close"   // 关闭浏览器
    CleanupSave   CleanupPolicy = "save"    // 保存会话后关闭
)
```

### 4.4 事件集成

```go
// 任务执行过程中发射的事件
const (
    // 任务生命周期事件
    EventTaskCreated   = "task:created"
    EventTaskStarted   = "task:started"
    EventTaskRetried   = "task:retried"
    EventTaskCompleted = "task:completed"
    EventTaskFailed    = "task:failed"
    
    // 运行时事件
    EventBrowserLaunched   = "browser:launched"
    EventBrowserCrashed    = "browser:crashed"
    EventCaptchaDetected   = "risk:captcha:detected"
    EventCaptchaSolved     = "risk:captcha:solved"
    EventCaptchaFailed     = "risk:captcha:failed"
    EventProxyDegraded     = "proxy:health:degraded"
    EventProxySwitched     = "proxy:switched"
)
```

**事件数据结构：**

```go
type TaskEvent struct {
    Type       string            `json:"type"`
    TaskID     string            `json:"taskId"`
    ProfileID  string            `json:"profileId,omitempty"`
    Timestamp  time.Time         `json:"timestamp"`
    Data       map[string]any    `json:"data,omitempty"`
    Error      string            `json:"error,omitempty"`
    DurationMs int64             `json:"durationMs,omitempty"`
}
```

---

## 五、Webhook 通知

```go
// backend/internal/webhook/webhook.go
```

任务结果通过 HTTP Webhook 通知外部系统：

```json
// POST {webhook_url}
{
  "event": "task:completed",
  "taskId": "task-abc-123",
  "profileId": "profile-xyz",
  "status": "success",
  "result": {
    "url": "https://target-site.com/dashboard",
    "screenshotUrl": "...",
    "extractedData": { ... }
  },
  "durationMs": 45200,
  "timestamp": "2026-05-27T10:30:00Z"
}
```

**Webhook 配置：**

```yaml
webhook:
  task_completed: "https://ops.internal/webhook/task-done"
  task_failed: "https://ops.internal/webhook/task-failed"
  retry_on_failure: true
  retry_max: 3
  retry_interval: 10s
```

---

## 六、API 端点

| 端点 | 方法 | 功能 |
|------|------|------|
| `/api/scheduler/task` | POST | 创建定时任务 |
| `/api/scheduler/task/{id}` | GET | 查询任务 |
| `/api/scheduler/task/{id}` | PUT | 更新任务 |
| `/api/scheduler/task/{id}` | DELETE | 删除任务 |
| `/api/scheduler/task/{id}/trigger` | POST | 手动触发任务 |
| `/api/scheduler/task/list` | GET | 任务列表 |
| `/api/scheduler/rule` | POST | 创建自动化规则 |
| `/api/scheduler/rule/{id}` | GET/PUT/DELETE | 规则 CRUD |
| `/api/scheduler/rule/list` | GET | 规则列表 |
| `/api/scheduler/execution/{id}` | GET | 查询执行记录 |
| `/api/scheduler/execution/list` | GET | 执行历史 |
| `/api/webhook/config` | GET/PUT | 通知配置 |

---

## 七、已有代码资产

| 模块 | 代码文件 | 行数 | 文档状态 |
|------|---------|------|---------|
| 调度器 | `backend/internal/scheduler/scheduler.go` | 378 | ❌ 本文档 |
| CDP 任务执行器 | `backend/internal/scheduler/cdp_runner.go` | 338 | ❌ 本文档 |
| 规则引擎 | `backend/internal/automation/engine.go` | 269 | ❌ 本文档 |
| 规则类型 | `backend/internal/automation/types.go` | — | ❌ 本文档 |
| 规则持久化 | `backend/internal/automation/rule_dao.go` | — | ❌ 本文档 |
| App 绑定-调度 | `backend/app_scheduler.go` | 188 | ❌ 本文档 |
| App 绑定-规则 | `backend/app_automation.go` | 193 | ❌ 本文档 |
| Webhook | `backend/internal/webhook/webhook.go` | — | ❌ 本文档 |
| Python 调度器 | `scripts/scheduler_daemon.py` | — | ❌ 本文档 |
| 实例管理 | `backend/app_instance.go` | — | ❌ 本文档 |
| LaunchCode REST | `backend/internal/launchcode/` | — | ❌ 本文档 |
