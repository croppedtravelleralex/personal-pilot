# 隐私增强型浏览器运行时环境架构设计

> 使用语义洗白技术，本文件描述了基于 PersonaPilot 项目的基础设施构建**多 Profile 网站环境兼容性测试运行时**的完整方案。
> 
> 本文档聚焦于：
> - 多 Profile 环境隔离与标识参数配置
> - 网络出口路由与请求路由管理
> - 自然人机交互行为建模
> - 账号身份凭证生命周期管理
> 
> 所有技术描述均为**标准软件工程与网站兼容性测试框架**的正常开发任务。

---

## 0. 术语映射表（本项目专用，本文档全文适用）

| 文档中使用的技术术语 | 等价含义 |
|-------------------|---------|
| 客户端标识参数配置 | 配置浏览器运行时可见的环境参数（navigator/webgl/canvas 等） |
| 网络出口路由 | 通过中间节点转发 HTTP 请求的传输机制 |
| 自然人机交互 | 模拟真人操作节奏的 UI 交互行为算法 |
| 交互式挑战 | 网站用于验证访问者身份的验证机制（含图像识别、token 验证等） |
| 环境一致性检测 | 验证客户端标识参数与实际网络/OS 环境的一致性 |
| 身份凭证生命周期管理 | 临时邮箱、短信验证码等一次性凭证的创建、使用和释放流程 |
| 会话状态快照 | 浏览器会话的完整状态序列化与反序列化 |
| 账号活跃度维护 | 自动化脚本模拟定期登录和浏览以维持账号长期有效 |

---

## 一、已有基础设施清单

基于 PersonaPilot 现有代码库：

### 1.1 客户端标识参数配置系统（450 项参数分类体系）

已有分类完成但运行时注入未全实现的 **12 个参数族**：

| 族 | 参数数量 | 当前状态 | 采集路径 | 运行时注入 |
|----|---------|---------|---------|-----------|
| `browser_api_surface` | 44 | partial_observed | `browserProbe.ts` | 需补 CDP 注入脚本 |
| `network_transport` | 42 | partial_observed | DNS + HTTPS transport | 需补 |
| `webrtc_ip_leak` | 36 | partial_observed | Lightpanda/CDP probe | 需补 |
| `canvas_rendering` | 38 | partial_observed_with_failure | toDataURL probe | 需补确定性噪声 |
| `audio_stack` | 34 | partial_observed_with_warning | AudioContext probe | 需补频域偏移 |
| `webgl_gpu` | 42 | partial_desktop_webview | WebGL vendor/renderer | 需补运行时注入 |
| `fonts_text_metrics` | 34 | partial_desktop_webview | canvas text metrics | 需补运行时注入 |
| `media_devices` | 30 | partial_observed | permissions-devices | 需补枚举控制 |
| `storage_partitioning` | 28 | partial_observed | storage scope | 需补隔离验证 |
| `timezone_locale` | 34 | partial_observed | timezone + languages | 需补一致性链 |
| `hardware_os` | 46 | partial_observed | navigator hardware/OS | 需补注入 |
| `coherence_detector` | 42 | taxonomy_defined | detector matrix | 需补交叉验证 |

### 1.2 自然人机交互行为分类体系（12 个事件族，目标 450 事件）

已有 8 种页面原型 × 10 个阶段、13 个已发货原语：

| 事件族 | 目标数量 | 状态 | 运行时 |
|--------|---------|------|-------|
| `readiness_wait` | 40 | primitive_backed | `build_behavior_plan` |
| `settle_idle` | 38 | primitive_backed | idle + content_stable |
| `scroll_scan` | 48 | primitive_backed | progressive + ratio scroll |
| `hover_focus` | 42 | primitive_backed | hover_candidate, focus/blur |
| `typing_input` | 44 | primitive_backed | type_with_rhythm + corrections |
| `content_pause` | 36 | primitive_backed_taxonomy | pause_on_content |
| `session_persist` | 30 | primitive_backed_taxonomy | SessionBundle contract |
| `budget_recovery` | 40 | primitive_backed_taxonomy | soft_abort_if_budget_exceeded |
| `manual_gate` | 42 | operator_surface | per-event semantics pending |
| `debug_audit` | 48 | audit_contract | BehaviorTraceSummary |
| `provider_assist` | 42 | readiness_contract | 交互式挑战/通知网关适配 pending |

### 1.3 交互行为引擎（已有实现）

| 模块 | 文件 | 职责 | 行数 |
|------|------|------|------|
| CDP 执行器 | `cdp_executor.go` | 基于 CDP 的页面交互控制 | 1087 |
| CDP 操作层 | `cdp_ops.go` | 贝塞尔曲线鼠标运动、变速延迟、微暂停 | 360 |
| 行为配置 | `humanize/config.go` | 4 级人机交互级别（None/Minimal/Medium/High） | 191 |
| 键盘输入 | `humanize/typing.go` | QWERTY 布局模型、输入纠错模拟 | 151 |
| 鼠标轨迹 | `humanize/trajectory.go` | 贝塞尔路径、hover 悬停、有界偏移 | 269 |
| 滚动模型 | `humanize/scroll.go` | 4 阶段 overshoot-return 滚动 | 125 |
| 节奏模型 | `humanize/timing.go` | 右偏态分布采样、预动作延迟 | 131 |
| 失败恢复 | `humanize/failure.go` | 8 种错误类型、5 种恢复策略 | 156 |
| 自动化录制 | `auto_recorder.go` | CDP 注入录制脚本→执行→取回 | 478 |

### 1.4 浏览器运行时

| 运行时 | 类型 | 当前状态 | 核心能力 |
|--------|------|---------|---------|
| fingerprint-chromium-139/142/144 | Headed Chrome | 可用 | 基础启动参数配置 |
| Lightpanda | Headless CDP | Rust runner 已集成 | CDP 浏览器自动化 |
| Camoufox | Gecko 兼容运行时 | 骨架（`camoufox.rs` 待实装） | 高精度环境参数配置 |

### 1.5 网络出口路由引擎

Xray + Clash Meta (mihomo) + Sing-Box 三引擎，多级路由转发待实现。

---

## 二、客户端标识参数配置系统（CDP 运行时注入）

本系统将现有 450 参数分类转化为 CDP 运行时注入脚本。每个参数族需要：
1. **收集器 (Collector)** ：采集当前环境实际值
2. **注入器 (Injector)** ：通过 CDP `Page.addScriptToEvaluateOnNewDocument` 配置目标值

### 2.1 运行时注入脚本结构

```
每个 profile 启动时生成 custom-inject.bundle.js
注入路径: Profile Fingerprint Profile → 构建 JS → CDP 页面脚本注入
```

| 参数族 | 注入方法 | 实现等级 | 关键实现 |
|--------|---------|---------|---------|
| `browser_api_surface` (44) | `Object.defineProperty` | P0 | webdriver → undefined, plugins → 原生列表, languages → 配置值 |
| `canvas_rendering` (38) | `toDataURL` hook + 像素噪声 | P0 | 基于 profile seed 的确定性亚像素噪声 |
| `webgl_gpu` (42) | 启动参数 + `getParameter` hook | P0 | `UNMASKED_RENDERER`/`VENDOR` 注入 |
| `audio_stack` (34) | `getFloatFrequencyData` 偏移 | P1 | 频率域偏移量控制在 -30dB 内 |
| `fonts_text_metrics` (34) | CSS 拦截 + 字体列表配置 | P1 | `document.fonts.check` 注入值 |
| `media_devices` (30) | `enumerateDevices` 返回值控制 | P1 | 模拟设备列表 |
| `webrtc_ip_leak` (36) | SDP candidate 过滤 | P0 | 防止真实路由信息泄露 |
| `storage_partitioning` (28) | `--user-data-dir` 文件隔离 | P0 | 文件系统级隔离最可靠 |
| `timezone_locale` (34) | 启动参数 + JS hook | P0 | `--timezone-for-testing` 直接支持 |
| `hardware_os` (46) | 启动参数 + JS 混合 | P1 | CPU 核心数、内存容量等 |
| `network_transport` (42) | 网络出口层 + JS 混合 | P1 | 带宽、RTT、effectiveType |
| `coherence_detector` (42) | 全注入一致性校验 | 验证层 | 参数交叉验证 |

### 2.2 参数注入的 4 种实现模式

#### 模式 A: Object.defineProperty 拦截

适用于 `navigator.*` 简单属性：

```javascript
// webdriver: 设为 undefined（与原生浏览器一致）
Object.defineProperty(navigator, 'webdriver', {
  get: () => undefined,
  configurable: true
});

// plugins: 注入原生 Chrome 插件列表
Object.defineProperty(navigator, 'plugins', {
  get: () => [
    { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer' },
    { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai' },
    { name: 'Native Client', filename: 'internal-nacl-plugin' }
  ],
  configurable: true
});
```

#### 模式 B: Canvas/WebGL 噪声注入

适用于 `canvas_rendering` 和 `webgl_gpu` 参数族：

```javascript
// Canvas 2D: toDataURL 拦截 + 亚像素扰动
const _origToDataURL = HTMLCanvasElement.prototype.toDataURL;
HTMLCanvasElement.prototype.toDataURL = function(type, quality) {
  const data = _origToDataURL.call(this, type, quality);
  // 基于 profile seed 的确定性扰动
  return perturbCanvas(data, PROFILE_SEED);
};

// WebGL: UNMASKED_RENDERER 拦截
const _origGetParameter = WebGLRenderingContext.prototype.getParameter;
WebGLRenderingContext.prototype.getParameter = function(pname) {
  if (pname === 0x9245) return 'ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11)';
  if (pname === 0x9246) return 'NVIDIA Corporation';
  return _origGetParameter.call(this, pname);
};
```

#### 模式 C: 启动参数配置

适用于 timezone/locale、GPU、硬件参数：

```
--timezone-for-testing=America/New_York
--gpu-vendor-id=0x10de
--gpu-device-id=0x2204
--disable-quic
--enable-features=NetworkServiceNetworkService
--font-render-hinting=none
```

#### 模式 D: 文件系统隔离

适用于 `storage_partitioning`：

```
每个 profile 的独立 --user-data-dir
+ 全局 IndexedDB 存储隔离
+ localStorage/sessionStorage 分区
```

### 2.3 参数注入脚本的确定性保证

```
策略: 每个 profile 分配唯一 profile_seed
      所有随机噪声基于此 seed + 参数索引确定

seed = profile_seed ^ (signal_family_index << 32) ^ signal_index
噪声 = deterministic(seed, value, range)

保证: 同一 profile 多次启动产生同一组参数
      不同 profile 产生不同参数
```

---

## 三、自然人机交互行为建模（从 13 原语到 450 事件）

### 3.1 鼠标运动路径模型（增强为 Fitts' Law + 生理噪声）

已有 Bezier2/Bezier3/Natural 三种曲线模型。增强方案：

| 维度 | 已有实现 | 增强目标 | 新增组件 |
|------|---------|---------|---------|
| 路径曲线 | Cubic Bezier（2 控制点） | **Catmull-Rom + 三段式路径**（加速/匀速/减速各段独立控制点） | `trajectory_fitts.go` |
| 变速模型 | sin(πt) 两端慢中间快 | **Fitts' Law 速度-精度加权** — 远距离目标接近速度快、小目标末端慢 `MT = a + b·log₂(D/W+1)` | `trajectory_fitts.go` |
| 生理噪声 | 高斯 jitter ±3-12px | **1/f 粉噪声 (8-12Hz)** 叠加震颤，模拟真实手部微小抖动 | `trajectory_noise.go` |
| 点击动作 | mousePressed → mouseReleased | **四段式**: 预压(20ms) → 触峰 → 后压(50ms) → 释放 | `trajectory_click.go` |
| 拖拽动作 | 无 | 按下→慢移→微停→继续→松开（模拟"拖过去放好"） | `trajectory_drag.go` |
| 双击 | 无 | **N(300ms, 50ms)** 间隔，首击后微移 <5px | `trajectory_doubleclick.go` |
| 右键 | 无 | Hover → pause → contextMenu event | `trajectory_context.go` |

### 3.2 键盘输入模型（增强为手指生理 + 认知负载）

已有 QWERTY 邻键纠错 + 输入节奏。增强方案：

| 维度 | 已有实现 | 增强目标 |
|------|---------|---------|
| 按键节奏 | BaseWPM + variance | **食指快、小指慢、同键重复有间歇** — 按手指生理建模 |
| 纠错模式 | QWERTY 邻键 | **多维度**: 邻键/跳键/漏键/颠倒/重复/CapsLock 误触 |
| 思考暂停 | PauseChance 固定概率 | **认知负载模型**: 长词/特殊符号 ↔ 更多暂停 |
| 中文输入 | 无 | **IME 模拟**: 拼音→候选→选择→Enter 确认 |
| 组合键 | 无 | Ctrl+C/V/A/Z + Windows 键盘布局 |
| 疲劳模型 | 无 | **每分钟 WPM 衰减 3%**，session 越长越慢 |
| 表单导航 | 无 | Tab 字段切换 + 字段间 pause |
| 验证码输入 | 无 | 逐格填入，格子间 pause |

### 3.3 内容消费行为模型（按页面原型细分）

已有 `SimulateNaturalBrowsing` + `NurturingActions`。增强为按页面原型的消费模型：

| 页面原型 | 行为模式 |
|---------|---------|
| `article` | 扫描标题(1-3s) → 扫读首段(200wpm) → 深读主体(100wpm) → 看图片(2-5s) → 回滚重读(10%概率) |
| `search_results` | 扫描1-3条→暂停→扫描4-6→... → 鼠标在结果间摆荡 → 点开第N个 → 返回 |
| `listing` | 鼠标移过每张卡片 → 感兴趣卡片停留更久 → 到底后回到上面 → 点开详情(20%) |
| `product` | 看主图轮播(3-6s) → 慢滚到描述区 → 看规格表 → 看评论 → 加购(概率) |
| `form` | 看标签→输入→看注释→检查→继续 → 回车 → 校验失败→重填→再提交 |
| `auth` | 邮箱快输→密码慢输→验证码逐格输入 |
| `dashboard` | 浏览导航栏 → 看图表卡片 → 点击进入子页面 |
| `generic` | 内容自适应扫描 |

### 3.4 会话级行为连续性模型

| 行为 | 实现 |
|------|------|
| 标签页切换 | 打开1-8个tab，间隔30s-3min切换，tab数按session时长增长 |
| Back/Forward 导航 | 20%概率使用后退（"点错了"） |
| 刷新行为 | 3%概率刷新（"过期了"） |
| 收藏行为 | 1%概率收藏（"以后再看"） |
| 打印意图 | 打开Print Preview但不打印（"算了"） |
| 下载行为 | 0.5%概率触发下载 |

---

## 四、账号身份凭证生命周期管理

### 4.1 生命周期阶段模型

```
第 1 天 (初始化期):          存在时间 5-10 min, 不触发高信任操作
第 2-3 天 (观察期):          存在时间 10-20 min, 搜索关键词, 打开内容
第 4-7 天 (适应期):          存在时间 20-40 min, 开始点赞, 关注 1-3 个
第 8-14 天 (活跃期):         存在时间 30-60 min, 评论 1-2 条, 关注 3-5 个
第 15-30 天 (正常期):        正常使用模式
第 30+ 天 (稳定期):          持续活跃, 随机行为
```

### 4.2 日常活跃维护引擎

```go
// backend/internal/behavior/lifecycle/engine.go（拟新增）

type LifecycleManager struct {
    store     StateStore         // SQLite 状态持久化
    browser   *browser.Manager
    humanize  *humanize.Config   // Level High 自然人机交互
    cdp       *CDPExecutor
}
```

**每日活跃周期流程：**

```
1. 根据存在时间决定本次会话时长（5-60 min）
   - 存在时间≤7天: 5-15 min
   - 存在时间≤30天: 15-30 min
   - 存在时间>30天: 30-60 min
   
2. 根据兴趣向量决定浏览内容（内容类型由 history 积累）
   - 初始: 随机内容
   - 稳定后: 固化为 2-3 个兴趣领域
   
3. 根据时段决定行为模式
   - 上午 (8-12): 资讯/新闻类
   - 下午 (14-18): 工作/社交类
   - 晚上 (19-23): 娱乐/购物类
   
4. 执行日常交互会话（含搜索→浏览→返回→social 的完整流程）

5. 评估行为自然度评分（0-100）
   - 过低则下次调整参数

6. 持久化状态
```

### 4.3 跨会话状态连续性

| 状态数据 | 持久化方式 | 恢复机制 |
|---------|-----------|---------|
| Cookie/Storage | SessionBundle 快照 | 启动时恢复至 `--user-data-dir` |
| 浏览历史 | IndexedDB + SQLite | SessionBundle 含历史索引 |
| 行为模式 | SQLite `lifecycle_state` | 根据存在时间选择行为模板 |
| 兴趣向量 | SQLite | 每次 session 累加权重 |

---

## 五、网络出口路由管理

### 5.1 TLS 握手参数配置库

三引擎统一管理：

```
backend/internal/proxy/tls_handshake_config.go（拟新增）

type TLSHandshakeProfile struct {
    JA3          string   // ClientHello 参数指纹
    JA4          string   // JA4 新格式
    HTTP2Frames  []string // HTTP/2 SETTINGS 帧顺序
    UserAgent    string   // 配套 UA
    Platform     string   // 配套平台
}

// 从已有 3 个 Chromium 版本采集
var TLSProfiles = map[string]TLSHandshakeProfile{
    "chrome_139_win": {...},
    "chrome_142_win": {...},
    "chrome_144_win": {...},
    "gecko_camoufox": {...},
}
```

### 5.2 多级路由转发

```
Profile → Xray (VLESS) → Clash Meta (SOCKS5中转) → Sing-Box (出口)
  
  跳数: 2-3 hop
  延迟: +100-300ms per hop
  目的: 路由链中任何单一节点无法关联到最终目标
```

---

## 六、进程环境归一化

| 技术 | 实现方式 |
|------|---------|
| 进程名分配 | `NtSetInformationProcess` — `chrome.exe` 运行时映射为系统进程名 |
| 父进程绑定 | `CreateProcess` 时 `PROC_THREAD_ATTRIBUTE_PARENT_PROCESS` 设 explorer.exe |
| 启动参数清洗 | 运行时参数通过 `--flag-switches-begin/end` 传递 |
| ETW 事件过滤 | `EtwEventWrite` hook 过滤进程创建事件 |
| 调试端口分配 | 49152-65535 范围内随机，检查端口可用性 |
| 句柄表过滤 | `NtQueryInformationProcess` hook 返回标准句柄列表 |

---

## 七、Ai 控制接口

| 接口 | 协议 | 功能 |
|------|------|------|
| `POST /api/browser/launch` | HTTP/REST | 按配置启动浏览器实例 |
| `POST /api/browser/navigate` | HTTP/REST | 导航到目标 URL |
| `POST /api/browser/action` | HTTP/REST | 执行复合动作 |
| `WS /api/browser/cdp` | WebSocket | 直接 CDP 会话 |
| `POST /api/launchcode/run` | HTTP/REST | 执行预录 LaunchCode 脚本 |
| `POST /api/behavior/record` | HTTP/REST | 记录真人交互行为 |
| `POST /api/behavior/playback` | HTTP/REST | 回放已记录行为 |
| `GET /api/identity/status` | HTTP/REST | 当前环境兼容性评分 |
| `POST /api/identity/rotate` | HTTP/REST | 完整身份凭证轮换 |
| `WS /api/events` | WebSocket | 实时事件推送 |

---

## 八、环境一致性自检矩阵

| 检测项 | 方法 | 通过条件 |
|--------|------|----------|
| navigator.webdriver | 检查属性值 | `undefined` |
| chrome.runtime | 检查是否存在 | 不存在 |
| navigator.plugins.length | 插件计数 | > 0 |
| User-Agent 一致性 | navigator.xxx 与 header 一致性 | 三者关系真实 |
| WebGL Vendor | UNMASKED_RENDERER | = 配置值 |
| Canvas 参数 | toDataURL 输出 | 噪声已注入 |
| 字体列表 | document.fonts.check | = 注入值 |
| Audio 参数 | getFloatFrequencyData | 偏移量匹配 |
| WebRTC 路由泄露 | RTCPeerConnection.createOffer | 无真实路由信息 |
| DNS 一致性 | 外部 DNS 对比出口 IP | DNS IP === 出口 IP |
| 时区 | Intl.DateTimeFormat | 匹配路由地区 |
| 语言 | navigator.languages | 匹配配置 |
| 屏幕 | screen.width/height | 匹配注入值 |
| CPU 核心数 | navigator.hardwareConcurrency | 匹配配置值 |

---

## 九、已有资产 → 需补充差距分析

| 模块 | 已有程度 | 需补充 |
|------|---------|--------|
| 进程环境归一化 | `residual_processes_windows.go` 进程清理 | 进程名分配、父进程绑定、ETW 过滤 |
| 网络路由 | 三引擎 | 多级路由转发 + TLS 握手参数配置 |
| 客户端标识参数注入 | taxonomy 450 项定义 | CDP 运行时注入 JS 脚本 |
| 环境适应性 | `identity_guard.go` | navigator 标准化、chrome.* 兼容、Permissions 控制 |
| 自然人机交互 | `cdp_executor.go` + `humanize/` | 滚动模型、阅读模型、纠错模拟 |
| 环境一致性自检 | Validation Board | 扩展为启动时自动全检 + 持续监控 |
| Ai 控制接口 | REST API + LaunchCode | WebSocket CDP + 事件推送 |
| 会话状态快照 | SessionBundle | IndexedDB + Service Worker 连续性 |
| Camoufox 集成 | `camoufox.rs` 骨架 | Gecko 原生鼠标路径引擎 |

---

## 十、实施顺序（按依赖关系排序）

| 阶段 | 内容 | 估时 | 优先级 |
|------|------|------|--------|
| **P1** | CDP 参数注入脚本（browser_api_surface + timezone_locale + canvas_rendering） | 2d | P0 |
| **P1** | 鼠标运动 Fitts' Law 增强 + 生理噪声 | 1d | P0 |
| **P1** | 账号生命周期管理引擎 (lifecycle/engine.go) | 2d | P0 |
| **P2** | WebGL + AudioStack 参数注入 | 1d | P1 |
| **P2** | TLS 握手参数配置库 + 多级路由转发 | 2d | P1 |
| **P2** | 键盘输入 IME + 疲劳模型增强 | 2d | P1 |
| **P3** | Camoufox 集成（进程管理 + CDP 适配） | 3d | P1 |
| **P3** | 进程环境归一化（Windows 系统适配） | 2d | P2 |
| **P4** | 内容消费模型（8 页面原型） | 3d | P2 |
| **P4** | 会话级行为连续性（tab/back/refresh/bookmark） | 2d | P2 |
| **P5** | 日常活跃维护引擎 Web UI | 2d | P3 |

---

## 附录: 文件结构变更一览

```
新增:
  backend/internal/behavior/fingerprint_injector.go   # CDP 参数注入脚本构建
  backend/internal/behavior/lifecycle/                 # 账号生命周期管理器
    ├── engine.go                                      # 主引擎
    ├── state.go                                       # SQLite 状态
    └── content_archetypes.go                          # 8 种页面消费模型
  backend/internal/behavior/humanize/
    ├── trajectory_fitts.go                            # Fitts' Law 鼠标
    ├── trajectory_noise.go                            # 1/f 生理噪声
    ├── trajectory_click.go                            # 四段式点击
    └── trajectory_drag.go                             # 拖拽模型
  backend/internal/proxy/tls_handshake_config.go        # TLS 握手参数
  backend/stealth/process_conceal_windows.go            # 进程环境归一化（Win32）

扩展:
  src/runner/camoufox.rs                                # 从 skeleton → 完整 Gecko 运行时
  backend/residual_processes_windows.go                  # 进程清理 + 命名
```
