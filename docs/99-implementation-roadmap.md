# PersonaPilot 全量实施路线图

> 所有待实现功能，按依赖关系分 5 个 Phase，按 P0-P2 排列。
>
> 2026-05-28 更新：Phase 1 P1/P2、Phase 2 P1 与 Phase 3-5 roadmap 文件级待办均已有可测试实现或 adapter/contract 实现；Camoufox 已作为主线 browser core kind 接入内核管理/校验/启动参数分发/UI 选择。`scripts/roadmap_evidence_smoke.ps1` 最新 report 为 `data/reports/roadmap-evidence/roadmap-evidence-smoke-1779951732878.json`，状态 `passed_with_external_evidence_pending`；`scripts/profile_browser_environment_probe.mjs` 已生成真实 Chromium profile-browser 观测报告 `data/validation-reports/profile-browser-environment-1779951691042.json`，状态 `passed`。仍需用真实 provider、跨机器、Camoufox runtime 和外部分发 smoke 证明生产闭环。

---

## Phase 1: 客户端环境参数注入系统

**来源：** `docs/26-environment-compatibility-runtime-design.md`  
**估时：** 5d

### P0

- [x] **1.1** `backend/internal/behavior/environment_injector.go`
  - [x] JS 脚本模板引擎（参数→JS 片段编译）
  - [x] 批量脚本生成（单次 CDP 注入）
  - [x] CDP `Page.addScriptToEvaluateOnNewDocument` 集成
- [x] **1.2** `browser_api_surface` 族注入 (44 个参数；本机实现证据 passed，Chromium profile-browser 观测 passed)
  - [x] `navigator.webdriver` → `undefined`
  - [x] `navigator.plugins` → profile 插件列表
  - [x] `navigator.mimeTypes` → 匹配 plugins
  - [x] `navigator.languages` → 从 Profile 读取
  - [x] `navigator.platform` / `vendor` / `userAgent` 一致性链
  - [x] `navigator.hardwareConcurrency` / `deviceMemory` → 配置值
- [x] **1.3** `canvas_rendering` 族注入 (38 个参数；本机实现证据 passed，Chromium profile-browser 观测 passed)
  - [x] `toDataURL()` hook + 亚像素噪声（基于 profile seed）
  - [x] `getImageData()` 返回值偏移
  - [x] `fillText()` 字形扰动
- [x] **1.4** `timezone_locale` 族注入 (34 个参数；本机实现证据 passed，Chromium profile-browser 观测 passed)
  - [x] `Intl.DateTimeFormat().resolvedOptions()` 注入
  - [x] `Date.getTimezoneOffset()` 偏差注入
  - [x] `Accept-Language` header 一致性协调

### P1

- [x] **1.5** `webgl_gpu` 族注入 (42 个参数)
  - [x] `getParameter(UNMASKED_VENDOR_WEBGL)` 注入
  - [x] `getParameter(UNMASKED_RENDERER_WEBGL)` 注入
  - [x] `getSupportedExtensions()` 过滤
- [x] **1.6** `audio_stack` 族注入 (34 个参数)
  - [x] `AnalyserNode.getFloatFrequencyData()` 频域偏移
  - [x] `AudioBuffer.copyFromChannel()` 噪声
- [x] **1.7** `fonts_text_metrics` 族注入 (34 个参数)
  - [x] `document.fonts` 拦截器
  - [x] Font 白名单列表注入
- [x] **1.8** `media_devices` 族注入 (30 个参数)
  - [x] `enumerateDevices()` 模拟设备列表
  - [x] `getUserMedia()` 权限返回
- [x] **1.9** `webrtc_ip_leak` 族注入 (36 个参数)
  - [x] `RTCPeerConnection.createOffer()` hook
  - [x] SDP `a=candidate` 字段过滤
  - [x] ICE candidate gathering 拦截

### P2

- [x] **1.10** `backend/internal/browser/consistency_matrix.go`
  - [x] 时区↔语言↔Accept-Language 一致性
  - [x] Screen 分辨率↔Window 大小↔DPR 一致性
  - [x] WebGL vendor↔GPU 驱动↔OS 平台一致性
  - [x] 路由出口地区↔时区↔语言一致性

---

## Phase 2: 交互行为引擎增强

**来源：** `docs/30-behavioral-fidelity-layer.md`  
**估时：** 8d

### P0

- [x] **2.1** `backend/internal/behavior/humanize/trajectory_fitts.go`
  - [x] Fitts' Law 速度模型: `MT = a + b·log₂(D/W+1)`
  - [x] 三段式路径: 加速段/匀速段/减速段
- [x] **2.2** `backend/internal/behavior/humanize/trajectory_noise.go`
  - [x] 1/f 粉噪声发生器（8-12Hz 等价 octaves）
  - [x] 噪声叠加到 Fitts 轨迹
- [x] **2.3** `backend/internal/behavior/humanize/trajectory_click.go`
  - [x] 四段式点击: 预压(20ms) → 触峰 → 后压(50ms) → 释放
- [x] **2.4** `backend/internal/behavior/humanize/trajectory_doubleclick.go`
  - [x] N(300ms, 50ms) 双键间隔，首击后微移 <5px
- [x] **2.5** `backend/internal/behavior/humanize/trajectory_drag.go`
  - [x] 拖拽模型: 按下→慢移→微停→继续→松开
- [x] **2.6** `backend/internal/behavior/humanize/trajectory_context.go`
  - [x] 右键菜单: Hover→pause→contextMenu event

### P1

- [x] **2.7** `backend/internal/behavior/humanize/typing_finger.go`
  - [x] QWERTY 手指分区速度比（食指 80%, 小指 40%）
- [x] **2.8** `backend/internal/behavior/humanize/typing_ime.go`
  - [x] 中文 IME 模拟: 拼音→候选→Arrow→Enter
- [x] **2.9** `backend/internal/behavior/humanize/typing_modifier.go`
  - [x] Ctrl+C/V/A/Z 组合键序列
- [x] **2.10** `backend/internal/behavior/humanize/typing_fatigue.go`
  - [x] 每分钟 WPM 衰减 2-3%
- [x] **2.11** `backend/internal/behavior/humanize/typing_context.go`
  - [x] 密码输入比普通文本慢 40%
  - [x] 验证码逐格填入（格子间 pause）
- [x] **2.12** `backend/internal/behavior/humanize/typing_tab.go`
  - [x] Tab 字段切换间 N(500, 200)ms
- [x] **2.13** `backend/internal/behavior/humanize/typing_clipboard.go`
  - [x] 剪贴板交互 (Ctrl+A → Ctrl+C → Ctrl+V)
- [x] **2.14** `backend/internal/behavior/humanize/scroll_physics.go`
  - [x] 触摸板惯性: 初速度→摩擦减速→回弹
- [x] **2.15** `backend/internal/behavior/humanize/scroll_content.go`
  - [x] 内容感知: 段落末减速、图片上方暂停、标题跳过
- [x] **2.16** `backend/internal/behavior/humanize/scroll_reread.go`
  - [x] 回读: 15-25% 概率回滚 100-300px
- [x] **2.17** `backend/internal/behavior/humanize/scroll_infinite.go`
  - [x] 无限滚动: 到底→暂停→加载更多→继续
- [x] **2.18** `backend/internal/behavior/humanize/scroll_microjitter.go`
  - [x] 无意微滚 ±20px

---

## Phase 3: 账号活跃度维护引擎

**来源：** `docs/26-environment-compatibility-runtime-design.md`  
**估时：** 5d

- [x] **3.1** `backend/internal/behavior/lifecycle/`（本机 runtime/store 证据 passed；真实账号调度归 Overall 外部验收）
  - [x] `engine.go` — 主引擎（日常活跃周期执行）
  - [x] `state.go` — SQLite 状态持久化
  - [x] `duration.go` — 基于存在时长的 session 策略
  - [x] `interest.go` — 兴趣向量演化
  - [x] `risk.go` — 行为自然度评分
- [x] **3.2** `backend/internal/behavior/lifecycle/content_types.go`
  - [x] 8 种页面类型消费模型: article/search_results/listing/product/form/auth/dashboard/generic
- [x] **3.3** `backend/internal/behavior/lifecycle/social_progression.go`
  - [x] 社交渐进: only-read → like → follow → comment
- [x] **3.4** `backend/internal/behavior/lifecycle/geo_coherence.go`
  - [x] 出口地区↔浏览时段↔内容偏好联动

---

## Phase 4: 工作流引擎与插件系统

**来源：** `docs/27-workflow-plugin-system-design.md`  
**依赖：** Phase 1 + 2  
**估时：** 10d

### P0

- [x] **4.1** `backend/internal/workflow/`
  - [x] `types.go` — Workflow/Step/Action/Config
  - [x] `engine.go` — 编排引擎（顺序/条件/并行）
  - [x] `executor.go` — 动作执行路由接口
  - [x] `variable.go` — 变量系统
  - [x] `condition.go` — 条件求值器
  - [x] `retry.go` — 重试策略引擎
- [x] **4.2** `backend/internal/workflow/plugin/`
  - [x] `registry.go` — 插件注册表
  - [x] `template.go` — 模板定义
  - [x] 内置插件: auth:login, auth:register, form:contact, provider:sms, provider:email, challenge:turnstile, challenge:recaptcha

### P1

- [x] **4.3** `backend/internal/workflow/recorder.go`
  - [x] CDP 事件→Steps 聚合
  - [x] 步骤自动归类
- [x] **4.4** `backend/internal/workflow/playback.go`
  - [x] 工作流级别多步编排
  - [x] 步骤间自然间隙计时

### P2

- [x] **4.5** `backend/internal/workflow/provider/`
  - [x] `gateway.go` — 统一 Provider 接口
  - [x] `sms_adapter.go` — 短信适配器
  - [x] `email_adapter.go` — 邮件适配器
  - [x] `challenge_adapter.go` — 人机验证适配器
- [x] **4.6** `backend/internal/workflow/form_mapper.go`
  - [x] 字段映射定义
  - [x] 批量表单填写执行器
  - [x] 数据生成器（name/email/phone/password/address）
- [x] **4.7** `backend/internal/launchcode/workflow_api.go`
  - [x] `POST /api/workflow`
  - [x] `POST /api/workflow/{id}/execute`
  - [x] `GET /api/workflow/{id}/status`
  - [x] `POST /api/plugin/install`
  - [x] `POST /api/workflow/{id}/export`

---

## Phase 5: 扩展功能构建

**来源：** `docs/34—38`  
**估时：** 15d

### 5.1 环境参数样板采集

- [x] **5.1.1** Collector JS probes（参数采集脚本 manifest）
- [x] **5.1.2** `backend/internal/reference/` — 样板库存储引擎
- [x] **5.1.3** `backend/internal/reference/recognizer.go` — 相似度匹配

### 5.2 跨浏览器会话迁移

- [x] **5.2.1** 环境参数映射表（Chromium ↔ Camoufox ↔ Lightpanda）
- [x] **5.2.2** `backend/internal/session/bundle.go` — Serialize/Deserialize + AES 加密
- [x] **5.2.3** `backend/internal/session/migrator.go` — 跨运行时 Cookie/Storage/参数迁移 contract

### 5.3 浏览器池管理

- [x] **5.3.1** `backend/internal/pool/`
  - [x] `engine.go` — 池引擎
  - [x] `policy.go` — 池策略
  - [x] `slot.go` — 槽位状态机
- [x] **5.3.2** 预热流程（Profile→路由→启动→注入）
- [x] **5.3.3** Acquire/Release API

### 5.4 页面结构自动发现

- [x] **5.4.1** `backend/internal/discovery/`
  - [x] `form_detector.go` — 表单检测
  - [x] `field_analyzer.go` — 字段语义分析
  - [x] `type_classifier.go` — 页面类型分类
  - [x] `challenge_recognizer.go` — 验证机制识别
  - [x] `flow_inferencer.go` — 流程推断
- [x] **5.4.2** 发现结果→工作流模板转换器

### 5.5 传输层特征优化

- [x] **5.5.1** `backend/internal/transport/`
  - [x] `tls_profile.go` — TLS 握手参数模板库
  - [x] `http2_profile.go` — HTTP/2 帧序随机化
  - [x] `header_order.go` — 请求头顺序模板
- [x] **5.5.2** Chromium 启动参数集成 contract
- [x] **5.5.3** Xray/Sing-Box 出站配置集成

---

## 任务汇总

| Phase | 内容 | 子任务数 | 估时 | 前置 |
|-------|------|---------|------|------|
| 1 | 客户端环境参数注入 | 10 | 5d | 无 |
| 2 | 交互行为引擎增强 | 18 | 8d | 无 |
| 3 | 账号活跃度维护引擎 | 6 | 5d | P1+P2 |
| 4 | 工作流引擎与插件系统 | 13 | 10d | P1+P2 |
| 5 | 扩展功能 | 18 | 15d | 递进 |
| **合计** | | **~65** | **~43d** | |
