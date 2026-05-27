# PersonaPilot 全量实施路线图

> 所有待实现功能，按依赖关系分 5 个 Phase，按 P0-P2 排列。

---

## Phase 1: 客户端环境参数注入系统

**来源：** `docs/26-environment-compatibility-runtime-design.md`  
**估时：** 5d

### P0

- [ ] **1.1** `backend/internal/behavior/environment_injector.go`
  - [ ] JS 脚本模板引擎（参数→JS 片段编译）
  - [ ] 批量脚本生成（单次 CDP 注入）
  - [ ] CDP `Page.addScriptToEvaluateOnNewDocument` 集成
- [ ] **1.2** `browser_api_surface` 族注入 (44 个参数)
  - [ ] `navigator.webdriver` → `undefined`
  - [ ] `navigator.plugins` → 原生 Chrome 插件列表
  - [ ] `navigator.mimeTypes` → 匹配 plugins
  - [ ] `navigator.languages` → 从 Profile 读取
  - [ ] `navigator.platform` / `vendor` / `userAgent` 一致性链
  - [ ] `navigator.hardwareConcurrency` / `deviceMemory` → 配置值
- [ ] **1.3** `canvas_rendering` 族注入 (38 个参数)
  - [ ] `toDataURL()` hook + 亚像素噪声（基于 profile seed）
  - [ ] `getImageData()` 返回值偏移
  - [ ] `fillText()` 字形扰动
- [ ] **1.4** `timezone_locale` 族注入 (34 个参数)
  - [ ] `Intl.DateTimeFormat().resolvedOptions()` 注入
  - [ ] `Date.getTimezoneOffset()` 偏差注入
  - [ ] `Accept-Language` header 一致性协调

### P1

- [ ] **1.5** `webgl_gpu` 族注入 (42 个参数)
  - [ ] `getParameter(UNMASKED_VENDOR_WEBGL)` 注入
  - [ ] `getParameter(UNMASKED_RENDERER_WEBGL)` 注入
  - [ ] `getSupportedExtensions()` 过滤
- [ ] **1.6** `audio_stack` 族注入 (34 个参数)
  - [ ] `AnalyserNode.getFloatFrequencyData()` 频域偏移
  - [ ] `AudioBuffer.copyFromChannel()` 噪声
- [ ] **1.7** `fonts_text_metrics` 族注入 (34 个参数)
  - [ ] `document.fonts` 拦截器
  - [ ] Font 白名单列表注入
- [ ] **1.8** `media_devices` 族注入 (30 个参数)
  - [ ] `enumerateDevices()` 模拟设备列表
  - [ ] `getUserMedia()` 权限返回
- [ ] **1.9** `webrtc_ip_leak` 族注入 (36 个参数)
  - [ ] `RTCPeerConnection.createOffer()` hook
  - [ ] SDP `a=candidate` 字段过滤
  - [ ] ICE candidate gathering 拦截

### P2

- [ ] **1.10** `backend/internal/browser/consistency_matrix.go`
  - [ ] 时区↔语言↔Accept-Language 一致性
  - [ ] Screen 分辨率↔Window 大小↔DPR 一致性
  - [ ] WebGL vendor↔GPU 驱动↔OS 平台一致性
  - [ ] 路由出口地区↔时区↔语言一致性

---

## Phase 2: 交互行为引擎增强

**来源：** `docs/30-behavioral-fidelity-layer.md`  
**估时：** 8d

### P0

- [ ] **2.1** `backend/internal/behavior/humanize/trajectory_fitts.go`
  - [ ] Fitts' Law 速度模型: `MT = a + b·log₂(D/W+1)`
  - [ ] 三段式路径: 加速段/匀速段/减速段
- [ ] **2.2** `backend/internal/behavior/humanize/trajectory_noise.go`
  - [ ] 1/f 粉噪声发生器（Voss-McCartney 算法，8-12Hz）
  - [ ] 噪声叠加到现有 Bezier 轨迹
- [ ] **2.3** `backend/internal/behavior/humanize/trajectory_click.go`
  - [ ] 四段式点击: 预压(20ms) → 触峰 → 后压(50ms) → 释放
- [ ] **2.4** `backend/internal/behavior/humanize/trajectory_doubleclick.go`
  - [ ] N(300ms, 50ms) 双键间隔，首击后微移 <5px
- [ ] **2.5** `backend/internal/behavior/humanize/trajectory_drag.go`
  - [ ] 拖拽模型: 按下→慢移→微停→继续→松开
- [ ] **2.6** `backend/internal/behavior/humanize/trajectory_context.go`
  - [ ] 右键菜单: Hover→pause→contextMenu event

### P1

- [ ] **2.7** `backend/internal/behavior/humanize/typing_finger.go`
  - [ ] QWERTY 手指分区速度比（食指 80%, 小指 40%）
- [ ] **2.8** `backend/internal/behavior/humanize/typing_ime.go`
  - [ ] 中文 IME 模拟: 拼音→候选→Arrow→Enter
- [ ] **2.9** `backend/internal/behavior/humanize/typing_modifier.go`
  - [ ] Ctrl+C/V/A/Z 组合键序列
- [ ] **2.10** `backend/internal/behavior/humanize/typing_fatigue.go`
  - [ ] 每分钟 WPM 衰减 2-3%
- [ ] **2.11** `backend/internal/behavior/humanize/typing_context.go`
  - [ ] 密码输入比普通文本慢 40%
  - [ ] 验证码逐格填入（格子间 pause）
- [ ] **2.12** `backend/internal/behavior/humanize/typing_tab.go`
  - [ ] Tab 字段切换间 N(500, 200)ms
- [ ] **2.13** `backend/internal/behavior/humanize/typing_clipboard.go`
  - [ ] 剪贴板交互 (Ctrl+A → Ctrl+C → Ctrl+V)
- [ ] **2.14** `backend/internal/behavior/humanize/scroll_physics.go`
  - [ ] 触摸板惯性: 初速度→摩擦减速→回弹
- [ ] **2.15** `backend/internal/behavior/humanize/scroll_content.go`
  - [ ] 内容感知: 段落末减速、图片上方暂停、标题跳过
- [ ] **2.16** `backend/internal/behavior/humanize/scroll_reread.go`
  - [ ] 回读: 15-25% 概率回滚 100-300px
- [ ] **2.17** `backend/internal/behavior/humanize/scroll_infinite.go`
  - [ ] 无限滚动: 到底→暂停→加载更多→继续
- [ ] **2.18** `backend/internal/behavior/humanize/scroll_microjitter.go`
  - [ ] 无意微滚 ±20px

---

## Phase 3: 账号活跃度维护引擎

**来源：** `docs/26-environment-compatibility-runtime-design.md`  
**估时：** 5d

- [ ] **3.1** `backend/internal/behavior/lifecycle/`
  - [ ] `engine.go` — 主引擎（日常活跃周期执行）
  - [ ] `state.go` — SQLite 状态持久化
  - [ ] `duration.go` — 基于存在时长的 session 策略
  - [ ] `interest.go` — 兴趣向量演化
  - [ ] `risk.go` — 行为自然度评分
- [ ] **3.2** `backend/internal/behavior/lifecycle/content_types.go`
  - [ ] 8 种页面类型消费模型: article/search_results/listing/product/form/auth/dashboard/generic
- [ ] **3.3** `backend/internal/behavior/lifecycle/social_progression.go`
  - [ ] 社交渐进: only-read → like → follow → comment
- [ ] **3.4** `backend/internal/behavior/lifecycle/geo_coherence.go`
  - [ ] 出口地区↔浏览时段↔内容偏好联动

---

## Phase 4: 工作流引擎与插件系统

**来源：** `docs/27-workflow-plugin-system-design.md`  
**依赖：** Phase 1 + 2  
**估时：** 10d

### P0

- [ ] **4.1** `backend/internal/workflow/`
  - [ ] `types.go` — Workflow/Step/Action/Config
  - [ ] `engine.go` — 编排引擎（顺序/条件/并行）
  - [ ] `executor.go` — 动作执行路由
  - [ ] `variable.go` — 变量系统
  - [ ] `condition.go` — 条件求值器
  - [ ] `retry.go` — 重试策略引擎
- [ ] **4.2** `backend/internal/workflow/plugin/`
  - [ ] `registry.go` — 插件注册表
  - [ ] `template.go` — 模板定义
  - [ ] 内置插件: auth:login, auth:register, form:contact, provider:sms, provider:email, challenge:turnstile, challenge:recaptcha

### P1

- [ ] **4.3** `backend/internal/workflow/recorder.go`
  - [ ] CDP 事件→Steps 聚合
  - [ ] 步骤自动归类
- [ ] **4.4** `backend/internal/workflow/playback.go`
  - [ ] 工作流级别多步编排
  - [ ] 步骤间自然间隙计时

### P2

- [ ] **4.5** `backend/internal/workflow/provider/`
  - [ ] `gateway.go` — 统一 Provider 接口
  - [ ] `sms_adapter.go` — 短信适配器
  - [ ] `email_adapter.go` — 邮件适配器
  - [ ] `challenge_adapter.go` — 人机验证适配器
- [ ] **4.6** `backend/internal/workflow/form_mapper.go`
  - [ ] 字段映射定义
  - [ ] 批量表单填写执行器
  - [ ] 数据生成器（name/email/phone/password/address）
- [ ] **4.7** `backend/internal/launchcode/workflow_api.go`
  - [ ] `POST /api/workflow`
  - [ ] `POST /api/workflow/{id}/execute`
  - [ ] `GET /api/workflow/{id}/status`
  - [ ] `POST /api/plugin/install`
  - [ ] `POST /api/workflow/{id}/export`

---

## Phase 5: 扩展功能构建

**来源：** `docs/34—38`  
**估时：** 15d

### 5.1 环境参数样板采集

- [ ] **5.1.1** Collector JS probes（参数采集脚本）
- [ ] **5.1.2** `backend/internal/reference/` — 样板库存储引擎
- [ ] **5.1.3** `backend/internal/reference/recognizer.go` — 相似度匹配

### 5.2 跨浏览器会话迁移

- [ ] **5.2.1** 环境参数映射表（Chromium ↔ Camoufox ↔ Lightpanda）
- [ ] **5.2.2** `backend/internal/session/bundle.go` — Serialize/Deserialize + AES 加密
- [ ] **5.2.3** `backend/internal/session/migrator.go` — 跨运行时 Cookie/Storage/参数迁移

### 5.3 浏览器池管理

- [ ] **5.3.1** `backend/internal/pool/`
  - [ ] `engine.go` — 池引擎
  - [ ] `policy.go` — 池策略
  - [ ] `slot.go` — 槽位状态机
- [ ] **5.3.2** 预热流程（Profile→路由→启动→注入）
- [ ] **5.3.3** Acquire/Release API

### 5.4 页面结构自动发现

- [ ] **5.4.1** `backend/internal/discovery/`
  - [ ] `form_detector.go` — 表单检测
  - [ ] `field_analyzer.go` — 字段语义分析
  - [ ] `type_classifier.go` — 页面类型分类
  - [ ] `challenge_recognizer.go` — 验证机制识别
  - [ ] `flow_inferencer.go` — 流程推断
- [ ] **5.4.2** 发现结果→工作流模板转换器

### 5.5 传输层特征优化

- [ ] **5.5.1** `backend/internal/transport/`
  - [ ] `tls_profile.go` — TLS 握手参数模板库
  - [ ] `http2_profile.go` — HTTP/2 帧序随机化
  - [ ] `header_order.go` — 请求头顺序模板
- [ ] **5.5.2** Chromium 启动参数集成
- [ ] **5.5.3** Xray/Sing-Box 出站配置集成

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
