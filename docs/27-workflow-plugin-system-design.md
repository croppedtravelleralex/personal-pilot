# 工作流插件系统与自动化服务开通框架设计

> 本文档描述基于现有行为引擎 (`behavior/`)、通知网关 (`sms/`, `email/`)、人机验证 (`captcha/`) 和 CDP 执行器 (`cdp_executor.go`) 的上层工作流编排系统。
> 
> **设计目标：** 提供一个可视化、可编程的交互工作流框架，允许用户通过插件组合实现多步骤网站表单自动化、通知回调集成和会话状态维护。

---

## 0. 术语映射

| 文档使用的技术术语 | 等价含义 |
|------------------|---------|
| 服务开通流程 | 在目标网站上完成身份凭证创建的自动化步骤序列 |
| 工作流步骤 | 一个原子化的浏览器交互动作（导航/点击/输入/等待/提取） |
| 插件 | 可复用的工作流步骤模板，含预配置的选择器、参数和回调 |
| 步骤编辑器 | 可视化的工作流创建、修改和调试界面 |
| 表单字段映射 | 将数据源字段与网页表单输入框关联的配置 |
| 通知回调 | 接收外部通知（短信/邮件）并通过 API 提取验证码的操作 |
| 交互式挑战 | 网站用于验证访问者身份的挑战机制 |
| 会话票据 | 工作流执行过程中产生的身份凭证和状态快照 |
| 步骤回放 | 按录制顺序重放工作流步骤以复现执行结果 |

---

## 一、系统架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Workflow Plugin System                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  Workflow     │  │  Plugin      │  │  Step Recorder /         │  │
│  │  Engine       │──│  Registry    │──│  Playback Engine         │  │
│  │  (编排运行)    │  │  (插件仓库)   │  │  (录制回放引擎)           │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────────┘  │
│         │                 │                      │                  │
│         └─────────────────┼──────────────────────┘                  │
│                           │                                         │
│  ┌────────────────────────▼──────────────────────────────────────┐  │
│  │               Action Executor Layer (动作执行层)                │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐   │  │
│  │  │ CDP      │ │ Humanize │ │ Provider │ │ Session        │   │  │
│  │  │ Executor │ │ Middleware│ │ Gateway  │ │ Manager        │   │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └────────────────┘   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                           │                                         │
│  ┌────────────────────────▼──────────────────────────────────────┐  │
│  │               Infrastructure Layer (基础设施层)                  │  │
│  │  ┌────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ ┌────────┐  │  │
│  │  │Browser │ │   SMS    │ │  Email   │ │Human   │ │TLS     │  │  │
│  │  │Runtime │ │  Gateway  │ │ Gateway  │ │Verify  │ │Profile │  │  │
│  │  └────────┘ └──────────┘ └──────────┘ └────────┘ └────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 二、Workflow Engine（工作流引擎）

### 2.1 核心概念

```
Workflow = [Step, Step, Step, ...]   ← 有序步骤列表
Step     = { Action, Config, Filter, Retry, OnResult }
Action   = 原子操作类型（见 2.3）
Config   = 该步骤的参数（选择器/数据/超时）
Filter   = 条件判断（仅在此条件满足时执行该步骤）
Retry    = 失败重试策略
OnResult = 结果处理（保存/转换/传递到下一步）
```

### 2.2 工作流模型

```go
// backend/internal/workflow/types.go（拟新增）

type Workflow struct {
    ID          string            `json:"id"`
    Name        string            `json:"name"`
    Description string            `json:"description"`
    Steps       []Step            `json:"steps"`
    Variables   map[string]any    `json:"variables"`     // 工作流级变量
    Config      WorkflowConfig    `json:"config"`
    CreatedAt   time.Time         `json:"createdAt"`
    UpdatedAt   time.Time         `json:"updatedAt"`
}

type WorkflowConfig struct {
    MaxRetries    int              `json:"maxRetries"`    // 步骤默认重试
    TimeoutSec    int              `json:"timeoutSec"`    // 整体超时
    HumanizeLevel string           `json:"humanizeLevel"` // none/minimal/medium/high
    SessionTTL    time.Duration    `json:"sessionTTL"`    // 会话票据保留时间
    OnError       string           `json:"onError"`       // abort / skip / retry
}

type Step struct {
    ID          string            `json:"id"`
    Name        string            `json:"name"`
    Action      string            `json:"action"`        // 动作类型
    Target      *TargetConfig     `json:"target,omitempty"`
    Data        map[string]any    `json:"data,omitempty"`
    Wait        *WaitConfig       `json:"wait,omitempty"`
    Condition   *ConditionConfig  `json:"condition,omitempty"`
    Retry       *RetryConfig      `json:"retry,omitempty"`
    OnResult    *ResultHandler    `json:"onResult,omitempty"`
    TimeoutSec  int               `json:"timeoutSec,omitempty"`
}

type TargetConfig struct {
    Selector    string            `json:"selector"`       // CSS 选择器
    Frame       string            `json:"frame,omitempty"`// iframe 定位
    WaitVisible bool              `json:"waitVisible"`    // 等待元素可见
    TimeoutSec  int               `json:"timeoutSec"`
    Multi       bool              `json:"multi"`          // 多元素模式
}

type WaitConfig struct {
    Type        string            `json:"type"`           // fixed / selector / condition / random
    DurationMs  int               `json:"durationMs,omitempty"`
    Selector    string            `json:"selector,omitempty"`
    MinMs       int               `json:"minMs,omitempty"`
    MaxMs       int               `json:"maxMs,omitempty"`
}

type RetryConfig struct {
    MaxAttempts  int              `json:"maxAttempts"`
    IntervalMs   int              `json:"intervalMs"`
    Backoff      string           `json:"backoff"`       // fixed / linear / exponential
}

type ConditionConfig struct {
    Type        string            `json:"type"`           // selector_exists / url_match / variable_eq
    Selector    string            `json:"selector,omitempty"`
    URLPattern  string            `json:"urlPattern,omitempty"`
    Variable    string            `json:"variable,omitempty"`
    Value       any               `json:"value,omitempty"`
}

type ResultHandler struct {
    SaveTo      string            `json:"saveTo"`         // 变量名
    Transform   string            `json:"transform"`      // extract_text / extract_attr / json_path
    PassToNext  bool              `json:"passToNext"`     // 传递给下一步
}
```

### 2.3 原子动作类型

| 动作 | 代码常量 | 参数 | 说明 |
|------|---------|------|------|
| 导航 | `navigate` | URL | 跳转到指定URL |
| 点击 | `click` | target | 点击元素（带人机交互轨迹） |
| 输入 | `type` | target, value, humanize(true/false) | 逐字输入文本 |
| 选择 | `select` | target, value | 下拉框选择 |
| 勾选 | `check` | target, checked | 复选框操作 |
| 提取 | `extract` | target, attr | 提取页面内容到变量 |
| 等待 | `wait` | config | 固定/条件/随机等待 |
| 截图 | `screenshot` | selector(可选) | 截图保存 |
| JS执行 | `script` | expression | 执行自定义JS |
| 滚动 | `scroll` | target/amount | 滚动到元素/指定距离 |
| 表单填写 | `fill_form` | mapping | 批量填写表单字段 |
| 通知回调 | `provider:wait` | type, timeout | 等待通知并提取验证码 |
| 人机验证 | `challenge:solve` | type, timeout | 自动处理人机验证 |
| 条件判断 | `condition` | config | 根据条件决定流程走向 |
| 跳转 | `goto` | step_id | 跳转到指定步骤 |
| 子流程 | `subflow` | workflow_id | 执行子工作流 |
| 变量赋值 | `set_variable` | name, value | 设置工作流变量 |
| 会话保存 | `session:save` | — | 保存当前会话票据 |
| 通知发送 | `provider:notify` | type, message | 发送通知 |

### 2.4 状态图

```
READY ──→ RUNNING ──→ STEP_START ──→ STEP_EXECUTING ──→ STEP_DONE
           │              │                │
           │              │                ├──→ STEP_FAILED ──→ RETRYING ──→ ...
           │              │                │                    │
           │              │                │                    └──→ STEP_ABORT
           │              │                │
           │              │                └──→ STEP_SKIPPED (条件不满足)
           │              │
           │              └──→ STEP_PAUSED (等待通知/人机验证)
           │
           └──→ COMPLETED / FAILED / ABORTED / TIMEOUT
```

---

## 三、Plugin Registry（插件注册表）

### 3.1 插件定义

插件是可复用的工作流步骤模板，包含预定义的选择器、数据和回调。

```go
// backend/internal/workflow/plugin/types.go（拟新增）

type Plugin struct {
    ID          string            `json:"id"`
    Name        string            `json:"name"`
    Version     string            `json:"version"`
    Category    string            `json:"category"`     // form / auth / navigation / provider
    Tags        []string          `json:"tags"`
    Steps       []StepTemplate    `json:"steps"`
    Config      PluginConfig      `json:"config"`
    Icon        string            `json:"icon,omitempty"`
}

type StepTemplate struct {
    Name        string            `json:"name"`
    Action      string            `json:"action"`
    Target      *TargetConfig     `json:"target,omitempty"`
    SelectorHint string           `json:"selectorHint,omitempty"` // 选择器提示文本
    Parameters  []ParameterDef    `json:"parameters"`
    Humanize    bool              `json:"humanize"`     // 是否启用自然人机交互
}

type ParameterDef struct {
    Name        string            `json:"name"`
    Type        string            `json:"type"`         // string / number / boolean / selector / data_source
    Label       string            `json:"label"`
    Required    bool              `json:"required"`
    Default     any               `json:"default,omitempty"`
    Options     []string          `json:"options,omitempty"` // 枚举值
    Source      string            `json:"source,omitempty"`  // 数据来源: variable / api / provider
}
```

### 3.2 内置插件模板

| 插件 | 类别 | 步骤数 | 用途 |
|------|------|--------|------|
| `auth:login` | auth | 4 | 登录流程（导航→填用户名→填密码→提交） |
| `auth:register` | auth | 8 | 服务开通流程（导航→填邮箱→填密→通知验证→填验证码→提交） |
| `form:contact` | form | 5 | 联系表单填写（逐字段输入+提交） |
| `form:order` | form | 6 | 下单流程（选商品→填地址→填支付→确认） |
| `provider:sms` | provider | 2 | 短信号码获取+等待验证码 |
| `provider:email` | provider | 2 | 邮箱验证码等待+提取 |
| `challenge:turnstile` | challenge | 1 | 自动处理 Cloudflare 交互式挑战 |
| `challenge:recaptcha` | challenge | 1 | 自动处理 reCAPTCHA |
| `navigation:paged_list` | navigation | 3 | 分页列表翻页遍历 |
| `extraction:table` | navigation | 2 | 表格数据提取 |

### 3.3 插件开发接口

```go
// 插件开发者只需实现这个接口
type PluginProvider interface {
    ID() string
    Name() string
    Templates() []StepTemplate
    Validate(ctx context.Context, params map[string]any) error
    OnBeforeStep(ctx context.Context, step *Step, vars map[string]any) error
    OnAfterStep(ctx context.Context, step *Step, result *StepResult, vars map[string]any) error
}
```

---

## 四、Step Recorder / Playback（步骤录制与回放）

### 4.1 录制流程

```
用户操作浏览器
     ↓
CDP 事件捕获（click/input/scroll/navigate）
     ↓
EventMonitor 接收原始事件流
     ↓
WorkflowRecorder 将事件流聚合为 Steps
     ├── 连续 click + input → "表单填写步骤"
     ├── 独立 click → "点击步骤"
     ├── 连续 scroll → "滚动步骤"
     └── 多字段输入 → "表单批量填写步骤"
     ↓
用户可以在 Step Editor 中修改/调整
     ├── 修改选择器（CSS/XPath 自动提取）
     ├── 插入等待步骤
     ├── 添加通知回调步骤
     ├── 设置变量映射
     └── 配置人机验证自动处理
     ↓
保存为 Workflow 或 Plugin
```

### 4.2 步骤编辑器

```typescript
// 前端类型定义（frontend/src/modules/workflow/types.ts）

interface StepEditorProps {
    workflow: Workflow
    onStepAdd: (step: Step) => void
    onStepUpdate: (stepId: string, changes: Partial<Step>) => void
    onStepRemove: (stepId: string) => void
    onStepReorder: (fromIndex: number, toIndex: number) => void
}

// 编辑器功能
interface EditorCapabilities {
    // 选择器增强
    selectorPicker: {
        highlightOnHover: boolean    // 悬停高亮元素
        clickToCapture: boolean     // 点击提取选择器
        autoGenerateXPath: boolean  // 自动生成 XPath
        multiSelect: boolean        // 多元素选择
    }
    // 变量面板
    variableInspector: {
        showAllVariables: boolean   // 显示所有变量
        dragToField: boolean        // 拖拽变量到输入框
        previewValue: boolean       // 预览变量值
    }
    // 调试面板
    debugPanel: {
        stepByStep: boolean         // 单步执行
        pauseOnStep: boolean        // 暂停在指定步骤
        stateInspect: boolean       // 查看当前状态
        screenshotOnStep: boolean   // 每步截图
    }
}
```

### 4.3 回放执行

```go
// backend/internal/workflow/playback.go（拟新增）

type PlaybackEngine struct {
    executor    *CDPExecutor
    humanizer   *humanize.Config
    providers   *provider.Manager    // 通知网关管理器
    captcha     *captcha.Manager      // 人机验证管理器
    session     *SessionManager
    recorder    *RecordingStore       // 录制存储（复用behavior包）
}

// ExecuteWorkflow 执行完整工作流
func (e *PlaybackEngine) ExecuteWorkflow(ctx context.Context, wf *Workflow) (*WorkflowResult, error) {
    // 1. 启动浏览器实例
    browser, err := e.launchBrowser(ctx, wf.Config)
    
    // 2. 遍历步骤列表
    for _, step := range wf.Steps {
        // 2a. 检查条件
        if step.Condition != nil && !e.evaluateCondition(step.Condition, vars) {
            recordSkip(step, "condition_not_met")
            continue
        }
        
        // 2b. 执行动作
        result, err := e.executeStep(ctx, browser, step, vars)
        
        // 2c. 处理结果
        if step.OnResult != nil {
            e.handleResult(step.OnResult, result, vars)
        }
        
        // 2d. 失败处理
        if err != nil {
            decision := humanize.HumanizedRetryDecision(
                mapError(err), step.Retry.Attempt, &humanizeConfig)
            switch decision.Action.Type {
            case RecoveryRetry, RecoveryRetryAfter:
                // 等待后重试
            case RecoveryGiveUp:
                return fail(step, err)
            case RecoverySkip:
                continue
            }
        }
        
        // 2e. 步骤间等待（自然人机交互节奏）
        gap := humanize.ComputeActionGap(&humanizeConfig)
        time.Sleep(time.Duration(gap) * time.Millisecond)
    }
    
    // 3. 保存会话票据
    e.session.Save(ctx, browser.SessionBundle())
    
    return success()
}
```

### 4.4 人机交互集成

| 录制方式 | 回放方式 | Humanize等级 | 适用场景 |
|---------|---------|-------------|---------|
| 原始CDP事件 | 直接CDP重放 | none | 简单功能验证 |
| 原始+时间戳 | 带自然延迟重放 | minimal | 需要近似人类节奏 |
| 原始+时间戳+偏移 | Fitts轨迹重放 | medium | 需要鼠标轨迹模拟 |
| 原始+时间戳+偏移+噪声 | 生理模型重放 | high | 需要极致拟真 |

---

## 五、通知网关集成

### 5.1 交互式挑战处理（人机验证）

```go
// 工作流中的「人机验证步骤」定义

// 在 Workflow Step 中：
step = {
    "action": "challenge:solve",
    "data": {
        "type": "auto",              // auto / turnstile / recaptcha / hcaptcha / image
        "timeout": 120,
        "fallback_behavior": [
            {"on": "timeout", "do": "switch_proxy"},
            {"on": "failed", "do": "retry"},
            {"on": "unsupported", "do": "pause_for_operator"}
        ]
    }
}

// 执行流程
// 1. CDP 检测交互式挑战 iframe
// 2. 识别类型（Turnstile/reCAPTCHA/hCaptcha/image）
// 3. 调用 Human Verification Manager 获取 token
//    - 缓存命中：直接返回
//    - Cache miss：调用 solver (capsolver/2captcha/ddddocr)
// 4. CDP 填入 token / 识别结果
// 5. 验证是否通过
// 6. 未通过则按 fallback_behavior 处理
```

### 5.2 短信通知网关

```go
// 工作流中的「短信接收步骤」定义

step = {
    "action": "provider:wait",
    "data": {
        "type": "sms",
        "provider": "5sim",          // 5sim / smspool / herosms
        "service": "google",         // 目标服务
        "country": "usa",
        "timeout": 180,
        "extract_otp": true,
        "fill_target": {
            "selector": "#phone_verify_code",
            "method": "type_text"    // type_text / paste
        },
        "after": [
            {"action": "release_number"}
        ]
    }
}

// 执行流程
// 1. 通过 Provider 购买号码（AcquireNumber）
// 2. CDP 填入号码到页面
// 3. CDP 点击"发送验证码"按钮
// 4. 轮询 WaitForCode（收到短信→提取OTP）
// 5. CDP 填入 OTP 到验证码输入框
// 6. ReleaseNumber（释放号码/标记完成）
```

### 5.3 邮件通知网关

```go
step = {
    "action": "provider:wait",
    "data": {
        "type": "email",
        "provider": "cloudflare_worker",
        "from_suffix": "noreply@target.com",
        "subject_contains": "verification",
        "timeout": 300,
        "extract_otp": true,
        "fill_target": {
            "selector": "#email_code",
            "method": "type_text"
        }
    }
}

// 执行流程
// 1. 创建临时邮箱（CreateInbox）
// 2. CDP 填入邮箱到注册页面
// 3. 轮询 WaitForCode（收到邮件→提取验证码）
// 4. CDP 填入验证码
```

### 5.4 并行等待（多通道同步）

```go
step = {
    "action": "provider:wait_parallel",
    "data": {
        "channels": [
            {"type": "email", "from_suffix": "noreply@target.com"},
            {"type": "sms", "service": "google", "country": "usa"}
        ],
        "fill_targets": {
            "email": {"selector": "#email_code"},
            "sms": {"selector": "#phone_code"}
        },
        "timeout": 300
    }
}

// 执行流程
// 并行启动 email 轮询 + sms 轮询
// 任一先到 → 填入对应字段
// 全部到齐 → 继续下一步
// 超时未到 → 按 fallback_behavior 处理
```

---

## 六、表单填写引擎

### 6.1 表单字段映射

```go
// backend/internal/workflow/form_mapper.go（拟新增）

type FormFieldMapping struct {
    FieldName    string            `json:"fieldName"`    // 表单字段名
    Selector     string            `json:"selector"`     // 元素选择器
    InputType    string            `json:"inputType"`    // text / email / password / tel / checkbox / select
    ValueSource  string            `json:"valueSource"`  // static / variable / generated / provider
    StaticValue  string            `json:"staticValue,omitempty"`
    VariableName string            `json:"variableName,omitempty"`
    Generator    string            `json:"generator,omitempty"`  // human_name / email / phone / random_string
    Validate     *ValidationRule   `json:"validate,omitempty"`   // 提交前校验
}

type ValidationRule struct {
    Type        string            `json:"type"`        // required / pattern / length / custom
    Pattern     string            `json:"pattern,omitempty"`
    MinLength   int               `json:"minLength,omitempty"`
    MaxLength   int               `json:"maxLength,omitempty"`
}
```

### 6.2 批量表单填写

```go
// 工作流步骤：一次填写多个字段

step = {
    "action": "fill_form",
    "data": {
        "mappings": [
            {"selector": "#email", "valueSource": "generated", "generator": "email", "humanize": true},
            {"selector": "#password", "valueSource": "generated", "generator": "random_password", "humanize": true},
            {"selector": "#name", "valueSource": "generated", "generator": "human_name", "humanize": true},
            {"selector": "#phone", "valueSource": "variable", "variableName": "sms_number", "humanize": false},
            {"selector": "#terms", "inputType": "checkbox", "valueSource": "static", "staticValue": "true"},
        ],
        "order": "sequential",       // sequential / parallel
        "humanize_typing": true,      // 逐字输入
        "error_retry": true,          // 输错后重试（退格→重输）
        "field_gap_ms": [200, 600]    // 字段间间隔范围
    }
}

// 执行流程
// 对于每个字段映射：
// 1. 等待元素可见
// 2. 移动到元素（贝塞尔轨迹）
// 3. 点击激活字段
// 4. 逐字输入（含打字节奏、纠错、思考暂停）
// 5. 移动到下一字段
// 6. 字段间间隔（随机 200-600ms）
```

### 6.3 数据生成器（内置）

| 生成器 | 输出示例 | 配置参数 |
|--------|---------|---------|
| `human_name` | James Smith | locale(gender) |
| `email` | james.smith84@deltajohnsons.com | domain |
| `phone` | +15627231715 | country_code |
| `random_password` | Kd9#mP2$xL | length, include_special |
| `random_string` | a8f3c9 | length, charset |
| `address` | 123 Main St, New York, NY 10001 | locale |
| `company` | Acme Corp | locale |
| `credit_card` | 4111-1111-1111-1111 | provider, bin |

---

## 七、实施依赖

| 模块 | 依赖现有代码 | 需新建 |
|------|------------|--------|
| Workflow Engine | `backend/app_launchcode.go` | `backend/internal/workflow/` 完整包 |
| Plugin Registry | `backend/internal/behavior/presets.go` | 插件定义 + 注册表 + 加载器 |
| Step Recorder | `backend/internal/behavior/recorder.go` | 事件→Steps 聚合逻辑 |
| Playback | `backend/internal/behavior/playback.go` | Workflow 级别的多步编排 |
| Step Editor | `frontend/src/modules/browser/pages/BehaviorRecordingPage.tsx` | 新的 WorkflowEditor 组件 |
| Provider Gateway | `backend/internal/sms/` + `email/` + `captcha/` | 统一 Provider Manager 接口 |
| Form Mapper | 无 | `backend/internal/workflow/form_mapper.go` |

---

## 八、REST API

| 端点 | 方法 | 功能 |
|------|------|------|
| `/api/workflow` | POST | 创建工作流 |
| `/api/workflow/{id}` | GET | 查询工作流 |
| `/api/workflow/{id}` | PUT | 更新工作流 |
| `/api/workflow/{id}` | DELETE | 删除工作流 |
| `/api/workflow/{id}/execute` | POST | 执行工作流 |
| `/api/workflow/{id}/status` | GET | 查询执行状态 |
| `/api/workflow/list` | GET | 工作流列表 |
| `/api/plugin` | POST | 注册插件 |
| `/api/plugin/list` | GET | 插件列表 |
| `/api/plugin/{id}/install` | POST | 安装插件 |
| `/api/workflow/{id}/export` | GET | 导出工作流为模板 |
| `/api/workflow/import` | POST | 从模板导入工作流 |

---

## 九、与现有行为引擎的关系

| 现有组件 | 关系 |
|---------|------|
| `cdp_executor.go` (1087行) | 底层 CDP 执行器 — 工作流引擎直接调用 |
| `humanize/` (6个包) | 自然人机交互中间件 — 每个步骤自动挂载 |
| `recorder.go` / `playback.go` | 录制回放基础 — 步骤录制器复用其事件捕获能力 |
| `auto_recorder.go` | 自动养号 — 可重构为工作流的一个特殊用例 |
| `sms/` + `email/` + `captcha/` | 通知网关 — 通过 Provider Gateway 接口统一暴露 |
| `humanize/failure.go` | 错误恢复策略 — 直接复用 HumanizedRetryDecision |
| `SessionBundle` | 会话票据 — 工作流执行结束后自动保存 |
