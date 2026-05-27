# Personal Pilot — 模块拆分与完整待办清单

> 基于当前后端架构(backend/ 下 61 个根 Go 文件 + 18 个 internal 包)的全量模块划分与功能缺失记录。
> 日期: 2026-05-17

---

## 一、当前模块结构

### cmd 层

| 入口 | 文件 | 职责 |
|------|------|------|
| `cmd/personal-pilot-core/main.go` | 桌面/Wails 主入口 | 初始化配置、数据库、日志、调度器、事件总线、启动 HTTP API 服务 |
| `cmd/gen-events/main.go` | 事件代码生成器 | 从 schema.yaml 生成 TypeScript 类型 + Go 常量/发射器/文档 |
| `cmd/profile-recover/main.go` | 配置恢复工具 | 从快照/备份恢复浏览器配置 |
| `cmd/xhs-nurture-demo/main.go` | 小红书养号演示 | 演示行为引擎用法 |

### App 层（backend/*.go，61 文件）

```
app.go                          — 核心 App 结构体 + 生命周期 + Wails 绑定注册
├── app_automation.go           — 自动化规则引擎绑定
├── app_backup.go / app_backup_ops.go — 备份/恢复
├── app_behavior.go             — 录制回放 + 行为引擎绑定
├── app_bookmark.go             — 书签管理
├── app_browser_action.go / _types.go — 浏览器动作执行
├── app_cookie.go               — Cookie 管理
├── app_deepseek_register.go / _win32.go — DeepSeek 注册自动化
├── app_eventlog.go             — 事件日志
├── app_group.go                — 分组管理
├── app_instance.go             — 实例启停管理
├── app_launchcode.go           — 启动码管理
├── app_license.go              — 许可证/授权
├── app_paths.go                — 路径管理
├── app_profile.go              — 配置管理
├── app_proxy_binding.go        — 代理绑定
├── app_proxy_import.go         — 代理导入
├── app_proxy_subscription.go   — 代理订阅管理
├── app_scheduler.go            — 定时任务
├── app_shutdown.go             — 优雅关闭
├── app_snapshot.go             — 快照管理
├── app_synchronizer.go         — 同步器
├── app_utils.go                — 工具函数
├── app_workbench_detection.go  — 工作台检测
├── bootstrap.go                — 启动引导
├── browser_launch_args.go      — 浏览器启动参数
├── browser_process_monitor.go  — 浏览器进程监控
├── browser_runtime_state.go    — 运行时状态
├── browser_start_settings.go   — 启动设置
├── license_state.go            — 许可证状态
├── residual_processes_*.go     — 残留进程清理
├── runtime_bridge.go / _wails.go / _paths.go — 运行时桥接
├── window_control_*.go         — 窗口控制
├── sysproc_*.go                — 系统进程
└── *_test.go                   — 测试文件(~15 个)
```

### internal 层（18 个包，~150+ 文件）

| 包名 | 文件数 | 职责 | 当前成熟度 |
|------|--------|------|-----------|
| **launchcode** | 15 | REST API 服务(路由/handler/中间件/认证/限流/选择器/DAO) | 核心已完善 |
| **browser** | 42 | 浏览器管理(配置/内核/指纹/身份/代理绑定/启动/进程) | 核心已完善 |
| **proxy** | 22 | 代理管理(解析/桥接/测速/健康/IP检测/信任评分/选择器) | 核心已完善 |
| **behavior** | 21 | 行为引擎(录制/回放/CDP执行器/人性化中间件/预设) | 核心已完善 |
| **events** | 8 | 事件系统(注册/发射器/桥接/日志/身份目录) | 完善 |
| **logger** | 12 | 日志系统(分级/格式化/拦截器/分片/内存写入) | 完善 |
| **config** | 2 | 配置加载/持久化 | 完善 |
| **automation** | 3 | 规则引擎(规则定义/条件评估/动作执行) | 已有未接入API |
| **scheduler** | 6 | 定时任务(调度器/CDP Runner/SQLite存储) | 已有未接入API |
| **email** | 5 | 临时邮箱(mail.tm + Cloudflare Worker) | 已有未接入API |
| **backup** | 4 | 备份加密/规格/预检 | 已有未接入API |
| **webhook** | 1 | Webhook 发送 | 已有未接入API |
| **wininput** | 3 | Windows 原生输入模拟(鼠标/键盘/坐标) | 已有未集成 |
| **database** | 1 | SQLite 数据库连接 | 基础 |
| **apppath** | 2 | 应用路径解析 | 基础 |
| **fsutil** | 2 | 文件系统工具 | 基础 |
| **tray** | 2 | 系统托盘 | 基础 |

### 数据流架构

```
[Wails GUI] ←→ [App 层(app_*.go)] → [internal 包] → [SQLite / 文件系统 / CDP / 外部API]
                                        ↕
                              [Launch HTTP API Server]
                              (localhost:19876)
                                 ↕
                           [外部工具/Claude Code]
```

---

## 二、API 端点文档（当前已实现）

### 认证与安全

| 端点 | 方法 | 功能 | 实现文件 |
|------|------|------|----------|
| `/api/health` | GET | 健康检查 | server.go |
| API 认证 | - | 可选 `X-Personal-Pilot-Api-Key` header | auth.go |
| IP 限制 | - | 默认仅 127.0.0.1 | server.go |
| 速率限制 | - | 可配置 RateLimiter | ratelimit.go |

### 实例管理

| 端点 | 方法 | 功能 | 实现文件 |
|------|------|------|----------|
| `/api/instances/stop` | POST | 停止实例(JSON body) | instance_api.go |
| `/api/instances/status?profileId=` | GET | 实例状态 | instance_api.go |
| `/api/instances/copy` | POST | 复制实例 | instance_api.go |
| `/api/instances/restart` | POST | 重启实例 | instance_api.go |
| `/api/instances/batch/start` | POST | 批量启动 | instance_api.go |
| `/api/instances/batch/stop` | POST | 批量停止 | instance_api.go |
| `/api/instances/{profileId}` | GET | 实例状态(路径ID) | instance_api.go |
| `/api/instances/{profileId}?action=start/stop/restart` | POST | 实例操作 | instance_api.go |

### 配置管理 (Profile)

| 端点 | 方法 | 功能 | 实现文件 |
|------|------|------|----------|
| `/api/profiles` | GET | 全部配置列表 | profile_api.go |
| `/api/profiles` | POST | 创建配置(可选自动启动) | profile_api.go |
| `/api/profiles/{id}` | GET | 配置详情 | profile_api.go |
| `/api/profiles/{id}` | PUT | 更新配置 | profile_api.go |
| `/api/profiles/{id}` | DELETE | 删除配置 | profile_api.go |

### 启动

| 端点 | 方法 | 功能 | 实现文件 |
|------|------|------|----------|
| `/api/launch` | POST | 按选择器启动(支持批量模式 all) | server.go |
| `/api/launch/{code}` | GET | 按启动码启动 | server.go |
| `/api/launch/logs?limit=N` | GET | 启动调用日志 | server.go |

### 浏览器工作台

| 端点 | 方法 | 功能 | 实现文件 |
|------|------|------|----------|
| `/api/workbench/navigate` | POST | 导航到URL | workbench_api.go |
| `/api/workbench/refresh` | POST | 刷新页面 | workbench_api.go |
| `/api/workbench/screenshot` | POST | 页面截图(base64) | workbench_api.go |
| `/api/workbench/fingerprint` | POST | 指纹快照 | workbench_api.go |
| `/api/workbench/fingerprint-health` | POST | 指纹健康评分 | workbench_api.go |
| `/api/workbench/identity/report` | POST | 身份强度报告 | workbench_api.go |
| `/api/workbench/identity/consistency` | POST | 身份一致性 | workbench_api.go |
| `/api/workbench/cookies/get` | POST | 获取Cookie | workbench_api.go |
| `/api/workbench/cookies/set` | POST | 设置Cookie(批量) | workbench_api.go |
| `/api/workbench/cookies/clear` | POST | 清除Cookie | workbench_api.go |
| `/api/workbench/tabs/list` | POST | 标签页列表 | workbench_api.go |
| `/api/workbench/tabs/switch` | POST | 切换标签页 | workbench_api.go |
| `/api/workbench/tabs/close` | POST | 关闭标签页 | workbench_api.go |
| `/api/workbench/tabs/new` | POST | 新建标签页 | workbench_api.go |
| `/api/workbench/storage/local-storage` | POST | 获取 LocalStorage | workbench_api.go |
| `/api/workbench/storage/set-local-storage` | POST | 设置 LocalStorage | workbench_api.go |
| `/api/workbench/behavior/start` | POST | 启动行为脚本 | workbench_api.go |
| `/api/workbench/behavior/stop` | POST | 停止行为脚本 | workbench_api.go |
| `/api/workbench/behavior/config` | POST | 配置行为强度 | workbench_api.go |
| `/api/workbench/nurture/start` | POST | 启动养号 | workbench_api.go |
| `/api/workbench/nurture/stop` | POST | 停止养号 | workbench_api.go |
| `/api/workbench/proxy/check` | POST | 检测代理 | workbench_api.go |
| `/api/workbench/proxy/speedtest` | POST | 代理测速 | workbench_api.go |
| `/api/workbench/activate` | POST | 激活窗口 | workbench_api.go |
| `/api/workbench/arrange` | POST | 排列窗口 | workbench_api.go |
| `/api/workbench/click` | POST | 点击元素 | workbench_api.go |
| `/api/workbench/type` | POST | 输入文本 | workbench_api.go |
| `/api/workbench/scroll` | POST | 滚动页面 | workbench_api.go |
| `/api/workbench/hover` | POST | 悬停 | workbench_api.go |
| `/api/workbench/double-click` | POST | 双击 | workbench_api.go |
| `/api/workbench/right-click` | POST | 右键 | workbench_api.go |
| `/api/workbench/wait` | POST | 等待 | workbench_api.go |
| `/api/workbench/actions` | POST | 批量动作序列(支持 stream) | workbench_api.go |

### 录制与回放

| 端点 | 方法 | 功能 | 实现文件 |
|------|------|------|----------|
| `/api/recording/start` | POST | 开始录制 | recording_api.go |
| `/api/recording/stop` | POST | 停止录制 | recording_api.go |
| `/api/recording/list` | GET | 录制列表 | recording_api.go |
| `/api/recording/status` | GET | 录制状态 | recording_api.go |
| `/api/recording/{id}` | GET | 录制详情(支持分页) | recording_api.go |
| `/api/recording/{id}` | DELETE | 删除录制 | recording_api.go |
| `/api/recording/play` | POST | 回放录制(含变体参数) | recording_api.go |
| `/api/recording/play/stop` | POST | 停止回放 | recording_api.go |
| `/api/recording/quick` | POST | 快速录制 | recording_api.go |
| `/api/recording/sessions/cleanup` | POST | 清理过期会话 | recording_api.go |

### 行为预设

| 端点 | 方法 | 功能 | 实现文件 |
|------|------|------|----------|
| `/api/behavior/presets` | GET | 预设列表 | recording_api.go |
| `/api/behavior/presets/{id}` | GET | 预设详情 | recording_api.go |

### CDP 代理

| 端点 | 方法 | 功能 | 实现文件 |
|------|------|------|----------|
| `/*` (非 /api 路径) | 所有 | 反向代理到当前活动实例的 CDP 端口 | server.go |

---

## 三、API 缺失补全清单（P0→P3）

### P0 — 核心缺失（外部接入必备）

| 端点 | 方法 | 功能 | 依赖 |
|------|------|------|------|
| `POST /api/proxy/subscribe` | POST | 添加代理订阅源(URL + 自动刷新) | proxy_dao |
| `DELETE /api/proxy/subscribe/{id}` | DELETE | 删除订阅源 | proxy_dao |
| `POST /api/proxy/subscribe/{id}/refresh` | POST | 手动刷新订阅 | proxy_dao, IsSingBoxProtocol |
| `GET /api/proxy/subscribe/list` | GET | 订阅源列表 | proxy_dao |
| `GET /api/proxy/list` | GET | 全部代理节点列表 | proxy_dao |
| `PUT /api/proxy/{id}` | PUT | 编辑代理节点 | proxy_dao |
| `POST /api/proxy/{id}/speedtest` | POST | 手动触发单节点测速 | proxy.SpeedTest |
| `POST /api/proxy/{id}/health` | POST | 手动触发 IP 健康检测 | proxy.FetchProxyIPInfo |
| `POST /api/proxy/batch/speedtest` | POST | 批量测速 | proxy.SpeedTest |
| `POST /api/proxy/batch/health` | POST | 批量 IP 健康检测 | proxy.FetchProxyIPInfo |
| `POST /api/proxy/manual` | POST | 添加手动代理(domain/port/username/password) | proxy.parseStandardProxy |
| `DELETE /api/proxy/manual/{id}` | DELETE | 删除手动代理 | proxy_dao |
| `PUT /api/proxy/manual/{id}` | PUT | 更新手动代理 | proxy_dao |
| `POST /api/proxy/quick-add` | POST | 智能识别并导入代理配置(URL/Clash/Base64) | parser.go |
| `POST /api/proxy/parse` | POST | 解析任意格式代理配置并返回结构化信息 | parser.go |
| `POST /api/proxy/subscribe/{id}/validate` | POST | 验证订阅源可用性(预览不导入) | proxy |
| `POST /api/proxy/subscribe/{id}/import-clash` | POST | 从 Clash 配置导入 | clash.go |
| `GET /api/proxy/subscribe/{id}/nodes` | GET | 订阅源下的所有节点 | proxy_dao |
| `POST /api/profiles/{id}/proxy` | PUT | 切换配置的代理 | browser.proxy_binding |
| `POST /api/profiles/{id}/clone` | POST | 克隆配置 | browser.profile |
| `GET /api/groups` | GET | 分组列表 | group_dao |
| `POST /api/groups` | POST | 创建分组 | group_dao |
| `PUT /api/groups/{id}` | PUT | 编辑分组 | group_dao |
| `DELETE /api/groups/{id}` | DELETE | 删除分组 | group_dao |
| `POST /api/groups/{id}/assign` | POST | 批量分配配置到分组 | group_dao |
| `GET /api/cores` | GET | 已安装浏览器内核列表 | core_dao |
| `GET /api/settings` | GET | 运行时设置 | config |
| `PUT /api/settings` | PUT | 更新设置 | config |

### P1 — 核心增强

| 端点 | 方法 | 功能 | 依赖 |
|------|------|------|------|
| `GET /api/events/stream` | GET | SSE 事件流(实时推送) | events |
| `POST /api/webhook` | POST | 配置 Webhook URL | webhook |
| `GET /api/webhook` | GET | 查看 Webhook 配置 | webhook |
| `DELETE /api/webhook` | DELETE | 删除 Webhook | webhook |
| `POST /api/recording/{id}/export` | POST | 导出录制为 JSON | recording_store |
| `POST /api/recording/import` | POST | 导入录制 | recording_store |
| `GET /api/system/info` | GET | 系统信息(版本/内存/磁盘) | - |
| `GET /api/system/stats` | GET | 运行时统计 | App |
| `POST /api/system/backup` | POST | 触发备份 | backup |
| `POST /api/system/config/reload` | POST | 热重载配置 | config |
| `POST /api/proxy/manual/batch` | POST | 批量添加手动代理 | proxy |
| `POST /api/proxy/manual/{id}/test` | POST | 测试手动代理连通性 | proxy.SpeedTest |

### P2 — 差异化能力

| 端点 | 方法 | 功能 | 依赖 |
|------|------|------|------|
| `POST /api/scraper/task` | POST | 定义采集任务(URL+规则+格式) | behavior.CDPExecutor |
| `GET /api/scraper/task/{id}/data` | GET | 获取采集数据 | - |
| `POST /api/scraper/export` | POST | 导出采集数据为 CSV/JSON | - |
| `POST /api/account/store` | POST | 加密存储账号密码 | credstore |
| `GET /api/account/list` | GET | 已存储账号列表 | credstore |
| `POST /api/account/{id}/autofill` | POST | 自动填充登录表单 | behavior.CDPExecutor |
| `POST /api/recording/{id}/merge` | POST | 合并录制 | behavior |
| `PUT /api/recording/{id}` | PUT | 重命名/编辑录制 | recording_store |
| `POST /api/recording/{id}/trim` | POST | 裁剪录制事件 | behavior |
| `GET /api/recording/stats` | GET | 录制库统计 | recording_store |
| `POST /api/recording/{id}/analyze` | POST | 录制操作模式分析 | behavior |
| `POST /api/recording/{id}/diff` | POST | 两次录制差异对比 | behavior |
| `POST /api/cores/download` | POST | 下载安装新内核 | download_core |
| `DELETE /api/cores/{id}` | DELETE | 删除内核 | core_dao |
| `GET /api/proxy/quality/summary` | GET | 代理池质量概览 | trust_score |
| `GET /api/proxy/quality/ranking` | GET | 代理综合评分排名 | trust_score |
| `GET /api/proxy/quality/history` | GET | 代理历史趋势 | trust_score |
| `GET /api/proxy/quality/geo-distribution` | GET | 代理地理位置分布 | iphealth |
| `POST /api/proxy/quality/auto-select` | POST | 智能推荐最佳代理 | selector |
| `GET /api/proxy/binding/list` | GET | 代理绑定关系 | proxy_binding |
| `POST /api/proxy/binding` | POST | 创建代理绑定 | proxy_binding |
| `DELETE /api/proxy/binding/{id}` | DELETE | 删除绑定 | proxy_binding |
| `POST /api/proxy/traffic/{proxyId}` | POST | 代理流量统计 | - |
| `POST /api/workbench/cookies/import` | POST | 批量导入Cookie | browser |
| `POST /api/workbench/cookies/export` | POST | 导出Cookie | browser |
| `POST /api/workbench/cookies/transfer` | POST | Cookie跨实例转移 | browser |
| `POST /api/workbench/cookies/verify` | POST | 验证登录态有效性 | cookie_verify |
| `GET /api/fingerprint/templates` | GET | 指纹模板列表 | fingerprint_policy |
| `POST /api/fingerprint/templates` | POST | 创建指纹模板 | fingerprint_policy |
| `POST /api/fingerprint/templates/{id}/apply` | POST | 应用指纹模板 | fingerprint_policy |
| `POST /api/fingerprint/randomize` | POST | 随机生成推荐指纹 | browser |
| `POST /api/fingerprint/compare` | POST | 对比两个实例指纹 | browser |

### P3 — 高级编排

| 端点 | 方法 | 功能 | 依赖 |
|------|------|------|------|
| `POST /api/orchestration/workflow` | POST | 创建工作流 | scheduler |
| `GET /api/orchestration/workflow/{id}` | GET | 工作流状态 | scheduler |
| `POST /api/orchestration/cron` | POST | 设置定时任务 | scheduler |
| `GET /api/rules` | GET | 自动化规则列表 | automation |
| `POST /api/rules` | POST | 创建规则 | automation |
| `PUT /api/rules/{id}` | PUT | 编辑规则 | automation |
| `DELETE /api/rules/{id}` | DELETE | 删除规则 | automation |
| `POST /api/rules/{id}/test` | POST | 模拟规则触发 | automation |
| `POST /api/instances/snapshot` | POST | 保存实例快照 | snapshot |
| `POST /api/instances/restore` | POST | 恢复快照 | snapshot |
| `POST /api/instances/broadcast` | POST | 广播操作 | workbench |
| `POST /api/instances/sync/actions` | POST | 同步动作 | workbench |
| `POST /api/template` | POST | 创建操作模板 | - |
| `POST /api/template/{id}/apply` | POST | 应用操作模板 | - |
| `POST /api/workbench/debug/selector` | POST | CSS选择器调试 | behavior.CDPExecutor |
| `POST /api/workbench/debug/list-elements` | POST | 页面可交互元素列表 | behavior.CDPExecutor |
| `GET /api/audit/operations` | GET | 操作审计日志 | - |
| `GET /api/audit/errors` | GET | 错误日志聚合 | - |
| `POST /api/integration/telegram` | POST | Telegram通知配置 | webhook |
| `POST /api/integration/email` | POST | 邮件通知配置 | webhook |
| `POST /api/profiles/batch-create` | POST | 多实例配置创建 | browser |
| `POST /api/ai/instruction` | POST | 自然语言→操作序列 | LLM + behavior |

---

## 四、功能层面待完成清单

### 4.1 紧急修复（P0 — 当前 Bug）

| 项目 | 位置 | 问题 | 影响 |
|------|------|------|------|
| **CDPExecutor 空实现** | `cdp_executor.go:ExecuteMutatedAction` | `MutatedGetHtml`(type 6)、`MutatedGetText`(type 7)、`MutatedScreenshot`(type 5) 在 switch 中落入 `default: return nil` | 通过 actions 数组传 `{"type":"screenshot"}` 静默无操作 |
| **`handlerRecordingStart` LimitReader** | `recording_api.go:84` | ~~已修复~~ | - |

### 4.2 已有基础设施—接入（P1）

| 能力 | 包 | 当前状态 | 任务 |
|------|-----|----------|------|
| **任务调度器 API 化** | `scheduler` | 完整调度引擎(CDP Runner + Cron/Interval/Event 触发 + 依赖链)但无 REST 接口 | 暴露任务 CRUD + 手动触发 + 状态查询 API |
| **自动化规则引擎 API 化** | `automation` | 完整事件匹配/条件评估/动作执行但仅内部使用 | 暴露规则 CRUD + 模拟触发 API |
| **临时邮箱接入自动化** | `email` | mail.tm + Cloudflare Worker 双通道已实现 | 接入注册流程：自动创建邮箱 → 等待验证码 → 自动填充 |
| **代理信任评分持久化** | `proxy/trust_score.go` | 加权评分模型已实现但未持久化 | 存数据库 + 质量趋势查询 + 智能排序 |
| **Webhook 接入事件总线** | `webhook` | 发送器已有但未接入 | 绑定内部事件 → 自动转发到外部 URL |
| **Windows 输入模拟集成** | `wininput` | 时钟级鼠标/键盘/坐标操作 | 无 CDP 时的原生窗口交互(弹框处理) |
| **备份加密接入** | `backup` | 备份规格+加密已有 | 通过 API 触发/恢复 + 自动定时备份 |
| **Cookie 有效性验证接入** | `browser/cookie_verify.go` | 验证逻辑已有 | 通过 API 暴露 + 自动续期流程 |

### 4.3 代理系统深度改进

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **代理自动故障转移** | P1 | 当前代理不可用时自动切换到同组备用节点 |
| **代理订阅自动刷新失败告警** | P1 | 订阅源过期/不可用通知 |
| **SOCKS5 认证传递完善** | P1 | 部分代理链路认证不完整 |
| **测速结果本地缓存** | P1 | 短时间重复测速直接返回缓存 |
| **Xray/sing-box 优雅退出** | P1 | SIGKILL→优先 SIGTERM |
| **手动代理与订阅代理统一管理** | P1 | 两种来源统一对待 |
| **住宅代理市场 API 集成** | P2 | BrightData/Oxylabs/IPRoyal 对接 |
| **代理链路多跳(Proxy Chain)** | P2 | 入口→中转→出口 |
| **粘性会话管理** | P2 | 固定出口 IP 至少 N 分钟 |
| **代理预热** | P2 | 预测代理提前建连 |
| **目标站点专项测速** | P2 | 针对用户指定 URL 测速 |
| **代理成本追踪** | P2 | 流量/时间/ROI 分析 |
| **SOCKS4/SOCKS4a 协议** | P2 | 协议兼容性补充 |
| **代理 IP 变更自动发现** | P2 | 域名解析变化自动更新 |
| **端口扫描/健康雷达** | P2 | 主动扫描池中 IP:PORT |

### 4.4 反检测深度改进

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **指纹基线自动校准** | P1 | 启动后比预期 vs 实际指纹，差异大告警 |
| **WebRTC 泄漏检测** | P1 | 检测真实 IP 是否泄漏 |
| **时区/IP 地理位置一致性校验** | P1 | 代理国家 vs 浏览器时区不符时自动修正 |
| **Canvas 噪声策略可配置** | P2 | 不同策略对不同反检测库效果不同 |
| **WebGPU 指纹模拟** | P2 | 当前只采集不伪造；Cloudflare Turnstile 已用 |
| **AudioContext 指纹伪造** | P2 | 防 audio fingerprint 跨配置关联 |
| **Navigator.permissions API 伪造** | P2 | 防止检测为自动化环境 |
| **Battery/NetworkInformation API 伪造** | P2 | 小众检测面补充 |
| **浏览器核心版本回退检查** | P2 | 新版本在某平台被识别时自动建议回退 |
| **Firefox 内核支持** | P2 | 不同指纹面，部分站点限定 |
| **移动端模拟增强** | P2 | 触控事件/viewport/GPU 深度模拟 |
| **Bot 检测仿真测试** | P2 | 自动测试 bot.sannysoft.com 等并生成报告 |
| **指纹渐变式变异** | P2 | 长期使用时指纹参数缓慢变化 |
| **TLS 指纹随机化(JA3/JA3S)** | P2 | 通过 xray/sing-box 随机化 |
| **HTTP/2 与 HTTP/3 指纹伪造** | P2 | Akamai/Cloudflare 用此识别 |
| **字体白名单/黑名单** | P2 | 防字体指纹 |

### 4.5 录制回放改进

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **录制事件去重与合并** | P2 | 连续相同操作合并 |
| **回放暂停/继续** | P2 | 用户可手动介入 |
| **回放步骤预览** | P2 | 回放前展示操作序列 |
| **录制自动标注关键节点** | P2 | 标记验证码/登录/支付等 |
| **回放失败智能重试** | P2 | 元素未找到时等待重试 |
| **回放异常检测** | P2 | 监测到验证码/弹窗时暂停告警 |
| **录制差异对比** | P2 | 两次录制操作序列对比 |
| **多会话录制合并为参数化模板** | P2 | `{{username}}` 变量注入 |

### 4.6 数据与持久化

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **SQLite 数据库迁移框架** | P1 | 当前无 schema 版本管理 |
| **敏感字段加密存储** | P1 | Cookie/代理密码加密存盘 |
| **旧数据自动清理** | P2 | 录制/日志/测速历史阈值清理 |
| **导出格式统一** | P2 | 配置/录制/日志统一格式 |
| **录制文件增量存储** | P2 | 当前全量存储，大文件优化 |

### 4.7 Cookie/会话管理

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **会话健康看板** | P2 | 所有配置登录态存活/有效期 |
| **自动续期** | P2 | Cookie 过期前自动重新获取 |
| **Cookie 转移** | P2 | 配置 A Cookie 复制给配置 B |
| **批量 Cookie 预热** | P2 | 导入后自动访问目标站种 Cookie |
| **Cookie + LocalStorage/IndexedDB 统一快照** | P2 | SPA 站点依赖 IndexedDB |
| **加密会话导出** | P2 | 密码保护的会话包 |

### 4.8 监控与运维

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **端口使用清理** | P1 | 异常退出后残留端口自动回收 |
| **健康检查端点增强** | P1 | 详细组件状态(CDP/代理/DB/存储) |
| **实例资源使用统计** | P2 | 各实例 CPU/内存/网络 |
| **CDP 连接池管理** | P2 | 复用连接减少 WebSocket 握手 |
| **启动速度优化** | P2 | 预缓存内核/预分配端口 |
| **启动参数冲突检测** | P2 | 指纹参数 vs 用户参数冲突告警 |
| **浏览器崩溃自动恢复** | P1 | CDP 断开自动重启并恢复标签页 |
| **Lightpanda 无头内核完善** | P2 | 深度集成比 Chromium 轻 10 倍 |
| **GPU/WebGL 黑名单检测** | P2 | 环境检测 |
| **窗口批量排列预设** | P2 | cascade/tiled/horizontal 布局 |
| **多屏/高 DPI 适配** | P2 | 多显示器场景 |

### 4.9 自动化与规则引擎

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **规则条件支持时间窗口** | P2 | 仅工作日/工作时间触发 |
| **规则执行历史** | P2 | 触发记录与结果 |
| **规则模板库** | P2 | 预置常见模板 |
| **规则链式触发** | P3 | A 完成自动触发 B |

### 4.10 安全加固

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **请求体大小统一限制** | P1 | 所有 POST 端点 io.LimitReader |
| **API 动态令牌** | P2 | 短时效 Token 签发 |
| **HTTPS 支持** | P2 | 远程 TLS 加密 |
| **操作权限分级** | P3 | 只读/操作/管理三级 |
| **API 精细化限流** | P2 | 按端点分组不同阈值 |
| **防数据泄漏(DLP)** | P2 | 配置隔离 |
| **会话录播回放** | P2 | 完整回放取证 |
| **水印截图** | P2 | 嵌入配置 ID+时间戳 |
| **剪贴板隔离** | P2 | 防 Ctrl+C/V 串台 |
| **防篡改审计日志** | P2 | 哈希链存储 |

### 4.11 AI/ML 集成

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **智能代理推荐** | P2 | ML 模型预测最佳代理 |
| **异常行为检测** | P2 | 配置行为偏离告警 |
| **验证码难度预测** | P2 | 启动前预测遇验证码概率 |
| **AI 反检测建议引擎** | P2 | 分析封禁原因给建议 |
| **社区指纹配置基准** | P2 | 匿名聚合成功率数据 |
| **自然语言→操作序列** | P3 | LLM 拆解指令为动作 |
| **代理成功率预测模型** | P2 | GBDT 预测代理成功概率 |
| **用户行为学习** | P3 | 录制真人→合成行为模型 |
| **情绪感知行为** | P3 | 根据页面内容调操作速度 |
| **A/B 测试框架** | P2 | 双配置盲测对比 |
| **配置兼容性数据库** | P2 | 社区共享经验 |
| **Bot 检测仿真测试** | P2 | 自动跑分+报告 |

### 4.12 团队与协作

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **多用户 RBAC** | P3 | 管理员/操作员/观察者 |
| **配置共享与锁定** | P3 | 防并发冲突 |
| **工作区隔离** | P3 | 客户项目完全隔离 |
| **操作活动日志** | P2 | 审计记录 |
| **LDAP/SSO 登录** | P3 | 企业统一认证 |
| **转售商面板** | P4 | 子账户+计费 |
| **WebSocket 实时事件流** | P2 | 外部系统监控 |

### 4.13 集成与外部系统

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **验证码打码集成** | P2 | 2Captcha/Capsolver/Anti-Captcha |
| **SMS 验证码集成** | P2 | Twilio/5sim/sms-activate |
| **临时邮箱集成自动化** | P2 | email 包接入注册流程 |
| **n8n/Node-RED 节点** | P3 | 外部工作流引擎触发 |
| **Python/JS/Go SDK** | P3 | OpenAPI → 客户端代码 |
| **Puppeteer/Playwright 兼容层** | P3 | 现成脚本直接跑 |
| **浏览器扩展自动注入** | P2 | 预装扩展到配置 |
| **插件系统(Lua/JS)** | P3 | 用户扩展 API |

### 4.14 测试与质量

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **并发安全性测试(-race)** | P1 | 多 goroutine 竞争检测 |
| **CDPExecutor 补全后的单元测试** | P1 | Screenshot/GetHtml/GetText |
| **端到端测试框架** | P2 | 真实浏览器验证 API |
| **代理解析 fuzz 测试** | P2 | 畸形配置不崩溃 |
| **错误注入测试** | P2 | 模拟网络断开/CDP 断开 |

### 4.15 部署与运维

| 项目 | 优先级 | 说明 |
|------|--------|------|
| **ARM64 支持** | P3 | 树莓派/Apple Silicon |
| **Kubernetes Helm** | P3 | headless 集群 |
| **Prometheus/Grafana** | P3 | 生产监控 |
| **分布式代理健康检测网络** | P3 | 多节点协同检测 |
| **边缘部署集中管理** | P4 | 多地节点统一调度 |
| **自动更新+增量补丁** | P3 | bsdiff+回滚 |

---

## 五、内部模块拆分建议

当前 `backend/` 根目录有 61 个 .go 文件，建议拆分：

| 当前结构 | 建议拆分 | 说明 |
|----------|----------|------|
| `app.go` + `app_*.go` (30+文件) | 保持 app_* 组织，按功能分组 | 当前已合理 |
| `browser_runtime_state.go` | → `internal/browser/` | 应移到 browser 包 |
| `browser_start_settings.go` | → `internal/browser/` | 应移到 browser 包 |
| `browser_process_monitor.go` | → `internal/browser/` | 应移到 browser 包 |
| `browser_launch_args.go` | → `internal/browser/` | 应移到 browser 包 |
| `window_control_*.go` | → `internal/wininput/` 或新包 `internal/winctrl` | 与 wininput 同类 |
| `sysproc_*.go` | → `internal/proxy/` | 仅用于隐藏窗口 |
| `residual_processes_*.go` | → `internal/browser/` | 浏览器相关 |
| `license_state.go` / `app_license.go` | → `internal/license/` | 独立领域 |
| `runtime_bridge.go` / `runtime_wails.go` | → `internal/runtime/` | 运行时桥接 |
| `bootstrap.go` | 保留在根 | 启动引导 |
| `runtime_paths.go` | → `internal/apppath/` | 路径相关 |

---

## 六、总览统计

| 类别 | 计数 |
|------|------|
| 当前 API 端点 | ~45 |
| API 缺失待补(P0~P3) | ~65 |
| 功能改进(P0~P3) | ~90+ |
| AI/ML 集成 | ~12 |
| 团队协作 | ~6 |
| 集成外部系统 | ~8 |
| 测试改进 | ~5 |
| 部署运维 | ~6 |
| **总计待完成** | **~190+ 项** |

### 各包可复用能力统计

| 包 | 能力 | 当前利用 | 未利用 |
|----|------|----------|--------|
| `scheduler` | 调度引擎+CDP Runner+SQLite存储 | 内部使用 | 无REST API |
| `automation` | 事件匹配+条件评估+动作执行 | 内部使用 | 无REST API |
| `email` | mail.tm+Cloudflare Worker 双通道 | 内部使用 | 未接入自动化 |
| `backup` | 备份加密+规格定义 | 内部使用 | 无REST API |
| `webhook` | HTTP webhook 发送 | 内部使用 | 无REST API 配置 |
| `wininput` | 原生鼠标/键盘/坐标 | 未使用 | 无集成 |
| `trust_score` | 代理加权评分模型 | 内部计算 | 未持久化/查询 |
| `cookie_verify` | Cookie 文件校验 | 内部使用 | 未独立暴露 |
| `fingerprint_health` | 指纹健康评分 | 内部计算 | 无趋势查询 |
