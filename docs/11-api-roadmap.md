# Launch API 改进路线图

> 基于当前 Launch Server REST API 覆盖度的评估与改进计划（2026-05-17）

---

## 一、当前状态

Launch Server 端口 `127.0.0.1:19876`，提供 ~45 个 API 端点，覆盖度约 **85%**。

### 已覆盖

| 领域 | 端点数 | 覆盖度 |
|------|--------|--------|
| 实例生命周期（启/停/重启/批量） | 8 | 100% |
| 页面导航与标签页管理 | 5 | 100% |
| 用户交互（点击/输入/滚动/悬停等） | 11 | 100% |
| Cookie 与存储 | 4 | 100% |
| 指纹与身份信息 | 4 | 100% |
| 录制与回放 | 10 | 90% |
| 行为引擎 | 3 | 70% |
| 窗口管理 | 2 | 100% |
| 代理检测 | 2 | 100% |
| 养号系统 | 2 | 60% |

### 数据获取能力

| 方式 | 状态 | 风控风险 | 调用方式 |
|------|------|----------|----------|
| 截图 | ✅ 已实现 | 低 — 纯视觉采集，浏览器端无感知 | `POST /api/workbench/screenshot` |
| CDP 执行 JS | ✅ 已实现 | 中 — SPA 的监控脚本能检测到 Runtime.evaluate | `actions` 传 `{"type":"script","script":"..."}` |
| 网络请求拦截 | ❌ 未实现 | 低 — 只监听网络，不注入任何东西 | 需新增 `Network.enable` |

**关于数据获取方式的决策指引（给调用方）：**

- **截图** → 适合视觉验证、DOM 结构变化频繁的页面。返回 base64 PNG dataURL，调用方需处理图像。
- **CDP 执行 JS** → 适合提取结构化数据（JSON/文本）。通过 `{"type":"script"}` 动作传任意 JS 表达式，返回值在 `ActionResult.Value` 中。
- **网络请求拦截**（待实现）→ 适合获取 SPA 页面背后的 API 数据。拦截 XHR/Fetch 响应，拿到原始 JSON，完全绕过页面渲染和 DOM 解析。
- **推荐优先级**：能拦截请求就不执行 JS，能执行 JS 就不截图。截图是最后手段。

---

## 二、85% → 99% 补全项

### P0 — 核心缺失（接入外部工具必备）

| 端点 | 功能 | 优先级 |
|------|------|--------|
| `POST /api/profiles` | 创建浏览器环境配置 | P0 |
| `PUT /api/profiles/{id}` | 更新环境配置 | P0 |
| `DELETE /api/profiles/{id}` | 删除环境配置 | P0 |
| `GET /api/profiles` | 列出全部配置（支持 ?tag=&keyword= 过滤） | P0 |
| `GET /api/profiles/{id}` | 获取单个配置详情 | P0 |
| `PUT /api/profiles/{id}/proxy` | 切换指定配置的代理 | P0 |
| `POST /api/proxy/subscribe` | 添加代理订阅源 | P0 |
| `POST /api/proxy/subscribe/{id}/refresh` | 手动刷新订阅 | P0 |
| `DELETE /api/proxy/subscribe/{id}` | 删除订阅源 | P0 |
| `GET /api/proxy/subscribe/list` | 查看全部订阅源 | P0 |
| `GET /api/proxy/list` | 查看全部代理节点 | P0 |
| `PUT /api/proxy/{id}` | 编辑代理节点 | P0 |
| `POST /api/proxy/{id}/speedtest` | 手动触发单节点测速 | P0 |
| `POST /api/proxy/{id}/health` | 手动触发单节点 IP 健康检测 | P0 |
| `POST /api/proxy/batch/speedtest` | 批量测速 | P0 |
| `GET /api/groups` | 分组列表 | P0 |
| `POST /api/groups` | 创建分组 | P0 |
| `PUT /api/groups/{id}` | 编辑分组 | P0 |
| `DELETE /api/groups/{id}` | 删除分组 | P0 |
| `POST /api/groups/{id}/assign` | 批量分配配置到分组 | P0 |
| `GET /api/cores` | 列出已安装浏览器内核 | P0 |
| `GET /api/settings` | 获取全部运行时设置 | P0 |
| `PUT /api/settings` | 更新运行时设置 | P0 |

### P1 — 核心增强

| 端点 | 功能 | 优先级 |
|------|------|--------|
| `GET /api/events/stream` | SSE 事件流（实例状态/录制完成/代理异常实时推送） | P1 |
| `POST /api/webhook` | 配置 Webhook 回调 URL | P1 |
| `GET /api/webhook` | 查看 Webhook 配置 | P1 |
| `DELETE /api/webhook` | 删除 Webhook 配置 | P1 |
| `POST /api/recording/{id}/export` | 导出录制为 JSON | P1 |
| `POST /api/recording/import` | 导入录制 | P1 |
| `GET /api/system/info` | 系统信息（版本/内存/磁盘） | P1 |
| `GET /api/system/stats` | 运行时统计（运行实例数/代理数） | P1 |
| `POST /api/system/backup` | 触发手动备份 | P1 |
| `POST /api/system/config/reload` | 热重载配置文件 | P1 |

### P2 — 差异化能力

| 端点 | 功能 | 优先级 |
|------|------|--------|
| `POST /api/scraper/task` | 定义采集任务（URL + 提取规则 + 导出格式） | P2 |
| `GET /api/scraper/task/{id}/data` | 获取采集数据 | P2 |
| `POST /api/scraper/export` | 导出采集数据为 CSV/JSON | P2 |
| `POST /api/account/store` | 安全加密存储账号密码 | P2 |
| `GET /api/account/list` | 列出存储的账号 | P2 |
| `POST /api/account/{id}/autofill` | 自动填充登录表单 | P2 |
| `PUT /api/recording/{id}` | 重命名/编辑录制元数据 | P2 |
| `POST /api/recording/{id}/trim` | 裁剪录制事件范围 | P2 |
| `POST /api/cores/download` | 下载/安装新浏览器内核 | P2 |
| `DELETE /api/cores/{id}` | 删除内核 | P2 |

### P3 — 高级编排

| 端点 | 功能 | 优先级 |
|------|------|--------|
| `POST /api/orchestration/workflow` | 自定义工作流：启动→导航→操作→截图→关闭 | P3 |
| `GET /api/orchestration/workflow/{id}` | 工作流执行状态 | P3 |
| `POST /api/orchestration/cron` | 定时任务调度（每日8点批量启动等） | P3 |
| `GET /api/diagnostics/cdp` | CDP 连接诊断 | P3 |
| `GET /api/diagnostics/proxy-chain` | 代理链路端到端延迟分解 | P3 |
| `POST /api/diagnostics/capture-har` | 捕获 HAR（HTTP 归档） | P3 |
| `GET /api/logs` | 查看/搜索/流式 tail 系统日志 | P3 |
| `POST /api/team/invite` | 邀请成员共享代理池/录制库 | P3 |
| `GET /api/team/profiles` | 团队共享配置列表 | P3 |

---

## 三、新增增强功能

### 3.1 网络请求拦截（数据采集增强）

新增 `POST /api/scraper/intercept` 端点，通过 CDP `Network.enable` 监听页面请求，拦截响应并返回原始 JSON 数据。

```
请求: POST /api/scraper/intercept
     {"profileId":"xxx", "urlPattern":"/api/products", "navigateTo":"https://example.com"}
响应: {"ok":true, "requests":[
         {"url":"/api/products", "status":200, "body":{...}},
         {"url":"/api/products?page=2", "status":200, "body":{...}}
       ]}
```

### 3.2 AI 指令端点

新增 `POST /api/ai/instruction`，用自然语言描述操作，自动拆解为动作序列执行。

```bash
curl -X POST http://127.0.0.1:19876/api/ai/instruction \
  -d '{"profileId":"xxx","instruction":"打开小红书首页，搜索Python，把前10条帖子的标题和点赞数提取出来"}'
```

底层调用 LLM 将自然语言 → ActionRequest 数组 → 执行 → 返回结果。

### 3.3 账号箱（凭据管理）

新增 `/api/account/*` 端点，加密存储账号密码，配合自动填充：

```bash
curl -X POST http://127.0.0.1:19876/api/account/store \
  -d '{"site":"taobao.com","username":"shop01","password":"encrypted_here"}'

curl -X POST http://127.0.0.1:19876/api/account/autofill \
  -d '{"profileId":"xxx","site":"taobao.com","selector":"#login-form"}'
# 自动导航到登录页 → 填充账号密码 → 点击登录
```

### 3.4 编排引擎（工作流自动化）

新增 `POST /api/orchestration/workflow` 定义可复用的自动化流程：

```json
{
  "name": "每日数据巡检",
  "steps": [
    {"action": "launch", "keyword": "店铺-A"},
    {"action": "navigate", "url": "https://seller.taobao.com"},
    {"action": "wait", "timeout": 5000},
    {"action": "screenshot", "key": "dashboard_A"},
    {"action": "launch", "keyword": "店铺-B"},
    {"action": "navigate", "url": "https://seller.taobao.com"},
    {"action": "screenshot", "key": "dashboard_B"},
    {"action": "notify", "webhook": "https://hooks.example.com/report"}
  ]
}
```

支持条件跳转、变量传递、错误重试、超时控制。

### 3.5 定时任务

新增 `POST /api/orchestration/cron` 配合工作流：

```bash
curl -X POST http://127.0.0.1:19876/api/orchestration/cron \
  -d '{"workflowId":"wf-001","cron":"0 8 * * *","timezone":"Asia/Shanghai"}'
# 每天早上8点执行工作流
```

### 3.6 批量操作模板

将常用操作流程保存为模板，一次定义多次复用：

| 端点 | 功能 |
|------|------|
| `POST /api/template` | 创建操作模板（录制或手动编排的步骤序列） |
| `GET /api/template/list` | 模板列表 |
| `POST /api/template/{id}/apply` | 在指定实例上执行模板 |
| `PUT /api/template/{id}` | 编辑模板 |
| `POST /api/template/{id}/share` | 导出模板为 JSON 分享给他人 |

模板示例——"登录淘宝并截取订单页"：

```json
{
  "name": "淘宝订单截图",
  "params": ["username", "password"],
  "steps": [
    {"type": "navigate", "url": "https://login.taobao.com"},
    {"type": "type", "selector": "#username", "value": "{{username}}"},
    {"type": "type", "selector": "#password", "value": "{{password}}"},
    {"type": "click", "selector": ".submit-btn"},
    {"type": "wait", "waitForSelector": ".orders-table", "timeout": 15000},
    {"type": "screenshot"}
  ]
}
```

### 3.7 代理池质量看板 API

目前代理测速和 IP 健康检测结果只有内部存储，缺少结构化查询接口：

| 端点 | 功能 |
|------|------|
| `GET /api/proxy/quality/summary` | 代理池整体质量概览（总数/可用/高延迟/高风险/待测） |
| `GET /api/proxy/quality/ranking` | 代理综合评分排名（速度+IP纯净度+地理位置加权） |
| `GET /api/proxy/quality/history?proxyId=x&range=7d` | 单个代理的历史测速/健康趋势 |
| `GET /api/proxy/quality/geo-distribution` | 代理 IP 地理位置分布统计（按国家/运营商） |
| `POST /api/proxy/quality/auto-select` | 根据目标网站自动推荐最佳代理（最快+同国家+纯净） |

### 3.8 实例快照与恢复

浏览器运行时的完整状态保存和恢复：

| 端点 | 功能 |
|------|------|
| `POST /api/instances/{id}/snapshot` | 保存实例快照（所有标签页的 URL、Cookie、LocalStorage） |
| `POST /api/instances/{id}/restore` | 从快照恢复实例（启动浏览器 → 打开标签页 → 注入 Cookie） |
| `GET /api/instances/snapshot/list` | 快照列表 |
| `DELETE /api/instances/snapshot/{id}` | 删除快照 |

```bash
# 保存当前状态
curl -X POST http://127.0.0.1:19876/api/instances/snapshot \
  -d '{"profileId":"shop-01"}'
# → {"id":"snap-001","tabs":[{"url":"https://taobao.com/orders"},...],"cookies":[...]}

# 在另一台机器上恢复
curl -X POST http://127.0.0.1:19876/api/instances/restore \
  -d '{"profileId":"shop-01","snapshotId":"snap-001"}'
# → 启动浏览器 → 导航到订单页 → Cookie 注入 → 保持登录态
```

### 3.9 自动化规则引擎 API

项目已有 `internal/automation` 包（规则引擎+条件评估），但未暴露为 REST API：

| 端点 | 功能 |
|------|------|
| `GET /api/rules` | 自动化规则列表 |
| `POST /api/rules` | 创建规则（事件触发→条件匹配→动作执行） |
| `PUT /api/rules/{id}` | 编辑规则 |
| `DELETE /api/rules/{id}` | 删除规则 |
| `POST /api/rules/{id}/test` | 模拟触发规则的执行结果预览 |

```json
// 规则示例：当代理延迟>500ms时自动切换到备用节点
{
  "name": "高延迟自动切换",
  "triggerEvent": "proxy:speed:result",
  "condition": "latencyMs>=500",
  "action": "run_task",
  "actionParams": {"taskId": "switch-backup-proxy"},
  "cooldown": "5m"
}
```

### 3.10 多实例同步操作

目前所有操作都是单实例的，新增广播/同步操作能大幅提高批量管理效率：

| 端点 | 功能 |
|------|------|
| `POST /api/instances/broadcast` | 对所有运行中实例广播同一操作（如全部导航到某个 URL） |
| `POST /api/instances/sync/actions` | 同步动作：确保所有实例在同一时间点执行动作（适合批量抢购/秒杀） |
| `POST /api/instances/sync/navigate` | 所有实例同时导航到指定页面 |
| `POST /api/instances/sync/click` | 所有实例同时点击元素 |
| `GET /api/instances/broadcast/status` | 查看广播操作在各实例上的执行结果 |

```bash
# 秒杀场景：5个实例同时点击"立即购买"
curl -X POST http://127.0.0.1:19876/api/instances/sync/actions \
  -d '{
    "profileIds": ["shop-01","shop-02","shop-03","shop-04","shop-05"],
    "action": {"type": "click", "selector": "#buy-now"},
    "syncTime": "2026-05-17T10:00:00Z",
    "jitterMs": 50
  }'
```

### 3.11 代理绑定与流量路由管理

| 端点 | 功能 |
|------|------|
| `GET /api/proxy/binding/list` | 查看所有配置的代理绑定关系 |
| `POST /api/proxy/binding` | 创建代理绑定（配置A → 代理B） |
| `DELETE /api/proxy/binding/{id}` | 删除绑定 |
| `POST /api/proxy/binding/recommend` | AI 推荐绑定策略（按地理位置/延迟/纯净度自动匹配） |
| `GET /api/proxy/traffic/{proxyId}` | 查看代理流量统计（请求数/字节数/错误率） |

### 3.12 Cookie 管理增强

已有基础的 Cookie 增删改查，补充以下能力：

| 端点 | 功能 |
|------|------|
| `POST /api/workbench/cookies/import` | 从 JSON/Netscape 格式文件批量导入 Cookie |
| `POST /api/workbench/cookies/export` | 导出 Cookie 为 JSON/Netscape 格式 |
| `POST /api/workbench/cookies/domain` | 获取特定域名的 Cookie |
| `POST /api/workbench/cookies/transfer` | 将实例 A 的 Cookie 复制到实例 B（快速换号） |
| `POST /api/workbench/cookies/verify` | 验证当前登录态是否有效（导航到目标页检查跳转） |

```bash
# 账号切换：把实例 A 的 Cookie 复制给实例 B
curl -X POST http://127.0.0.1:19876/api/workbench/cookies/transfer \
  -d '{"fromProfileId":"shop-01","toProfileId":"shop-02","domains":[".taobao.com"]}'
```

### 3.13 指纹模板管理

| 端点 | 功能 |
|------|------|
| `GET /api/fingerprint/templates` | 指纹模板列表（预配置的指纹组合） |
| `POST /api/fingerprint/templates` | 创建指纹模板 |
| `POST /api/fingerprint/templates/{id}/apply` | 将指纹模板应用到指定配置 |
| `POST /api/fingerprint/randomize` | 基于目标平台随机生成推荐指纹参数 |
| `POST /api/fingerprint/compare` | 对比两个实例的指纹差异 |

```bash
# 为目标平台推荐指纹
curl -X POST http://127.0.0.1:19876/api/fingerprint/randomize \
  -d '{"target":"taobao.com","viewport":"1920x1080","locale":"zh-CN"}'
# → 返回推荐的 WebGL 指纹、字体列表、Canvas、AudioContext 等参数
```

### 3.14 协同浏览器（远程协作）

| 端点 | 功能 |
|------|------|
| `POST /api/instances/{id}/share-cdp` | 生成可分享的 CDP 连接 URL（含一次性令牌） |
| `POST /api/instances/{id}/cast` | 将浏览器画面实时投射到 Web 页面（VNC 模式） |
| `POST /api/instances/{id}/collaborate` | 邀请其他人共同操控同一浏览器实例 |

### 3.15 浏览器配置克隆与多实例创建

| 端点 | 功能 |
|------|------|
| `POST /api/profiles/{id}/clone` | 克隆配置（批量生成多个类似配置，自动递增名称） |
| `POST /api/profiles/batch-create` | 多实例配置创建（指定基模板 + 数量 + 命名规则） |
| `POST /api/profiles/{id}/export` | 导出配置为可移植 JSON |
| `POST /api/profiles/import` | 导入配置 |

```bash
# 多实例创建 50 个淘宝店铺配置，自动编号
curl -X POST http://127.0.0.1:19876/api/profiles/batch-create \
  -d '{
    "templateId": "tmpl-taobao-base",
    "count": 50,
    "namePattern": "店铺-{n}",
    "proxyStrategy": "auto-assign"
  }'
```

### 3.16 页面元素定位调试工具

| 端点 | 功能 |
|------|------|
| `POST /api/workbench/debug/selector` | 测试 CSS 选择器能否匹配到元素（返回匹配数+高亮截图） |
| `POST /api/workbench/debug/xpath` | 测试 XPath 表达式 |
| `POST /api/workbench/debug/dom-tree` | 获取页面 DOM 树快照（用于定位） |
| `POST /api/workbench/debug/list-elements` | 列出页面上所有可交互元素（按钮/输入框/链接） |

```bash
# 调试：查看页面有哪些可点击按钮
curl -X POST http://127.0.0.1:19876/api/workbench/debug/list-elements \
  -d '{"profileId":"xxx","filter":"button,a,input[type=submit]"}'
# → 返回元素的标签、文本、CSS 选择器、坐标
```

### 3.17 录制回放统计分析

| 端点 | 功能 |
|------|------|
| `GET /api/recording/stats` | 录制库统计（总数/总事件数/各类型分布） |
| `POST /api/recording/{id}/analyze` | 分析录制中的操作模式（页面跳转路径/最常点击位置/平均停留时间） |
| `POST /api/recording/{id}/diff` | 对比两次录制的差异（哪些操作变了） |
| `GET /api/recording/{id}/heatmap` | 生成录制事件的点击热力图坐标数据 |

### 3.18 配置文件分组批量管理

| 端点 | 功能 |
|------|------|
| `POST /api/groups/{id}/batch-update-proxy` | 分组内所有配置统一更换代理 |
| `POST /api/groups/{id}/batch-update-fingerprint` | 分组内所有配置统一更换指纹 |
| `POST /api/groups/{id}/rotate-proxies` | 分组内配置的代理自动轮换（每天换一批） |
| `GET /api/groups/{id}/health` | 分组健康状况（存活率/成功率/平均延迟） |

### 3.19 外部系统集成

| 端点 | 功能 |
|------|------|
| `POST /api/integration/telegram` | 配置 Telegram Bot 通知（实例崩溃/代理失效/录制完成） |
| `POST /api/integration/discord` | 配置 Discord Webhook 通知 |
| `POST /api/integration/slack` | 配置 Slack Webhook 通知 |
| `POST /api/integration/email` | 配置 SMTP 邮件通知（含告警阈值设置） |

```bash
# 配置 Telegram 通知
curl -X POST http://127.0.0.1:19876/api/integration/telegram \
  -d '{"botToken":"xxx","chatId":"123456","events":["browser:instance:crashed","proxy:bridge:died"]}'
```

### 3.20 手动代理添加（域名/端口/认证）

支持通过 Domain Name、Port、Username、Password 手动添加代理，适用于没有标准订阅链路的自建代理或私有代理：

| 端点 | 功能 |
|------|------|
| `POST /api/proxy/manual` | 添加手动代理 |
| `GET /api/proxy/manual/list` | 手动代理列表 |
| `PUT /api/proxy/manual/{id}` | 更新手动代理 |
| `DELETE /api/proxy/manual/{id}` | 删除手动代理 |
| `POST /api/proxy/manual/batch` | 批量添加手动代理 |
| `POST /api/proxy/manual/{id}/test` | 测试手动代理连通性 |

```json
// 请求体示例 —— socks5 带认证
{
  "name": "我的香港服务器",
  "protocol": "socks5",
  "domain": "hk-proxy.example.com",
  "port": 1080,
  "username": "proxyuser",
  "password": "proxypass",
  "dnsServers": "8.8.8.8",
  "groupName": "自建节点",
  "tags": ["低延迟", "住宅IP"],
  "regionHint": "hk"
}

// 请求体示例 —— http 无认证
{
  "name": "免费代理",
  "protocol": "http",
  "domain": "123.123.123.123",
  "port": 8080
}

// 返回
{
  "ok": true,
  "proxyId": "manual-hk-001",
  "proxyConfig": "socks5://proxyuser:proxypass@hk-proxy.example.com:1080"
}
```

### 3.21 代理快捷导入（智能格式识别）

| 端点 | 功能 |
|------|------|
| `POST /api/proxy/quick-add` | 智能添加：自动识别并解析粘贴的代理配置（URL / Clash / Base64） |
| `POST /api/proxy/parse` | 解析任意格式的代理配置并返回结构化信息（预览/调试用） |

```bash
# 粘贴 Clash 节点配置
curl -X POST http://127.0.0.1:19876/api/proxy/quick-add \
  -d '{"raw": "vmess://eyJhZGQiOiIxMjMuMTIzLjEyMy4xMjMiLCJwb3J0IjoiNDQzIiwiaWQiOiI...", "groupName": "已导入"}'

# 粘贴标准代理 URL
curl -X POST http://127.0.0.1:19876/api/proxy/quick-add \
  -d '{"raw": "socks5://user:pass@1.2.3.4:1080", "name": "我的代理"}'
```

### 3.22 订阅源验证与预览

| 端点 | 功能 |
|------|------|
| `POST /api/proxy/subscribe/{id}/validate` | 验证订阅源可用性（返回节点数量/协议分布预览，不实际导入） |
| `POST /api/proxy/subscribe/{id}/import-clash` | 从 Clash 配置文件导入订阅 |
| `GET /api/proxy/subscribe/{id}/nodes` | 查看订阅源下的所有节点详情 |

### 3.23 操作日志审计

| 端点 | 功能 |
|------|------|
| `GET /api/audit/operations` | 操作日志列表（谁/什么时间/做了什么操作/结果） |
| `GET /api/audit/operations/{id}` | 操作详情（含请求/响应体） |
| `GET /api/audit/errors` | 错误日志聚合（按错误类型/频率/实例分组） |
| `POST /api/audit/export` | 导出审计日志为 CSV/JSON |

```bash
# 查看最近失败的操作
curl -X GET http://127.0.0.1:19876/api/audit/errors?since=24h&severity=error
```

---

## 四、功能层面改进（不限于 API）

以下是从功能完整性和产品质量角度出发的改进项，可与 API 结合或独立实现。

### 4.1 已有基础设施但未接入的能力

项目内部已有部分完善的基础设施，但未被充分利用或暴露：

| 能力 | 所在包 | 当前状态 | 建议 |
|------|--------|----------|------|
| **任务调度器** | `internal/scheduler` | 已有 CDP Runner + Cron/Interval/Event 触发 + 依赖链 + 重试逻辑，但未暴露为 REST API | 通过 API 暴露任务 CRUD、手动触发、状态查询 |
| **自动化规则引擎** | `internal/automation` | 已有事件匹配、条件评估（支持比较运算符+嵌套字段）、动作执行，但仅内部使用 | 通过 API 暴露规则 CRUD，绑定到内部事件总线 |
| **临时邮箱客户端** | `internal/email` | 支持 mail.tm + Cloudflare Worker 双通道，自动创建、轮询验证码 | 注册场景自动化：与行为模板结合，自动接收验证码并填充 |
| **代理信任评分** | `internal/proxy/trust_score.go` | 已有加权评分模型（延迟/IP健康/历史成功率/区域匹配），但未持久化和查询 | 与代理质量看板 API 结合，提供推荐排序 |
| **代理绑定解析** | `internal/browser/proxy_binding.go` | 已有按快照多字段匹配（SourceID+Name/URL+Config），但无 API 查询 | 通过 API 暴露绑定状态、手动重新绑定 |
| **Webhook 发送器** | `internal/webhook` | 已有 Webhook 发送能力 | 通过 API 配置 + 接入事件总线 |
| **Windows 输入模拟** | `internal/wininput` | 已有时钟级精确的鼠标/键盘/坐标操作 | 可支持无 CDP 时的原生窗口交互，如弹出对话框处理 |
| **备份加密** | `internal/backup` | 已有备份规格定义和加密能力 | 通过 API 触发备份/恢复、查看历史备份 |
| **身份指纹健康评分** | `internal/browser/fingerprint_health.go` | 已有完整评分模型 | 与 API 结合提供指纹模板推荐 |
| **Cookie 登录态验证** | `internal/browser/cookie_verify.go` | 已有 Cookie 有效性验证 | 通过 API 暴露验证结果 |

### 4.2 浏览器核心体验

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **Lightpanda 无头内核支持完善** | 已有 `internal/browser/lightpanda*.go` 但集成尚浅，Lightpanda 比 Chromium 轻 10 倍 | P2 |
| **浏览器崩溃自动恢复** | CDP 断开时自动重启实例并恢复标签页 | P1 |
| **多内核并行启动优化** | 当前 Chromium 启动较慢，可做预热池 | P2 |
| **启动参数校验与冲突检测** | 用户传入的启动参数与指纹参数冲突时告警 | P2 |
| **GPU/WebGL 黑名单检测** | 某些环境下 GPU 黑名单会导致指纹不一致 | P2 |

### 4.3 代理系统

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **代理订阅自动刷新失败告警** | 订阅源过期/不可用时通知用户 | P1 |
| **代理自动切换** | 当前代理不可用时自动切换到同组备用节点 | P1 |
| **代理分组延迟加权路由** | 按最低延迟自动路由，避免手动选择 | P2 |
| **SOCKS5 认证支持增强** | 当前部分场景下认证传递不完整 | P1 |
| **测速结果本地缓存** | 短时间内的重复测速请求直接返回缓存 | P1 |
| **Xray/sing-box 优雅退出** | 当前有 SIGKILL 场景，应优先 SIGTERM 等待 | P1 |

### 4.4 录制与回放

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **CDPExecutor 空实现补全** | `MutatedGetHtml`/`MutatedGetText`/`MutatedScreenshot` 在 middleware 定义了但 executor 没处理，静默跳过 | **P0 — 当前 bug** |
| **录制事件去重与合并** | 连续的相同操作合并为单一步骤 | P2 |
| **回放暂停/继续** | 用户可在回放过程中手动介入 | P2 |
| **回放步骤预览** | 回放前展示即将执行的操作序列 | P2 |
| **录制自动标注** | 识别验证码/登录框等关键节点并自动标记 | P2 |
| **回放失败智能重试** | 元素未找到时等待重试而非立即失败 | P2 |

### 4.5 反检测与身份安全

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **指纹基线自动校准** | 启动后自动比对预期指纹与实际指纹，差异过大时告警 | P1 |
| **WebRTC 泄漏检测** | 检测真实 IP 是否通过 WebRTC 泄漏 | P1 |
| **Canvas/WebGL 指纹一致性随时间检查** | 长时间运行时定期检查指纹是否漂移 | P2 |
| **行为模式随机化增强** | 鼠标轨迹、滚动模式、打字节奏的变异参数可配置层级 | P2 |
| **时区/IP 地理位置一致性校验** | 代理 IP 所在国家与浏览器时区不符时自动修正 | P1 |
| **屏幕分辨率/色深/字体列表完整性校验** | 指纹注入的参数与浏览器实际渲染结果对比 | P2 |

### 4.6 数据与持久化

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **SQLite 数据库迁移框架** | 当前无 schema 版本管理，修改表结构困难 | P1 |
| **敏感字段加密存储** | Cookie、代理密码等应加密后存盘 | P1 |
| **旧数据自动清理** | 录制文件、日志、测速历史定期清理阈值配置 | P2 |
| **导出格式统一** | 所有导出（配置/录制/日志）统一为标准格式 | P2 |
| **录制文件增量存储** | 当前全量存储事件，大录制文件可达数 MB，应支持增量压缩 | P2 |

### 4.7 代理系统专项改进

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **代理自动故障转移** | 当前代理不可用时自动切换到同组延迟最低的备用节点 | P1 |
| **代理订阅自动刷新失败告警** | 订阅源过期/不可用时通知用户 | P1 |
| **SOCKS5 认证传递完善** | 当前部分代理链路认证信息传递不完整 | P1 |
| **测速结果本地缓存** | 短时间内的重复测速请求直接返回缓存结果 | P1 |
| **Xray/sing-box 优雅退出** | 当前有 SIGKILL 场景，应优先 SIGTERM 等待进程自退出 | P1 |
| **手动代理与订阅代理统一管理** | 两种来源的代理在列表、测速、健康检测中统一对待 | P1 |
| **代理分组延迟加权路由** | 按最低延迟自动路由，避免手动选择 | P2 |
| **代理池按需预热** | 预测即将用到的代理提前建连减少首次延迟 | P2 |
| **自建代理 IP 变更自动发现** | 域名解析结果变化时自动更新代理配置 | P2 |
| **代理地区/运营商统计** | 代理池的地理分布和运营商分布概览 | P2 |

### 4.8 监控与运维

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **实例资源使用统计** | 每个浏览器实例的 CPU/内存/网络使用量 | P2 |
| **CDP 连接池管理** | 复用 CDP 连接减少 WebSocket 频繁握手 | P2 |
| **端口使用清理** | 异常退出后残留端口自动回收 | P1 |
| **启动速度优化** | 预缓存内核二进制、预分配端口 | P2 |
| **启动参数校验与冲突检测** | 用户传入的启动参数与指纹参数冲突时告警 | P2 |
| **GPU/WebGL 黑名单检测** | 某些环境下 GPU 黑名单会导致指纹不一致 | P2 |

### 4.9 安全加固

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **API 动态令牌** | 当前只有静态 Key，增加短时效 Token 签发 | P2 |
| **HTTPS 支持** | 为远程访问场景提供 TLS 加密 | P2 |
| **操作权限分级** | 只读/操作/管理三级权限（为团队协作准备） | P3 |
| **请求体大小统一限制** | 所有 POST/PUT 端点统一 `io.LimitReader` | P1 |
| **API 调用频率精细化限流** | 按端点分组设置不同限流阈值 | P2 |

### 4.10 浏览器核心体验

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **Lightpanda 无头内核支持完善** | 已有 `internal/browser/lightpanda*.go` 但集成尚浅，Lightpanda 比 Chromium 轻 10 倍 | P2 |
| **浏览器崩溃自动恢复** | CDP 断开时自动重启实例并恢复标签页 | P1 |
| **多内核并行启动优化** | 当前 Chromium 启动较慢，可做预热池 | P2 |
| **浏览器窗口批量排列预设** | 除 grid 外增加 cascade/tiled/horizontal 等布局 | P2 |
| **实例启动健康检查增强** | 除端口就绪外增加页面可交互性检查 | P2 |
| **多屏/高 DPI 适配** | 多显示器场景下窗口位置计算 | P2 |

### 4.11 自动化与规则引擎

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **规则引擎 API 化** | `internal/automation` 规则引擎暴露为 REST 端点 | P2 |
| **规则条件支持时间窗口** | 条件增加时间范围（仅工作日/仅工作时间触发） | P2 |
| **规则执行历史** | 规则触发的执行记录与结果 | P2 |
| **规则模板库** | 预置常见规则模板（代理失效切换/CDP 断开自动重启） | P2 |
| **规则链式触发** | 规则 A 执行完成后自动触发规则 B | P3 |

### 4.12 测试与质量

| 改进项 | 说明 | 优先级 |
|--------|------|--------|
| **CDPExecutor 空实现补全后的单元测试** | 补全 `MutatedScreenshot`/`GetHtml`/`GetText` 后立即补测试 | P1 |
| **端到端测试框架** | 启动真实浏览器后验证各 API 端点的端到端行为 | P2 |
| **代理解析 parser 的 fuzz 测试** | 恶意/畸形订阅配置不会导致崩溃 | P2 |
| **并发安全性测试** | 用 `-race` 验证多 goroutine 并发操作 | P1 |
| **错误注入测试** | 模拟网络断开/CDP 断开/代理失效场景，验证恢复行为 | P2 |

---

## 五、构建顺序建议

```
阶段1 (P0) — 核心修复 + 配置 CRUD + 代理订阅管理 + 手动代理添加 + 分组管理 + 系统设置
              ├── 补全 CDPExecutor 空实现（Screenshot/GetHtml/GetText 当前 bug）
              ├── 配置 CRUD 完善
              ├── 代理订阅源管理、手动代理添加(domain/port/username/password)、快捷导入(quick-add)
              ├── 分组管理
              └── 系统设置
    ↓
阶段2 (P1) — 基础设施接入 + 质量改进
              ├── SSE 事件流 + Webhook API 化 + 备份 API
              ├── 浏览器崩溃自动恢复 + 端口清理
              ├── 代理自动故障转移 + SOCKS5 认证修复 + 测速缓存
              ├── 请求体大小统一限制(io.LimitReader)
              ├── 指纹基线校准 + WebRTC 泄漏检测 + 时区-IP 校验
              ├── SQLite 迁移框架 + 敏感字段加密
              └── 并发安全性测试(-race)
    ↓
阶段3 (P2) — 差异能力 + 数据智能
              ├── 数据采集 + 账号箱 + 录制编辑(merge/trim/diff/analyze)
              ├── 代理质量看板 + 信任评分持久化 + 订阅验证/预览
              ├── Cookie 健康看板 + 自动续期 + 跨实例转移
              ├── Firefox 内核 + Lightpanda 深度集成
              ├── 定时任务 + 规则引擎 API + 工作流编排
              ├── 代理链路多跳 + TLS/HTTP2 指纹随机化
              ├── 目标站点专项测速 + 代理预热 + 粘性会话
              ├── 录制自动标注 + 回放异常检测
              ├── Canvas 噪声策略配置 + 字体白名单 + 渐变式指纹变异
              ├── Bot 检测仿真测试
              ├── 验证码打码集成 + SMS 集成 + 临时邮箱接入自动化
              ├── 实例快照与恢复
              └── 端到端测试 + fuzz测试 + 错误注入测试
    ↓
阶段4 (P3) — 高级编排 + 安全合规
              ├── 批量操作模板 + 工作流编排 + 条件分支
              ├── 操作审计 + 防篡改审计日志 + 旧数据清理
              ├── WebGPU/AudioContext/Permissions/Battery API 伪造
              ├── 行为模式随机化层级 + 情绪感知行为
              ├── 代理成本追踪 + ROI 分析
              ├── 指纹模板管理 + A/B 测试框架
              ├── 远程协作 + RBAC + 权限分级 + 动态令牌
              ├── HTTPS 支持 + API 精细化限流
              ├── WebSocket 事件流 + 外部系统集成(Telegram/Discord/Slack/Email)
              ├── 代理 IP 变更自动发现 + 健康雷达
              └── Python/JS/Go SDK 生成
    ↓
阶段5 (P4+) — 团队企业 + AI/ML + 平台生态
              ├── 多用户 + 工作区隔离 + 配置共享/锁定 + 审批工作流
              ├── LDAP/SSO + 转售商面板 + 用量计费
              ├── 住宅代理市场 API 集成(BrightData/Oxylabs/IPRoyal)
              ├── 用户行为学习 + 从真人操作合成行为模型
              ├── 自然语言→操作序列(LLM 编排)
              ├── 代理成功率预测模型 + 验证码难度预测 + 反检测建议引擎
              ├── 社区指纹配置基准 + 配置兼容性数据库
              ├── n8n/Zapier 节点 + Puppeteer 兼容层 + 插件系统
              ├── ARM64 + Kubernetes Helm + Prometheus/Grafana
              ├── 分布式代理健康检测网络 + 边缘部署集中管理
              └── 配置模板市场 + 会话市场
```

---

## 六、技术预备项

- 添加 `swagger.json` / OpenAPI 3.0 规范文档
- 统一返回格式：全部端点使用 `{"ok":bool,"data":?,"error":?,"meta":?}` 信封
- 统一错误码：区分参数错误(4xx)、逻辑冲突(409)、内部错误(5xx)
- 添加分页规范：`?offset=&limit=` 参数统一处理
- API 版本前缀：`/api/v1/...` 为向后兼容做准备

---

# 第五部分：更长远的增强领域

以下是与前述内容不重复的全新补充，从 11 个维度提出。

## 5.1 指纹与反检测纵深

| 功能 | 说明 | 价值 |
|------|------|------|
| **WebGPU 指纹模拟** | 当前只采集 WebGPU 信息但不主动伪造；Cloudflare Turnstile 已开始使用 WebGPU 检测自动化 | 补齐高级检测盲区 |
| **AudioContext 指纹主动伪造** | 检测并计算 AudioContext 哈希但不在启动时注入伪造值；fingerprintJS 等库靠此关联用户 | 防止跨配置音频指纹关联 |
| **Navigator.permissions API 伪造** | 自动回复 permissions.query 为 "prompt"/"denied"，防止被检测为自动化环境 | 低投入高回报 |
| **Battery API / NetworkInformation API 伪造** | 伪造充电状态、电量、网络类型、RTT 值 | 补齐小众但有效的检测面 |
| **Screen.orientation / deviceMemory 伪造** | 伪造屏幕方向、设备内存 | 指纹一致性补充 |
| **TLS 指纹随机化 (JA3/JA3S)** | 通过 xray/sing-box 为每配置随机化 TLS Client Hello 指纹 | 避开 CDN 级别的 TLS 指纹识别 |
| **HTTP/2 与 HTTP/3 指纹伪造** | Akamai/Cloudflare 使用 HTTP/2 帧特征识别自动化流量 | 高级 CDN 防御对抗 |
| **Canvas 噪声策略可配置** | 当前无策略选择（加像素 / 改色值 / 随机方块），不同策略对反检测有效性不同 | 精细化反检测 |
| **字体白名单/黑名单** | 控制哪些字体暴露给页面，按配置配置 | 防字体指纹跨配置关联 |
| **浏览器核心版本回退检查** | 已知新版本在某平台被识别时自动建议回退旧版本 | 持续兼容性保障 |
| **Firefox / Safari 内核支持** | 当前仅 Chromium；Firefox 有完全不同的指纹面，部分站点限定 Firefox 访问 | 扩展覆盖站点范围 |
| **移动端浏览器模拟增强** | 当前基础 UA 支持，缺触控事件、viewport、GPU 等深度移动指纹伪造 | 移动端账号管理 |
| **Bot 检测仿真测试** | 内置自动化测试：用配置访问 bot.sannysoft.com / browserleaks.com / recaptcha demo 并生成检测报告 | 上线前体检 |
| **指纹渐变式变异** | 同一配置长期使用时指纹参数缓慢变化而非突变，避免被标记为"指纹刷新" | 长期运营必备 |

## 5.2 代理系统深度扩展

| 功能 | 说明 | 价值 |
|------|------|------|
| **住宅代理市场 API 集成** | 对接 BrightData / Oxylabs / IPRoyal / Smartproxy，自动拉取+轮换+用量追踪 | 直接使用商业代理 |
| **SOCKS4/SOCKS4a/SOCKS5h 协议支持** | 当前只 SOCKS5；不同场景需要不同 SOCKS 变种 | 协议兼容性 |
| **代理链路 (Proxy Chain) 多跳** | 入口→中转→出口多级代理，xray/sing-box 原生支持但未暴露配置 | 极高匿名需求 |
| **粘性会话管理** | 同一配置固定用同一出口 IP 至少 N 分钟才允许轮换 | 防中途换 IP 触发风控 |
| **代理自动预热** | 预测即将使用的代理提前建连，减少首次请求延迟 | 提升首屏速度 |
| **代理成本追踪与 ROI 计算** | 记录每个代理的流量/时间消耗，统计每元成本带来的成功会话数 | 优化代理采购 |
| **目标站点专项测速** | 不仅测 httpbin/gstatic，针对用户指定的目标 URL 测速 | 真实性能数据 |
| **代理 IP 变更自动发现** | 域名解析结果变化或出口 IP 变化时自动更新记录 | 自建代理动态 IP |
| **代理端口扫描/健康雷达** | 对代理池中的 IP:PORT 做存活扫描，标记不可用节点 | 大规模池维护 |
| **代理地区/运营商/ASN 统计** | 代理池构成的全方位统计分析 | 池容量规划 |
| **BrightData/Oxylabs 代理自动轮换集成** | 直接从住宅代理 API 获取新 IP 并注入配置 | 商业化代理即插即用 |
| **代理过滤器（国家/城市/ISP/ASN/类型）** | 按多维度筛选可用代理 | 精准选代理 |
| **代理质量评分持久化与趋势** | 将 trust_score 计算结果存储并提供时间序列查询 | 发现性能退化 |

## 5.3 录制与回放增强

| 功能 | 说明 | 价值 |
|------|------|------|
| **录制差异对比** | 对比两次录制的操作序列差异（哪些步骤变了） | 调试/审计 |
| **录制合并** | 将多次录制合并为一个完整工作流 | 复用 |
| **回放异常检测** | 回放时发现页面出现验证码/弹窗/布局变化时暂停告警 | 避免盲跑 |
| **录制自动标注关键节点** | 识别登录/支付/验证码等阶段并自动标记 | 可读性 |
| **回放步骤预览** | 回放前展示操作序列，用户可确认/编辑 | 安全 |
| **回放暂停/继续** | 回放过程中用户可手动介入再继续 | 人机协同 |
| **多会话录制合并为参数化模板** | 多次录制的类似操作合并为带变量的模板（`{{username}}`） | 泛化复用 |

## 5.4 AI/ML 集成

| 功能 | 说明 | 价值 |
|------|------|------|
| **自然语言→操作序列** | LLM 将一句话指令拆解为可执行动作序列 | 零门槛自动化 |
| **智能代理推荐** | 基于目标站点+时段+配置特征的 ML 模型预测最佳代理 | 降低封号率 |
| **异常行为检测** | 用无监督学习检测配置行为异常（可能被盗用/共享） | 安全告警 |
| **验证码难度预测** | 启动前预测遇验证码概率，高风险时提前准备 | 策略优化 |
| **AI 反检测建议引擎** | 配置被封后 AI 分析原因并给出具体修改建议 | 快速恢复 |
| **社区指纹配置基准** | 匿名聚合各平台成功率最高的指纹配置组合 | 经验复用 |

## 5.5 团队与企业协作

| 功能 | 说明 | 价值 |
|------|------|------|
| **多用户 RBAC** | 管理员/操作员/观察者三级权限 | 企业部署 |
| **配置共享与锁定** | 共享配置给团队成员，同时加锁防并发使用 | 防冲突 |
| **工作区隔离** | 不同客户的项目完全隔离（配置/代理/录制独立） | 代理商模式 |
| **操作活动日志** | 谁在什么时间对哪个配置做了什么操作 | 审计 |
| **配置借出/归还** | 像图书馆一样借出配置，用完归还 | 团队流程 |
| **共享代理池 + 配额** | 多人共享代理池但每个人有带宽额度限制 | 资源管控 |
| **审批工作流** | 创建/删除配置需要上级审批 | 企业合规 |
| **LDAP/SSO 登录** | 企业统一身份认证接入 | 企业部署 |
| **转售商面板** | 子账户+用量计费+账单 | 商业化 |

## 5.6 集成与外部系统

| 功能 | 说明 | 价值 |
|------|------|------|
| **Python/JS/Go SDK** | 从 OpenAPI 规范自动生成客户端 SDK | 开发者友好 |
| **n8n / Node-RED / Zapier 节点** | 通过 webhook 让外部工作流引擎触发浏览器操作 | 生态连接 |
| **WebSocket 实时事件流** | 外部系统可订阅实例状态/代理健康变更推送 | 实时监控 |
| **验证码打码集成** | 对接 2Captcha / Capsolver / Anti-Captcha 自动解决验证码 | 自动化续命 |
| **SMS 验证码集成** | 对接 Twilio / 5sim / sms-activate 接收注册/登录短信 | 账号注册自动化 |
| **临时邮箱集成** | `internal/email` 已有 mail.tm + Cloudflare Worker，需接入自动化流程 | 注册链路闭环 |
| **Puppeteer/Playwright 兼容层** | 现有脚本不改代码即可通过 humanize 中间件获得反检测能力 | 生态兼容 |
| **浏览器扩展自动注入** | 预装并校验指定扩展到每个配置中 | 扩展自动化 |
| **Webhook 动作过滤器** | 按事件类型/级别/来源过滤哪些事件转发到哪个 webhook | 精细化通知 |
| **插件系统 (Lua/JS)** | 用户可编写小脚本扩展 API 行为 | 可扩展性 |

## 5.7 Cookie/会话管理增强

| 功能 | 说明 | 价值 |
|------|------|------|
| **会话健康看板** | 显示所有配置的登录态是否存活、剩余有效期 | 及时发现失效 |
| **自动续期** | Cookie 过期前自动导航到目标站重新登录获取新 Cookie | 免手动续期 |
| **Cookie 转移** | 将配置 A 的 Cookie 复制到配置 B（同站换号） | 快速切换 |
| **批量 Cookie 预热** | 导入 Cookie 后自动访问目标站完成"种 cookie"流程 | 提高 Cookie 存活率 |
| **Cookie 与 LocalStorage/IndexedDB 统一快照** | SPA 站点登录态依赖 IndexedDB，一并导出导入 | 完整会话迁移 |
| **加密会话导出** | 导出带密码保护的会话包，可在不同实例间安全传输 | 安全分享 |

## 5.8 数据分析与智能

| 功能 | 说明 | 价值 |
|------|------|------|
| **配置健康统一看板** | 指纹评分+代理评分+Cookie 存活率+近期成功率聚合展示 | 全局状态一眼清 |
| **平台成功率追踪** | 追踪哪些配置在哪些平台的成功率、存活时长 | 运营决策依据 |
| **A/B 测试框架** | 两个不同指纹/代理配置对同一目标站盲测并对比表现 | 数据驱动优化 |
| **代理 ROI 分析** | 每个代理的成本 vs 带来的成功会话数 | 省钱 |
| **使用热力图** | 每天什么时段配置使用最多、哪个平台最活跃 | 运营洞察 |
| **配置画像** | 自动学习正常配置行为模式，偏离时告警 | 安全 |

## 5.9 安全与合规

| 功能 | 说明 | 价值 |
|------|------|------|
| **防数据泄漏 (DLP)** | 配置 A 的数据不会意外混入配置 B 的文件下载、剪贴板 | 数据隔离 |
| **会话录播回放** | 录下完整操作过程可回放查看（类似浏览器回放） | 取证/调试 |
| **水印截图** | 自动截图时在图片中嵌入配置 ID + 时间戳（肉眼不可见） | 防截屏泄露溯源 |
| **剪贴板隔离** | 不同配置之间剪贴板隔离，防 Ctrl+C/V 数据串台 | 操作安全 |
| **键盘输入加密** | 输入密码时在应用层加密后再传给 CDP | 防输入事件监控 |
| **防篡改审计日志** | 操作日志使用哈希链存储，防事后篡改 | 合规 |

## 5.10 部署与运维

| 功能 | 说明 | 价值 |
|------|------|------|
| **ARM64 支持** | 支持树莓派 / Apple Silicon / AWS Graviton 部署 | 降本 |
| **Kubernetes Helm Chart** | 容器化 headless 集群编排 | 大规模部署 |
| **Prometheus 指标 + Grafana 面板** | 暴露活跃配置数/代理池大小/请求延迟等指标 | 生产监控 |
| **分布式代理健康检测网络** | 多节点共同检测代理可用性，结果聚合 | 更准更快 |
| **边缘部署 + 集中管理** | 多地部署浏览器节点，一个管理端统一调度 | 分布式自动化 |
| **自动更新 + 增量补丁** | 检查更新、bsdiff 增量下载、回滚能力 | 产品化 |
| **系统托盘快速操作** | 最小化到托盘，一键启动/停止/查看状态 | 用户体验 |

## 5.11 跨领域综合能力

| 功能 | 说明 | 价值 |
|------|------|------|
| **用户行为学习** | 录制真人操作→合成行为模型→让其他配置模仿同样行为模式 | 极高拟真度 |
| **情绪感知行为变化** | 根据页面内容（支付成功页→节奏轻快；错误页→犹豫）调整操作速度 | 反高级行为检测 |
| **配置兼容性数据库** | 社区共享："Amazon 登录推荐 fingerprint X + proxy Y + viewport Z" | 经验复用 |
| **从真人操作中学习行为模型** | 录制一段真人使用记录，自动提取鼠标轨迹/滚动节奏/打字韵律的特征，生成可复用的行为模型 | 行为反检测极致 |
| **AI 自然语言指令→操作序列** | "打开小红书搜 Python，前 10 条帖子的标题和点赞数给我"→自动拆解执行→返回结构化数据 | 终极易用性 |
| **CAPTCHA 智能预测** | 启动前根据指纹+代理+目标站历史预测遇验证码概率 | 风险预判 |
| **代理成功率预测模型** | GBDT 模型基于特征预测代理对目标站的成功概率 | 提效降本 |

