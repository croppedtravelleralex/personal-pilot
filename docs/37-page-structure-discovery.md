# 页面结构自动发现系统设计

> 本文档描述自动分析目标网站 DOM 结构、提取表单字段、识别验证机制、推断交互流程的系统。
> 目的：为工作流引擎提供"零配置"的网站感知能力，自动生成交互步骤模板。

---

## 0. 术语表

| 术语 | 含义 |
|------|------|
| 结构发现 (Structure Discovery) | 自动分析页面 DOM 提取表单字段、按钮、交互区域 |
| 表单原型 (Form Archetype) | 表单的语义分类（登录/注册/联系/搜索/支付等） |
| 交互点 (Interaction Point) | 页面上可交互的元素（输入框/按钮/复选框/下拉框等） |
| 流程推断 (Flow Inference) | 根据页面结构推断完整的"填写→提交→验证"流程 |

## 一、架构

```
┌─────────────────────────────────────────────────────────────┐
│               Page Structure Discovery Engine                │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  输入: URL / Page DOM Snapshot                                │
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐   │
│  │ Form         │───▶│ Field        │───▶│ Archetype    │   │
│  │ Detector     │    │ Analyzer     │    │ Classifier   │   │
│  └──────────────┘    └──────────────┘    └──────────────┘   │
│        │                                                     │
│        ▼                                                     │
│  ┌──────────────┐    ┌──────────────┐                        │
│  │ Challenge    │───▶│ Flow         │───▶ 工作流模板         │
│  │ Recognizer   │    │ Inference    │                        │
│  └──────────────┘    └──────────────┘                        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## 二、表单检测 (Form Detector)

```
输入: Page DOM → 遍历所有 <form> 及 input-like 元素
输出: 表单列表，每个含字段详情

检测要素:
  - <form> 标签（最可靠）
  - <input> 按 type 分类: text/email/password/tel/checkbox/select
  - 无 <form> 包裹的 input（SPA 模式）→ JS 事件绑定检测
  - aria-label / placeholder / label 提取字段标签
  - name / id / autocomplete 提取语义
```

## 三、字段分析 (Field Analyzer)

```go
type DiscoveredField struct {
    Selector     string            // CSS 选择器
    Tag          string            // input / select / textarea
    InputType    string            // text / email / password / tel / etc.
    Label        string            // 人类可读标签
    Name         string            // name 属性
    AutoComplete string            // autocomplete 属性 (email / tel / name / etc.)
    Required     bool              // required 属性
    MaxLength    int               // maxlength
    Pattern      string            // pattern 正则

    // 推断
    SemanticType string  // email / name / phone / address / password / code / search / submit
    // 基于: label 文本 + autocomplete + name + type + placeholder
}

// 语义类型推断规则:
//   label~"email|邮箱|邮件|邮箱地址" + name~"email|mail" → email
//   label~"password|密码" + type="password" → password
//   label~"phone|手机|电话|tel" + autocomplete="tel" → phone
//   label~"code|验证码|otp" + maxlength=4-8 → verification_code
//   label~"name|名字|姓名" + autocomplete="name" → name
//   input[type="submit"] + value~"注册|sign|create|register" → submit_register
//   input[type="submit"] + value~"登录|signin|login" → submit_login
```

## 四、表单原型分类 (Archetype Classifier)

| 原型 | 判定条件 | 生成模板 |
|------|---------|---------|
| `auth_login` | 有 password 字段 + 无 email_verification | 3 步: 填用户名 → 填密码 → 提交 |
| `auth_register` | 有 password + email + verification_code | 8 步: 含邮箱/通知网关/人机验证 |
| `auth_reset` | password + code 字段 + "重置" 语义 | 5 步: 含验证码验证 |
| `form_contact` | 无 password + 无 verification | 3-5 步: 逐字段填写 |
| `form_checkout` | 含 address + payment 字段 | 6 步: 含地址/支付信息 |

## 五、验证机制识别 (Challenge Recognizer)

```go
type DetectedChallenge struct {
    Type    string    // turnstile / recaptcha_v2 / recaptcha_v3 / hcaptcha / image / none
    SiteKey string    // 验证服务 Site Key
    Mode    string    // invisible / interactive / checkbox
}

// 检测方式:
//   Turnstile:    iframe[src*="challenges.cloudflare.com"]
//   reCAPTCHA:    iframe[src*="google.com/recaptcha"], .g-recaptcha
//   hCaptcha:     iframe[src*="hcaptcha.com"], .h-captcha
//   Image CAPTCHA: <img> + <input> 组合
```

## 六、流程推断 (Flow Inference)

根据页面结构和 URL 模式推断完整交互流程：

```
输入: 当前页面 DOM + URL
输出: WorkflowTemplate

推断算法:
  1. 检测表单原型 → 确定基础模板
  2. 检测验证机制 → 插入人机验证步骤
  3. 检测通知网关字段 → 插入 Provider:wait 步骤
  4. 推断后续页面 → 基于链接 + 提交后预期 URL
  
  示例: "auth_register + Turnstile + email → 8 步模板"
```

## 七、API

| 端点 | 方法 | 功能 |
|------|------|------|
| `POST /api/discover/page` | POST | 分析指定 URL 的结构 |
| `POST /api/discover/form` | POST | 提取表单字段详情 |
| `POST /api/discover/flow` | POST | 推断完整流程模板 |
| `GET /api/discover/archetypes` | GET | 所有已识别的原型列表 |
