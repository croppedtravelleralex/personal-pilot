# 信任状态继承与环境一致性治理设计

## 问题背景

当前 PersonaPilot 已覆盖指纹、代理、行为和会话等多个运行面，但这些能力如果只按单点功能推进，容易出现三个长期风险：环境字段互相矛盾、授权凭证生命周期不可控、遇到风险/验证信号时继续自动重试。Phase 6 的目标不是绕过第三方检测，而是把已授权账号、会话、网络与运行环境管理成可审计、可暂停、可迁移的一致性系统。

## 安全边界

1. 仅面向用户自有账号、授权账号、内部系统或明确许可的测试环境。
2. 不实现绕过 CAPTCHA、403、账号验证、访问控制或平台反滥用机制的自动化方案。
3. 发现风险/验证信号时，默认暂停任务、记录证据、交由人工复核，而不是自动规避。
4. 凭证管理必须遵守最小权限、显式授权、可撤销和加密存储原则。
5. 任何真实 provider 或 OAuth2 验收都必须先记录授权范围和数据处理边界；跨机器迁移验收已按本机自用范围取消。

## 设计哲学

本架构的核心是把长期账号运行从“临时拼字段”提升为“可信状态管理”：

1. **减少不必要浏览器操作**：能通过官方、授权 API 完成的任务，不强行走浏览器自动化。
2. **信任状态继承**：用户授权建立的 session、token、device profile 和风险状态可以被生命周期化管理。
3. **一致性优先**：设备族、浏览器内核、代理出站、语言地区和行为节奏必须彼此自洽。
4. **可观测反馈**：风险/验证信号进入事件流，驱动暂停、降级、人工复核和后续修复。

## 架构总图

```text
五层治理：

层1: 传输一致性层    — 代理出站、TLS/HTTP 模板与所选浏览器内核保持一致
层2: 凭证治理层      — 授权 token/session 生命周期、加密存储、续期与撤销
层3: 设备族一致性层  — 从设备族生成自洽 profile，避免字段级随机拼接
层4: 行为可审计层    — 行为计划可回放、可解释、可限制，不做无限重试
层5: 风险反馈层      — 检测风险/验证信号后暂停任务、记录证据、人工复核
```

## 各层设计

### 层1 — 传输一致性层

**目标**：避免浏览器内核、代理出站和网络特征之间出现明显配置矛盾。

**关键设计**：

- 预置 3-4 个浏览器族 TLS/HTTP 参数模板（Chrome / Edge / Firefox），用于内部一致性检查和出站配置生成。
- 代理出站配置（Xray / Sing-Box）构建时显式绑定 runtime family、ALPN、header order 和 HTTP/2 策略。
- 输出 evidence：当前 profile 使用了哪个 runtime family、哪个代理模板、哪些字段仍未覆盖。

**集成点**：`proxy/xray.go`、`proxy/singbox.go`、`backend/internal/transport/` 的出站配置生成函数。

### 层2 — 凭证治理层

**目标**：对用户明确授权的 session/token 做生命周期管理，避免凭证散落、过期后误用或本机 restore 失控。

**关键设计**：

- `CredentialChain`：绑定到 Profile 的授权来源、scope、创建时间、过期时间、刷新状态和撤销状态。
- 信任等级：`initial`（首次授权）→ `established`（凭证可用）→ `needs_review`（需要人工确认）→ `revoked`（已撤销）。
- Access Token / Refresh Token 加密存储，复用现有 AES-GCM 基础设施。
- Profile 本机导入/恢复时凭证默认不导出；只有显式本地开关和授权记录齐全时才纳入 SessionBundle。跨机器凭证迁移已取消。

**集成点**：`session/bundle.go` 扩展、SQLite 新增 `adversarial_credential_chains` 表。

### 层3 — 设备族一致性层

**目标**：从真实设备族谱生成完整自洽的 profile，而不是字段级独立随机。

**关键设计**：

- 预置设备族：Win11 商务办公本、家用台式机、创作者工作站。
- 每个设备族定义 Screen / Hardware / GPU / Locale / Font / Plugin / MediaDevices 参数。
- 指纹生成器 `GenerateFromFamily(family, seed)` 输出自洽字段，并保留 seed 与生成版本。
- Consistency Engine 评分复用 `fingerprint_consistency.go` 的 10 维交叉验证。

**集成点**：`behavior/environment_injector.go` 上游新增生成器，生成 DeviceProfile 替代散装字段。

### 层4 — 行为可审计层

**目标**：把自动化行为做成可解释、可限制、可回放的计划，而不是不可控的无限自动重试。

**关键设计**：

- 扩展现有 `humanize/` 包：新增贝塞尔曲线变体、视线扫描模拟、页面间过渡行为。
- 噪声 Profile 设计：不同设备族/不同场景有不同节奏，但必须能被审计和复现。
- 错误注入、长暂停、阅读停顿等能力默认由策略开关控制，并记录到 behavior audit。
- IME 输入法链路作为可选能力，默认只在明确需要中文输入的已授权流程中启用。

**集成点**：`behavior/humanize/` 内部扩展，对调用方保持 typed plan 输入输出。

### 层5 — 风险反馈层

**目标**：把 CAPTCHA、403、账号验证、异常跳转等信号纳入证据链，驱动暂停、降级、人工复核和配置修正。

**关键设计**：

- Detector：检测 CAPTCHA 弹出、403、账号验证、异常跳转、登录失效等信号。
- Analyzer：归因到可能的配置问题，例如代理健康、地区不一致、凭证过期、profile 缺字段。
- Policy：默认动作是暂停任务、标记 profile、通知人工复核；禁止默认自动绕过或无限重试。
- 通过已有 `events.EmitAndLog()` → `automation.RuleEngine` 管道驱动审计与人工处理流程。

**集成点**：复用 `events/` 已定义的 `risk:*` 事件、`automation/` 规则引擎和 Validation Board evidence。

## 数据流

```text
用户触发操作
  │
  ├─ 有授权 API / 凭证可用? ─→ 使用官方授权路径完成 → 写入审计记录
  │
  └─ 需要浏览器环境
       │
       ├─ 设备族生成完整 profile
       ├─ Consistency Engine 预检评分
       ├─ 传输一致性模板绑定代理出站
       ├─ 启动浏览器 → 注入环境脚本
       ├─ 行为计划执行并记录 evidence
       │
       └─ 运行中
            ├─ 出现风险/验证信号? → 暂停 → 记录证据 → 人工复核
            └─ 成功 → 更新凭证/会话状态与审计记录
```

## 依赖关系

```text
层1（传输一致性） ← 依赖 backend/internal/transport/ 与代理配置生成
层2（凭证治理）   ← 依赖 SQLite、AES-GCM、SessionBundle
层3（设备族一致性）← 依赖 behavior/environment_injector.go + consistency_matrix.go
层4（行为可审计） ← 依赖 humanize/ 包和 behavior audit
层5（风险反馈）   ← 依赖 events/ + automation/ + Validation Board evidence
```

所有五层可独立验证，层间无阻塞依赖。

## 与现有架构的关系

- 全部实现在 Go backend 内，不新增语言。
- 不修改 Tauri / Rust 层。
- 不新增第二套 UI；后续只在 Settings / Validation 面板展示状态。
- 新增数据库表使用 `adversarial_` 前缀，迁移必须可回滚、可审计。
- 所有 native/system 调用继续通过既有服务边界，不绕过 `src/services/desktop.ts` 规则。

## 风险与限制

1. **授权边界**：OAuth2、provider API、SessionBundle 迁移必须先确认用户授权范围。
2. **凭证安全**：token 泄露风险高，必须加密、脱敏展示、支持撤销和过期清理。
3. **模板维护**：TLS/HTTP 和设备族模板需要随浏览器版本更新。
4. **误判成本**：风险信号初期可能误报，必须保留人工复核路径。
5. **不承诺规避检测**：本设计目标是合规的一致性治理与风险可观测，不是绕过第三方访问控制或反滥用机制。
