# Personal Pilot — 模块实现难度与技术栈评估

> 基于 docs/12-module-split-and-backlog.md 模块拆分方案，评估每模块的实现难度、推荐技术栈和潜在依赖。
> 日期: 2026-05-17

---

## 一、核心模块（当前已完善）

### 1.1 launchcode — REST API 服务层

| 维度 | 评估 |
|------|------|
| **难度** | 中等 (3/5) |
| **当前技术** | `net/http` + `net/http/httputil` (反向代理) |
| **推荐** | 保持 `net/http`，可考虑引入 `chi`/`gorilla/mux` 做路由参数 |
| **新增依赖** | 无需新增，保持 Go 标准库 + 已用的 `gorilla/websocket` |
| **实现路径** | 当前 15 个文件已完整，继续在现有 pattern 上添加 handler |

### 1.2 browser — 浏览器管理

| 维度 | 评估 |
|------|------|
| **难度** | 高 (4/5) |
| **当前技术** | CDP over WebSocket (`gorilla/websocket`), `os/exec` |
| **推荐** | 维持当前技术栈 |
| **新增依赖** | 无 |
| **说明** | 已 42 个文件，相对完善。Firefox 内核支持需要 `geckodriver`+ Marionette 协议，与 CDP 不同 |

### 1.3 proxy — 代理管理

| 维度 | 评估 |
|------|------|
| **难度** | 高 (4/5) |
| **当前技术** | `metacubex/mihomo`, `xray`/`sing-box` 外部进程, `gopkg.in/yaml.v3` |
| **推荐** | 维持当前技术栈 |
| **新增依赖** | BrightData/Oxylabs 集成需要各自 REST API 客户端 |
| **说明** | 已 22 个文件，桥接+解析+测速+健康+评分体系完整 |

### 1.4 behavior — 行为引擎

| 维度 | 评估 |
|------|------|
| **难度** | 高 (4/5) |
| **当前技术** | CDP + WebSocket + JS 注入 + Go 贝塞尔曲线算法 |
| **推荐** | 维持当前技术栈 |
| **说明** | 已 21 个文件，录制/回放/人性化中间件完整。需补全 `cdp_executor.go` 中 `MutatedScreenshot`/`GetHtml`/`GetText` 空实现 |

### 1.5 events — 事件系统

| 维度 | 评估 |
|------|------|
| **难度** | 低 (2/5) |
| **当前技术** | Go + 代码生成(gen_*.go) |
| **推荐** | 保持 |
| **说明** | 已完善，支持 ~30 事件类型 |

### 1.6 logger — 日志系统

| 维度 | 评估 |
|------|------|
| **难度** | 低 (2/5) |
| **当前技术** | 自定义 Go 日志库 |
| **推荐** | 保持 |
| **说明** | 12 个文件，带分级/格式化/分片/拦截器，已成熟 |

### 1.7 config — 配置管理

| 维度 | 评估 |
|------|------|
| **难度** | 低 (1/5) |
| **当前技术** | `gopkg.in/yaml.v3` |
| **推荐** | 保持 |

---

## 二、已存在但未接入 API 的模块

### 2.1 scheduler — 调度器 API 化

| 维度 | 评估 |
|------|------|
| **难度** | 低 (2/5) |
| **当前技术** | 纯 Go + CDP WebSocket |
| **推荐** | 保持当前实现 |
| **新增依赖** | 无 |
| **估计工时** | 2-3 天 |
| **实现路径** | 在 launchcode 包添加 handler → 调用 scheduler 的 AddTask/RunTaskNow/ListTasks → 返回 JSON |
| **风险** | 低，scheduler 包测试已覆盖 |

### 2.2 automation — 规则引擎 API 化

| 维度 | 评估 |
|------|------|
| **难度** | 低 (2/5) |
| **当前技术** | 纯 Go，条件评估器(比较/包含/嵌套字段) |
| **推荐** | 保持 |
| **新增依赖** | 无 |
| **估计工时** | 2-3 天 |
| **实现路径** | launchcode 新增 handler CRUD → 绑定 RuleStore → 暴露 Evaluate 方法 |
| **风险** | 低，需要接入事件总线与 scheduler 联动 |

### 2.3 email — 临时邮箱接入自动化

| 维度 | 评估 |
|------|------|
| **难度** | 中 (3/5) |
| **当前技术** | REST API 客户端(mail.tm + Cloudflare Worker) |
| **推荐** | 保持当前实现 |
| **新增依赖** | 无 |
| **估计工时** | 3-5 天 |
| **实现路径** | 创建邮箱 → 自动填写注册页 → 轮询验证码 → 自动填充验证码 |
| **风险** | 低，mail.tm/Cloudflare Worker 是免费服务，有速率限制 |

### 2.4 backup — 备份 API 化

| 维度 | 评估 |
|------|------|
| **难度** | 低 (2/5) |
| **当前技术** | 加密+规格+预检 |
| **推荐** | 保持 |
| **新增依赖** | 无 |
| **估计工时** | 1-2 天 |

### 2.5 webhook — Webhook API 化

| 维度 | 评估 |
|------|------|
| **难度** | 低 (1/5) |
| **当前技术** | HTTP POST |
| **推荐** | 保持 |
| **新增依赖** | 无 |
| **估计工时** | 1 天 |

### 2.6 wininput — Windows 原生输入集成

| 维度 | 评估 |
|------|------|
| **难度** | 中 (3/5) |
| **当前技术** | Windows API (user32.dll) via `golang.org/x/sys/windows` |
| **推荐** | 保持 |
| **新增依赖** | 无 |
| **说明** | 仅 Windows 可用，处理无 CDP 时的原生弹框(文件上传/保存对话框等) |

---

## 三、API 缺失补全（按优先级的实现分析）

### P0 组（核心缺失，~18 项）

| 端点 | 难度 | 工时 | 关键依赖 | 实现要点 |
|------|------|------|----------|----------|
| 代理订阅 CRUD | 低 | 2d | proxy_dao | 已有 app_proxy_subscription.go，加 handler 即可 |
| 手动代理添加 | 低 | 1d | proxy.parseStandardProxy | domain+port+username+password → URL 拼接 |
| 代理快捷导入 | 中 | 2d | parser.go | 智能识别格式(URL/Clash/Base64) |
| 订阅验证/预览 | 中 | 1d | proxy | 拉取+解析但不写入 DB |
| 配置切换代理 | 低 | 0.5d | proxy_binding | 已有 BindProfileToProxy |
| 代理列表/编辑 | 低 | 1d | proxy_dao | 简单 CRUD |
| 单节点测速/健康 | 低 | 1d | proxy.SpeedTest/FetchProxyIPInfo | 已有实现，加 HTTP 包装 |
| 批量测速/健康 | 低 | 1d | 同上 + goroutine 池 | 并发控制 |
| 分组 CRUD | 低 | 1d | group_dao | 简单 CRUD |
| 配置克隆 | 低 | 0.5d | browser.profile | 已有 CopyProfile |
| 设置 GET/PUT | 低 | 1d | config | 读/写 config.yaml |
| **P0 合计** | - | **~12d** | - | 2-3 周 |

### P1 组（核心增强，~18 项）

| 端点/功能 | 难度 | 工时 | 关键依赖 | 实现要点 |
|-----------|------|------|----------|----------|
| SSE 事件流 | 中 | 3d | events | `/api/events/stream` → `text/event-stream` + 事件订阅 |
| Webhook CRUD + 事件绑定 | 低 | 2d | webhook | 4 个端点 + 事件总线桥接 |
| 录制导出/导入 | 低 | 1d | recording_store | JSON 序列化/反序列化 |
| 系统信息/统计 | 低 | 1d | runtime/metrics | PID/内存/磁盘/运行实例数 |
| 备份触发 | 低 | 1d | backup | 已有实现 |
| 配置热重载 | 中 | 2d | config | SIGHUP 或监听文件变化 |
| 浏览器崩溃恢复 | **高** | **5d** | browser+behavior | CDP 断开检测 → 自动重启 → 恢复标签页 |
| 代理自动故障转移 | 中 | 3d | proxy.selector | 监听 bridge died 事件 → 选备用节点 |
| SOCKS5 认证修复 | 中 | 2d | proxy.http_client | 审计认证传递链路 |
| 测速缓存 | 低 | 1d | proxy.speedtest | LRU cache + TTL |
| 指纹基线校准 | 中 | 3d | browser.fingerprint | CDP → 提取指纹 → 对比预期 |
| WebRTC 泄漏检测 | 中 | 2d | browser | CDP evaluate + 导航到检测页 |
| 时区-IP 校验 | 低 | 1d | browser.geo_mismatch | 已有 geo_mismatch.go |
| SQLite 迁移 | 中 | 2d | database | golang-migrate 或 goose |
| 敏感字段加密 | 中 | 3d | backup.encrypt | AES-GCM + key 管理 |
| 请求体限流统一 | 低 | 1d | launchcode | 审计所有 handler + io.LimitReader |
| 并发安全测试 | 中 | 2d | - | go test -race |
| **P1 合计** | - | **~35d** | - | 7 周 |

### P2 组（差异化能力，~35 项）

| 端点/功能 | 难度 | 工时 | 关键依赖 | 实现要点 |
|-----------|------|------|----------|----------|
| 账号箱 | 中 | 3d | credstore | 加密存储+自动填充+账号轮换 |
| 录制编辑(merge/trim/diff) | 中 | 3d | behavior | 事件序列操作 |
| 代理质量看板 | 中 | 3d | trust_score | 持久化+聚合查询 |
| Cookie 健康看板 | 中 | 3d | cookie_verify | 持久化+有效期检查 |
| Cookie 自动续期 | **高** | 5d | browser | 导航→检测登录态→重新获取 |
| Firefox 内核 | **极高** | **15d** | - | geckodriver + Marionette 协议完全不同 |
| Lightpanda 深度集成 | 中 | 5d | browser.lightpanda | 已有一半 |
| 规则引擎 API | 低 | 2d | automation | CRUD + 事件绑定 |
| 工作流编排 | **高** | **10d** | scheduler | DAG 定义+执行引擎 |
| 代理链路多跳 | 中 | 3d | xray/singbox | 链式 outbound 配置 |
| TLS 指纹随机化 | 中 | 3d | xray/singbox | uTLS 库配置 |
| 目标站点专项测速 | 中 | 2d | proxy.speedtest | 用户自定义测速 URL |
| 代理预热 | 中 | 2d | proxy | 预创建桥接 |
| 粘性会话 | 低 | 1d | proxy | 最小绑定时间 |
| Canvas 噪声策略 | 中 | 2d | behavior.inject_script | 不同噪声算法 |
| 验证码打码集成 | 中 | 3d | - | 2Captcha/Capsolver API |
| SMS 验证码集成 | 中 | 3d | - | Twilio/5sim API |
| 临时邮箱自动化 | 中 | 5d | email | 注册流程闭环 |
| 实例快照 | **高** | **10d** | browser | 全状态保存+恢复 |
| Bot 检测仿真 | 中 | 3d | behavior | CDP 导航到检测站+结果解析 |
| 端到端测试 | **高** | **8d** | - | 真实浏览器+Playwright |
| fuzz 测试 | 中 | 3d | parser | go-fuzz |
| 错误注入测试 | 中 | 2d | - | 模拟各种故障 |
| **P2 合计** | - | **~95d** | - | 4-5 月 |

### P3 组（高级编排，~15 项）

| 端点/功能 | 难度 | 工时 | 关键依赖 | 实现要点 |
|-----------|------|------|----------|----------|
| 批量操作模板 | 中 | 5d | - | 模板定义+参数注入+执行 |
| 工作流条件分支 | **高** | 8d | scheduler | DAG+条件评估 |
| 操作审计 | 中 | 3d | - | 持久化+查询+哈希链 |
| 行为模式随机化层级 | 中 | 3d | humanize | 可配置变异参数 |
| 情绪感知行为 | **高** | 5d | humanize | 页面内容分析+行为调整 |
| 高级指纹伪造(WebGPU等) | 中 | 5d | behavior | CDP 注入+命令行参数 |
| 远程协作 | **极高** | **15d** | - | RBAC+WebSocket+CDP共享 |
| HTTPS 支持 | 中 | 3d | - | TLS 证书+配置 |
| WebSocket 事件流 | 中 | 3d | events | ws 升级+事件订阅 |
| 外部系统集成 | 低 | 3d | webhook | 适配器模式 |
| **P3 合计** | - | **~55d** | - | 2-3 月 |

### P4+ 组（团队企业+AI+平台，~20 项）

| 端点/功能 | 难度 | 工时 | 关键依赖 | 实现要点 |
|-----------|------|------|----------|----------|
| 多用户 RBAC | **极高** | **20d** | - | 用户系统+角色+权限 |
| LDAP/SSO | 高 | 10d | - | OIDC/LDAP 协议 |
| 住宅代理 API | 中 | 5d | - | 多供应商适配器 |
| LLM 编排 | **高** | 15d | - | 提示工程+CDP 执行 |
| 代理成功率模型 | **极高** | **20d** | ML | 特征工程+模型训练+部署 |
| ARM64/K8s | 中 | 5d | - | 交叉编译+Helm chart |
| **P4+ 合计** | - | **~100d** | - | 5-6 月 |

---

## 四、技术栈推荐总结

### 后端（当前 Go + 推荐新增）

| 场景 | 当前栈 | 推荐 | 理由 |
|------|--------|------|------|
| HTTP 路由 | `net/http` ServeMux | 保持或加 `chi` (轻量) | 当前 ~45 端点够用，新加 60+ 后路由管理复杂 |
| 数据库迁移 | 无 | `golang-migrate/migrate` | 最轻量，Go 原生，支持 SQLite |
| WebSocket | `gorilla/websocket` | 保持 | 已稳定使用 |
| SSE | 无 | Go 标准库 | 无需第三方，Content-Type: text/event-stream |
| 限流 | 自定义 | 保持 | 已实现 |
| 加密 | 自定义 | 保持 | AES-GCM 已实现 |
| 定时任务 | 自定义 scheduler | 保持 | 已实现完整 |
| ML/推理 | 无 | `onnxruntime-go` | 代理成功率预测模型的轻量推理 |
| CI/CD | 无 | GitHub Actions | 自动测试+构建 |

### 推荐新增 Go 依赖

```
golang-migrate/migrate  — SQLite 数据库迁移
chi 或 labstack/echo    — HTTP 路由(如果端点超 80+)
microsoft/onnxruntime   — ML 模型推理(可选)
```

### 前端（当前 Tauri 2 + React + Vite + TypeScript）

| 场景 | 评估 |
|------|------|
| **当前栈** | Tauri 2 + React 18 + Vite + TypeScript + Tailwind + Zustand |
| **推荐** | 保持当前栈 |
| **新依赖建议** | `recharts`(图表), `react-flow`(工作流编排可视化), `react-query`(API 状态管理) |

### 平台支持

| 平台 | 当前 | 评估 |
|------|------|------|
| **Windows** | ✅ 完整支持 | wininput/窗口控制/系统托盘/残留进程清理 |
| **macOS** | ⚠️ 部分 | 启动桥接缺少 macOS 窗口控制集成测试 |
| **Linux** | ⚠️ 部分 | 需要 Xvfb/Xorg 支持 headless |
| **ARM64** | ❌ 未支持 | 交叉编译 + 浏览器核心 ARM64 构建 |
| **Docker** | ❌ 未支持 | 需 Xvfb + VNC + 预下载内核 |

---

## 五、各模块技术复杂度热力图

```
                 实现难度        风险        业务价值       推荐优先级
launchcode        ████▎        ██          ██████         ——
browser           █████        ████        ██████         ——
proxy             █████        ████        ██████         ——
behavior          █████        ████        ██████         ——
─────────── API 缺失补充分割线 ───────────
scheduler API    ██▎          █           ██████         P0-P1
automation API   ██▎          █           █████▎         P1-P2
email 自动化     ████         ██          ████▎          P2
backup API       ██           █           ████           P1
webhook API      █            █           ████           P1
wininput 集成    ████         ██          ███            P2
代理订阅管理     ██           ██          ██████         P0
配置 CRUD        ██           ██          ██████         P0
Cookie 自动续期  █████        ████        ██████         P2
浏览器崩溃恢复   █████        ████        ██████         P1
Firefox 内核     ████████     ██████      ████           P2
LLM 编排         ██████       █████       ██████         P4+
多用户/RBAC      ██████       █████       ██████         P4+
代理成功率模型   ███████      ██████      ██████         P4+
实例快照         ██████       █████       ██████         P2
工作流编排       ██████       █████       ██████         P3
TLS/HTTP2 指纹   ████         ███         ████           P2
Bot 检测仿真     ████         ███         █████          P2
验证码打码集成   ███          ██          ██████         P2
```

## 六、推荐实施路线

### 立即开始（6 周内可完成）

```
第1-2周: P0 API 缺失
  ├── 代理订阅 CRUD + 手动代理添加 + 快捷导入 + 订阅验证 (3d)
  ├── 配置 CRUD 完善 + 克隆 + 分组管理 (2d)
  └── 代理列表/编辑 + 单节点测速/健康 + 批量操作 (3d)

第3-4周: P0->P1 API
  ├── 调度器 API + 备份 API + Webhook API (3d)
  ├── 系统信息/统计 + 设置 GET/PUT (2d)
  ├── 录制导出/导入 (1d)
  └── CDPExecutor 空实现补全 + 限流统一 (2d)

第5-6周: P1 质量改进
  ├── 测速缓存 + 代理自动故障转移 (4d)
  ├── SQLite 迁移框架 (2d)
  ├── 敏感字段加密 (2d)
  └── 并发安全测试 + 请求体限流收尾 (2d)
```

### 中期（3-6 月）

```
第7-12周: P1-P2 核心体验
  ├── 浏览器崩溃自动恢复 + 端口清理 (5d)
  ├── 指纹基线校准 + WebRTC 检测 + 时区-IP 校验 (5d)
  ├── 规则引擎 API + 事件流 SSE (5d)
  ├── 代理链路多跳 + TLS 指纹 (5d)
  ├── Cookie 健康看板 + 自动续期 (5d)
  └── 端到端测试 + fuzz 测试 (5d)

第13-20周: P2 差异化能力
  ├── 实例快照与恢复 (8d)
  ├── 代理质量看板 + 信任评分持久化 (5d)
  ├── 录制编辑 + 分析 + 差异对比 (5d)
  ├── 验证码打码 + SMS + 临时邮箱集成 (8d)
  ├── Bot 检测仿真 (3d)
  └── Canvas 噪声 + 字体白名单 + 渐变指纹 (5d)
```

### 长期（6-12 月）

```
第21-32周: P3 高级
  ├── 工作流编排 + 条件分支 (10d)
  ├── 操作审计 + 防篡改审计日志 (5d)
  ├── 高级指纹伪造(WebGPU/AudioContext等) (5d)
  ├── 行为模式随机化 + 情绪感知 (5d)
  ├── 批量操作模板 (5d)
  └── 远程协作 + RBAC (15d)

第33周+: P4+ 
  ├── 住宅代理市场 API (5d)
  ├── LLM 编排 (15d)
  ├── 代理成功率预测模型 (20d)
  ├── ARM64/K8s (5d)
  └── 插件系统 + SDK 生成 (15d)
```

---

## 七、技术风险清单

| 风险 | 影响 | 概率 | 缓解 |
|------|------|------|------|
| Firefox 内核协议完全不同(Marionette vs CDP) | 极高工作量 | 中 | 评估是否真的需要，或只用 Chromium 模拟 Firefox UA |
| 代理成功率预测 ML 模型效果不可控 | 浪费投入 | 高 | 先做简单启发式规则，再逐步迭代 ML |
| 多用户 RBAC 影响整个架构 | 大量重构 | 中 | 先做 API Key 级别隔离，不做全量用户系统 |
| LLM 编排的可靠性 | 用户体验差 | 中 | 关键操作有确认步骤，LLM 只辅助编排 |
| Cookie 自动续期在 SPA 站点失效 | 功能不可用 | 高 | 先用规则引擎重试，标记失败站点 |
| 浏览器崩溃恢复导致数据丢失 | 数据安全 | 低 | 写时复制状态保持 |
