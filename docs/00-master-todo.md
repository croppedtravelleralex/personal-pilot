# Personal Pilot — 全部待办事项总清单

> 整合 docs/11-api-roadmap.md、docs/12-module-split-and-backlog.md、docs/13-tech-stack-and-effort.md 中所有待办。
> 共 **190+ 项**，按优先级 P0→P4+ 排列。

---

## P0 — 紧急修复与核心缺失（先做这些）

### 当前 Bug 修复

- [ ] **补全 CDPExecutor 空实现** — `cdp_executor.go:ExecuteMutatedAction` switch 中 `MutatedScreenshot`(type 5)、`MutatedGetHtml`(type 6)、`MutatedGetText`(type 7) 落入 `default: return nil` 静默跳过，需补全执行逻辑
- [ ] **`recording/play/stop` 端点仅在 `s.recording != nil` 时注册** — `server.go:L325-L327`，条件注册可能导致外部调用 404
- [ ] **去掉 mihomo 依赖** — 唯一用途 `adapter.ParseProxy` 测速可走现有 fallback `httpClientDelayTest()`，移除后间接依赖从 80+ 降到 20

### 代理订阅管理（API 缺失）

- [ ] `POST /api/proxy/subscribe` — 添加代理订阅源（URL + 自动刷新）
- [ ] `DELETE /api/proxy/subscribe/{id}` — 删除订阅源
- [ ] `POST /api/proxy/subscribe/{id}/refresh` — 手动刷新订阅
- [ ] `GET /api/proxy/subscribe/list` — 订阅源列表
- [ ] `GET /api/proxy/subscribe/{id}/nodes` — 订阅源下的所有节点
- [ ] `POST /api/proxy/subscribe/{id}/validate` — 验证订阅源可用性（预览不导入）
- [ ] `POST /api/proxy/subscribe/{id}/import-clash` — 从 Clash 配置文件导入

### 手动代理添加

- [ ] `POST /api/proxy/manual` — 添加手动代理（domain/port/username/password/protocol）
- [ ] `DELETE /api/proxy/manual/{id}` — 删除手动代理
- [ ] `PUT /api/proxy/manual/{id}` — 更新手动代理
- [ ] `POST /api/proxy/manual/batch` — 批量添加手动代理
- [ ] `POST /api/proxy/manual/{id}/test` — 测试手动代理连通性
- [ ] `POST /api/proxy/quick-add` — 智能识别并导入（URL/Clash/Base64）
- [ ] `POST /api/proxy/parse` — 解析任意格式代理配置并返回结构化信息

### 代理节点管理

- [ ] `GET /api/proxy/list` — 全部代理节点列表
- [ ] `PUT /api/proxy/{id}` — 编辑代理节点
- [ ] `POST /api/proxy/{id}/speedtest` — 手动触发单节点测速
- [ ] `POST /api/proxy/{id}/health` — 手动触发 IP 健康检测
- [ ] `POST /api/proxy/batch/speedtest` — 批量测速
- [ ] `POST /api/proxy/batch/health` — 批量 IP 健康检测

### 配置与分组管理

- [ ] `POST /api/profiles` — 创建配置（已有，需完善）
- [ ] `GET /api/profiles` — 配置列表（已有）
- [ ] `GET /api/profiles/{id}` — 配置详情（已有）
- [ ] `PUT /api/profiles/{id}` — 更新配置（已有）
- [ ] `DELETE /api/profiles/{id}` — 删除配置（已有）
- [ ] `POST /api/profiles/{id}/proxy` — 切换配置的代理
- [ ] `POST /api/profiles/{id}/clone` — 克隆配置
- [ ] `GET /api/groups` — 分组列表
- [ ] `POST /api/groups` — 创建分组
- [ ] `PUT /api/groups/{id}` — 编辑分组
- [ ] `DELETE /api/groups/{id}` — 删除分组
- [ ] `POST /api/groups/{id}/assign` — 批量分配配置到分组

### 系统基础

- [ ] `GET /api/cores` — 已安装浏览器内核列表
- [ ] `GET /api/settings` — 运行时设置
- [ ] `PUT /api/settings` — 更新设置

---

## P1 — 核心增强

### 基础设施接入（已有代码未暴露 API）

- [ ] **调度器 API 化** — `internal/scheduler` 已有完整调度引擎，暴露 CRUD + 手动触发 + 状态查询
- [ ] **规则引擎 API 化** — `internal/automation` 已有事件匹配/条件评估/动作执行，暴露 CRUD + 模拟触发
- [ ] **临时邮箱接入自动化** — `internal/email` 已有 mail.tm + Cloudflare Worker，接入注册流程
- [ ] **代理信任评分持久化** — `internal/proxy/trust_score.go` 模型已有，存 DB + 趋势查询
- [ ] **Webhook 接入事件总线** — `internal/webhook` 发送器已有，绑定内部事件
- [ ] **Windows 输入模拟集成** — `internal/wininput` 已有鼠标/键盘/坐标，处理无 CDP 弹框
- [ ] **备份加密 API** — `internal/backup` 已有加密/规格，暴露触发/恢复 API
- [ ] **Cookie 有效性验证接入** — `internal/browser/cookie_verify.go` 验证逻辑已有，暴露 API

### 事件与通知

- [ ] `GET /api/events/stream` — SSE 事件流（实时推送实例状态/代理健康/录制完成）
- [ ] `POST /api/webhook` — 配置 Webhook URL
- [ ] `GET /api/webhook` — 查看 Webhook 配置
- [ ] `DELETE /api/webhook` — 删除 Webhook

### 录制与系统

- [ ] `POST /api/recording/{id}/export` — 导出录制为 JSON
- [ ] `POST /api/recording/import` — 导入录制
- [ ] `GET /api/system/info` — 系统信息（版本/内存/磁盘）
- [ ] `GET /api/system/stats` — 运行时统计（运行实例数/代理数）
- [ ] `POST /api/system/backup` — 触发备份
- [ ] `POST /api/system/config/reload` — 热重载配置

### 代理系统改进

- [ ] **代理自动故障转移** — 当前代理不可用时自动切换到同组备用节点
- [ ] **SOCKS5 认证传递完善** — 部分代理链路认证信息不完整
- [ ] **测速结果本地缓存** — 短时间重复测速直接返回缓存
- [ ] **Xray/sing-box 优雅退出** — SIGKILL → 优先 SIGTERM 等待进程自退出
- [ ] **手动代理与订阅代理统一管理** — 两种来源在列表/测速/健康检测中统一对待

### 反检测改进

- [ ] **指纹基线自动校准** — 启动后比对预期指纹与实际指纹，差异大告警
- [ ] **WebRTC 泄漏检测** — 检测真实 IP 是否通过 WebRTC 泄漏
- [ ] **时区/IP 地理位置一致性校验** — 代理国家与浏览器时区不符时自动修正

### 数据与持久化

- [ ] **SQLite 数据库迁移框架** — 引入 golang-migrate 管理 schema 版本
- [ ] **敏感字段加密存储** — Cookie、代理密码等加密后存盘

### 浏览器改进

- [ ] **浏览器崩溃自动恢复** — CDP 断开时自动重启实例并恢复标签页
- [ ] **端口使用清理** — 异常退出后残留端口自动回收
- [ ] **健康检查端点增强** — 返回详细组件状态（CDP/代理/DB/存储）

### 安全加固

- [ ] **请求体大小统一限制** — 所有 POST/PUT 端点统一 `io.LimitReader`（部分已完成，需审计所有端点）

### 测试

- [ ] **CDPExecutor 补全后的单元测试** — Screenshot/GetHtml/GetText
- [ ] **并发安全性测试** — `go test -race`

---

## P2 — 差异化能力

### 数据采集

- [ ] `POST /api/scraper/task` — 定义采集任务（URL + 提取规则 + 导出格式）
- [ ] `GET /api/scraper/task/{id}/data` — 获取采集数据
- [ ] `POST /api/scraper/export` — 导出采集数据为 CSV/JSON
- [ ] **网络请求拦截** — 通过 CDP `Network.enable` 获取 SPA 页面背后的原始 API 数据

### 账号管理

- [ ] `POST /api/account/store` — 加密存储账号密码
- [ ] `GET /api/account/list` — 已存储账号列表
- [ ] `POST /api/account/{id}/autofill` — 自动填充登录表单

### 录制编辑

- [ ] `POST /api/recording/{id}/merge` — 合并多个录制
- [ ] `PUT /api/recording/{id}` — 重命名/编辑录制元数据
- [ ] `POST /api/recording/{id}/trim` — 裁剪录制事件范围
- [ ] `GET /api/recording/stats` — 录制库统计
- [ ] `POST /api/recording/{id}/analyze` — 录制操作模式分析
- [ ] `POST /api/recording/{id}/diff` — 两次录制差异对比

### 代理质量看板

- [ ] `GET /api/proxy/quality/summary` — 代理池质量概览
- [ ] `GET /api/proxy/quality/ranking` — 代理综合评分排名
- [ ] `GET /api/proxy/quality/history` — 代理历史趋势
- [ ] `GET /api/proxy/quality/geo-distribution` — 代理地理位置分布
- [ ] `POST /api/proxy/quality/auto-select` — 智能推荐最佳代理

### 代理绑定与流量

- [ ] `GET /api/proxy/binding/list` — 代理绑定关系
- [ ] `POST /api/proxy/binding` — 创建代理绑定
- [ ] `DELETE /api/proxy/binding/{id}` — 删除绑定
- [ ] `POST /api/proxy/traffic/{proxyId}` — 代理流量统计

### Cookie 增强

- [ ] `POST /api/workbench/cookies/import` — 批量导入 Cookie（JSON/Netscape）
- [ ] `POST /api/workbench/cookies/export` — 导出 Cookie
- [ ] `POST /api/workbench/cookies/transfer` — Cookie 跨实例转移
- [ ] `POST /api/workbench/cookies/verify` — 验证登录态有效性
- [ ] **会话健康看板** — 所有配置登录态存活/有效期
- [ ] **Cookie 自动续期** — 过期前自动导航重新获取
- [ ] **批量 Cookie 预热** — 导入后自动访问目标站种 Cookie
- [ ] **Cookie + LocalStorage/IndexedDB 统一快照** — SPA 站点完整会话迁移
- [ ] **加密会话导出** — 密码保护的会话包

### 指纹模板

- [ ] `GET /api/fingerprint/templates` — 指纹模板列表
- [ ] `POST /api/fingerprint/templates` — 创建指纹模板
- [ ] `POST /api/fingerprint/templates/{id}/apply` — 应用指纹模板
- [ ] `POST /api/fingerprint/randomize` — 随机生成推荐指纹
- [ ] `POST /api/fingerprint/compare` — 对比两个实例指纹

### 内核管理

- [ ] `POST /api/cores/download` — 下载安装新内核
- [ ] `DELETE /api/cores/{id}` — 删除内核

### 反检测深度改进

- [ ] **Canvas 噪声策略可配置** — 不同策略（加像素/改色值/随机方块）
- [ ] **WebGPU 指纹模拟** — 当前只采集不伪造
- [ ] **AudioContext 指纹伪造** — 防音频指纹跨配置关联
- [ ] **Navigator.permissions API 伪造** — 防止检测为自动化
- [ ] **Battery/NetworkInformation API 伪造** — 小众检测面补充
- [ ] **浏览器核心版本回退检查** — 新版本被识别时建议回退
- [ ] **Firefox 内核支持** — 不同指纹面，部分站点限定
- [ ] **移动端模拟增强** — 触控事件/viewport/GPU 深度模拟
- [ ] **Bot 检测仿真测试** — 自动测试 bot.sannysoft.com 等并生成报告
- [ ] **指纹渐变式变异** — 长期使用时指纹参数缓慢变化
- [ ] **TLS 指纹随机化(JA3/JA3S)** — 通过 xray/sing-box 随机化
- [ ] **HTTP/2 与 HTTP/3 指纹伪造** — Akamai/Cloudflare 用此识别
- [ ] **字体白名单/黑名单** — 防字体指纹跨配置关联

### 录制回放改进

- [ ] **录制事件去重与合并** — 连续相同操作合并
- [ ] **回放暂停/继续** — 用户可手动介入
- [ ] **回放步骤预览** — 回放前展示操作序列
- [ ] **录制自动标注关键节点** — 标记验证码/登录/支付等
- [ ] **回放失败智能重试** — 元素未找到时等待重试
- [ ] **回放异常检测** — 检测到验证码/弹窗时暂停告警

### 代理深度增强

- [ ] **住宅代理市场 API 集成** — BrightData/Oxylabs/IPRoyal 对接
- [ ] **代理链路多跳(Proxy Chain)** — 入口→中转→出口
- [ ] **粘性会话管理** — 固定出口 IP 至少 N 分钟
- [ ] **代理预热** — 预测即将使用的代理提前建连
- [ ] **目标站点专项测速** — 针对用户指定 URL 测速
- [ ] **代理成本追踪** — 流量/时间/ROI 分析
- [ ] **SOCKS4/SOCKS4a 协议** — 协议兼容性补充
- [ ] **代理 IP 变更自动发现** — 域名解析变化自动更新
- [ ] **端口扫描/健康雷达** — 主动扫描池中 IP:PORT
- [ ] **代理订阅自动刷新失败告警** — 订阅源过期通知

### 浏览器增强

- [ ] **Lightpanda 无头内核完善** — 深度集成，比 Chromium 轻 10 倍
- [ ] **多内核并行启动优化** — 预热池
- [ ] **GPU/WebGL 黑名单检测** — 环境检测
- [ ] **窗口批量排列预设** — cascade/tiled/horizontal 布局
- [ ] **多屏/高 DPI 适配** — 多显示器场景
- [ ] **启动参数冲突检测** — 指纹参数 vs 用户参数冲突告警

### 数据与持久化

- [ ] **旧数据自动清理** — 录制/日志/测速历史阈值清理
- [ ] **导出格式统一** — 配置/录制/日志统一格式
- [ ] **录制文件增量存储** — 当前全量存储，大文件优化

### 自动化与规则引擎

- [ ] **规则条件支持时间窗口** — 仅工作日/工作时间触发
- [ ] **规则执行历史** — 触发记录与结果
- [ ] **规则模板库** — 预置常见模板

### 集成外部系统

- [ ] **验证码打码自动化闭环** — 2Captcha/Capsolver solver 代码与 handler/route 已部分落地；仍需 production manager wiring/config、CDP 检测、截图/sitekey 抽取、回填、Anti-Captcha/本地 OCR 可选扩展
- [ ] **SMS 验证码自动化闭环** — 5sim/SMSPool provider 代码与 handler/route 已部分落地；仍需 production manager wiring/config、真实验收、finish/prices/config/stats、CDP 填号/填码、sms-activate/HeroSMS 可选扩展
- [ ] **临时邮箱自动化闭环** — inbox/wait-code REST API 已部分落地；session persistence 仍是 best-effort，仍需邮件列表/详情/config/stats 和注册流水线自动填充
- [ ] **浏览器扩展自动注入** — 预装扩展到配置

### AI/ML 基础

- [ ] **智能代理推荐** — 基于规则的代理推荐（先做规则，再迭代 ML）
- [ ] **异常行为检测** — 配置行为偏离告警
- [ ] **验证码难度预测** — 启动前根据指纹+代理+目标站预测
- [ ] **代理成功率预测模型** — GBDT 模型预测代理成功概率
- [ ] **A/B 测试框架** — 双配置盲测对比
- [ ] **配置兼容性数据库** — 匿名聚合成功率数据
- [ ] **Bot 检测仿真测试** — 自动跑分+报告

### 监控

- [ ] **CDP 连接池管理** — 复用连接减少 WebSocket 握手
- [ ] **启动速度优化** — 预缓存内核/预分配端口

### 安全

- [ ] **API 动态令牌** — 短时效 Token 签发
- [ ] **HTTPS 支持** — 远程 TLS 加密
- [ ] **API 精细化限流** — 按端点分组不同阈值
- [ ] **防数据泄漏(DLP)** — 配置隔离
- [ ] **会话录播回放** — 完整回放取证
- [ ] **水印截图** — 嵌入配置 ID+时间戳
- [ ] **剪贴板隔离** — 防 Ctrl+C/V 串台
- [ ] **防篡改审计日志** — 哈希链存储

### 测试

- [ ] **端到端测试框架** — 启动真实浏览器验证 API
- [ ] **代理解析 fuzz 测试** — 畸形配置不崩溃
- [ ] **错误注入测试** — 模拟网络断开/CDP 断开

---

## P3 — 高级编排与团队

### 工作流编排

- [ ] `POST /api/orchestration/workflow` — 创建工作流（DAG 任务编排）
- [ ] `GET /api/orchestration/workflow/{id}` — 工作流执行状态
- [ ] `POST /api/orchestration/cron` — 设置定时任务
- [ ] **工作流条件分支** — if/else 逻辑
- [ ] **规则链式触发** — 规则 A 完成自动触发 B

### 批量操作模板

- [ ] `POST /api/template` — 创建操作模板
- [ ] `POST /api/template/{id}/apply` — 应用模板
- [ ] **批量操作模板引擎** — 模板参数化 + 变量注入

### 实例快照

- [ ] `POST /api/instances/snapshot` — 保存实例快照（标签页+Cookie+存储）
- [ ] `POST /api/instances/restore` — 恢复快照
- [ ] `GET /api/instances/snapshot/list` — 快照列表

### 配置批量操作

- [ ] `POST /api/profiles/batch-create` — 批量创建配置
- [ ] **配置生命周期管理** — 过期策略、N 天不活动自动归档

### 元素调试

- [ ] `POST /api/workbench/debug/selector` — CSS 选择器调试
- [ ] `POST /api/workbench/debug/list-elements` — 页面可交互元素列表
- [ ] `POST /api/workbench/debug/dom-tree` — DOM 树快照

### 操作审计

- [ ] `GET /api/audit/operations` — 操作日志列表
- [ ] `GET /api/audit/errors` — 错误日志聚合

### 反检测高级

- [ ] **行为模式随机化层级** — 鼠标/滚动/打字变异参数可配置
- [ ] **情绪感知行为** — 根据页面内容(成功页/错误页)调整操作速度

### 团队协作

- [ ] **多用户 RBAC** — 管理员/操作员/观察者
- [ ] **配置共享与锁定** — 防并发冲突
- [ ] **工作区隔离** — 客户项目完全隔离
- [ ] **操作活动日志** — 审计记录
- [ ] **远程协作 + CDP 共享** — 一次性令牌分享

### 集成

- [ ] **WebSocket 实时事件流** — 外部系统订阅
- [ ] **Telegram/Discord/Slack/Email 通知集成**
- [ ] **n8n/Node-RED 节点** — 外部工作流引擎触发
- [ ] **Puppeteer/Playwright 兼容层** — 现有脚本不改代码跑
- [ ] **Python/JS/Go SDK** — OpenAPI → 客户端代码自动生成

### 部署

- [ ] **ARM64 支持** — 树莓派/Apple Silicon
- [ ] **Kubernetes Helm Chart** — headless 集群编排
- [ ] **Prometheus 指标 + Grafana 面板** — 生产监控
- [ ] **自动更新 + 增量补丁** — bsdiff + 回滚

---

## P4+ — 平台化与企业级

### 团队与企业

- [ ] **LDAP/SSO 登录** — 企业统一认证
- [ ] **转售商面板** — 子账户 + 用量计费 + 账单
- [ ] **审批工作流** — 创建/删除配置需审批

### AI/ML 深度集成

- [ ] **自然语言→操作序列** — LLM 将一句话拆解为动作
- [ ] **用户行为学习** — 录制真人→合成行为模型
- [ ] **AI 反检测建议引擎** — 分析封禁原因并给具体建议
- [ ] **社区指纹配置基准** — 匿名聚合各平台成功率最高配置
- [ ] **代理成功率预测模型(ML)** — GBDT 模型

### 平台生态

- [ ] **插件系统(Lua/JS)** — 用户扩展 API
- [ ] **配置模板市场** — 分享/下载预配置模板
- [ ] **会话市场** — 分享预登录会话
- [ ] **住宅代理市场 API 集成** — BrightData/Oxylabs/IPRoyal 自动化
- [ ] **分布式代理健康检测网络** — 多节点协同
- [ ] **边缘部署集中管理** — 多地节点统一调度

### 模块重构（代码质量）

- [ ] **browser_runtime_state.go → internal/browser/** — 归位
- [ ] **browser_start_settings.go → internal/browser/** — 归位
- [ ] **browser_process_monitor.go → internal/browser/** — 归位
- [ ] **browser_launch_args.go → internal/browser/** — 归位
- [ ] **window_control_*.go → internal/winctrl/** — 新建包归位
- [ ] **sysproc_*.go → internal/proxy/** — 归位
- [ ] **residual_processes_*.go → internal/browser/** — 归位
- [ ] **license_state.go + app_license.go → internal/license/** — 独立领域
- [ ] **runtime_bridge.go / runtime_wails.go → internal/runtime/** — 新建包归位
- [ ] **bootstrap.go** — 保留根目录（启动引导）
- [ ] **添加 Swagger/OpenAPI 3.0 规范文档**
- [ ] **统一返回格式** — 全部端点 `{"ok":bool,"data":?,"error":?,"meta":?}`
- [ ] **统一错误码** — 参数错误(4xx)、逻辑冲突(409)、内部错误(5xx)
- [ ] **添加分页规范** — `?offset=&limit=` 统一处理
- [ ] **API 版本前缀** — `/api/v1/...`

---

## 优先级速览

| 优先级 | 类别 | 项数 | 预估工时 |
|--------|------|------|----------|
| **P0** | 紧急修复 + API 缺失 | 38 | ~2-3 周 |
| **P1** | 核心增强 + 质量改进 | 30 | ~7 周 |
| **P2** | 差异化能力 | 80+ | ~4-5 月 |
| **P3** | 高级编排 + 团队 | 30 | ~2-3 月 |
| **P4+** | 平台化 + 企业 + AI | 15 | ~5-6 月 |
| **总计** | | **~190+** | **~12-18 月** |
