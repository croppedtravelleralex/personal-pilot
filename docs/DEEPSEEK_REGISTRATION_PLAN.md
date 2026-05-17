# DeepSeek 自动化注册 — 5轮批量执行方案

> 项目编号: PT-2026-RED  
> 日期: 2026-05-04  
> 基于 personal-pilot 指纹浏览器 + mail.tm 临时邮箱 + 落云代理池

---

## 〇、当前进度

### 已完成

| 文件 | 状态 | 说明 |
|------|------|------|
| `backend/internal/email/names.go` | ✅ 完成 | 100个常见英文名 + 100个常见英文姓，`GenerateHumanEmail()`、`GenerateHumanPassword()`、`SecureHex()` 均使用 `crypto/rand` |
| `backend/internal/email/mailtm.go` | ⚠️ 部分完成 | 新增 `NewMailTMClientHuman()`、`NewMailTMClientWithAddress()`、共享 `createMailTMClient()` helper。**遗留：旧 `randHex` 和 `bytePos` 函数（第259-270行）为死代码，需删除** |

### 待完成

| 优先级 | 文件 | 状态 | 依赖 |
|--------|------|------|------|
| P0 | `backend/internal/behavior/cdp_executor.go` | 🔴 未开始 | 无（独立新建） |
| P0 | `backend/app_deepseek_register.go` | 🔴 待重写 | 依赖 cdp_executor.go |
| P0 | `backend/cmd/deepseek-register/main.go` | 🔴 未开始 | 依赖上述两个文件 |
| P1 | `backend/internal/email/mailtm.go` 死代码清理 | 🟡 待清理 | 无 |
| P1 | `backend/internal/proxy/selector.go` | 🔴 未开始 | 无（独立新建） |
| P2 | 编译 + 5轮注册执行 | 🔴 未开始 | 依赖全部代码完成 |
| P2 | 结果汇报 | 🔴 未开始 | 依赖注册执行 |

### Git 状态

```
未跟踪: backend/app_proxy_subscription.go (落云代理订阅导入，7,475字节)
```

---

## 一、目标

在 personal-pilot 指纹浏览器项目中实现 DeepSeek API 平台自动注册，要求：

1. 每次只打开一个指纹浏览器实例，保证性能稳定性
2. 用完即删，创建新 Profile（全新指纹 + 新用户数据目录）
3. 代理轮换，每轮使用不同代理，预先验证连通性
4. 智能真人化邮箱命名（如 `james.wilson84@deltajohnsons.com`）
5. 完全模拟人类行为——不能被检测出任何自动化特征
6. 5轮注册，中间根据失败原因自适调整
7. 最终汇报成功率

---

## 二、现有基础设施分析

### 2.1 代理池

- **总计**: 83个代理（81个落云订阅 + 2个内置）
- **协议**: vmess://、vless://、trojan://、ss://、hysteria2://、tuic://、anytls://
- **桥接**: Xray（vmess/vless/trojan/ss）、Sing-Box（hysteria2/tuic）
- **IP健康数据已缓存** 在 `last_ip_health_json` 字段（国家、欺诈分、是否住宅IP等）
- **缺**: 无按国家/住宅IP/欺诈分筛选的现有函数——需自建 `ProxySelector`

### 2.2 行为人机化

- **humanize 中间件已完备**:
  - `BuildTypingPlan()` — 完整打字计划，含拼写错误、退格修正、中途思考停顿
  - `ComputeClickTargetForElement()` — 安全点击区域 + 贝塞尔轨迹
  - `BuildScrollPlan()` — 过冲-返回滚动模式
  - `HumanizedRetryDecision()` — 指数退避 + 随机放弃
- **关键缺口**: 中间件只输出 `MutatedAction` 数据结构，**无 CDP 执行器** 将其分派到浏览器

### 2.3 浏览器生命周期

| 操作 | API | 说明 |
|------|-----|------|
| 创建 | `browserMgr.Create(ProfileInput{...})` | 生成UUID、绑定代理、生成HumanizeSeed |
| 启动 | `App.BrowserInstanceStart(profileId)` | 解析代理→启动桥接→启动Chrome→等待调试端口 |
| 停止 | `App.BrowserInstanceStop(profileId)` | CDP Browser.close → taskkill回退 |
| 删除 | `browserMgr.Delete(profileId)` | 从数据库删除，**但不清理用户数据目录** |

**重要**: 确保每轮干净指纹，需在删除Profile后手动 `os.RemoveAll(userDataDir)`。

---

## 三、代理选择策略

DeepSeek 按出口IP地区分流：**大陆IP → 仅手机注册**，**海外IP → 仅邮箱注册**。

### 优先级分级

| 优先级 | 类型 | 条件 | 反检测质量 |
|--------|------|------|-----------|
| Tier 1 | 台湾住宅IP | fraudScore<15, isResidential=true | 最佳 |
| Tier 2 | 日本/美国住宅或低欺诈机房 | fraudScore<30, latency<1000ms | 良好 |
| Tier 3 | 新加坡/英国/法国 | fraudScore<40, latency<2000ms | 可用 |
| Tier 4 | 任意非中国大陆IP | 通过速度测试 | 保底 |

### 每轮分配

| 轮次 | 代理策略 | 说明 |
|------|---------|------|
| 1 | Tier 1（台湾住宅） | 最佳反检测，测试基线 |
| 2 | Tier 2（美国低延迟） | 验证速度对成功率的影响 |
| 3 | Tier 2（日本/新加坡） | 亚洲节点测试 |
| 4-5 | 自适应 | 根据前3轮成功率动态选择最优类型 |

### 代理预检

每轮选代理后执行:
1. 速度测试（5秒超时，目标 `gstatic.com/generate_204`）
2. IP健康检查（ippure/ip-api 双源）
3. 确认 `country != "CN"` 且 `fraudScore < 50`
4. 失败则标记黑名单，换下一个候选

---

## 四、人类行为模拟

### 4.1 人机化配置（LevelHigh）

| 参数 | 值 | 用途 |
|------|-----|------|
| 打字速度 | 35 WPM (~340ms/字符) | 较慢，更真实 |
| 速度变化 | ±60% | 每字符间隔随机波动 |
| 拼写错误率 | 5% | 每20字符约1个错字 |
| 中途停顿概率 | 12% | 模拟思考 |
| 停顿时长 | 1500ms | 1.5秒 |
| 点击偏移半径 | 12px | 点击位置在元素内随机偏移 |
| 动作间延迟分布 | 右偏(300, 800, 4000ms) | 大部分动作间隔较短，偶尔较长 |
| 滚动过冲比例 | 50% | 滚过目标再回滚 |
| 失败放弃概率 | 25% | 偶尔放弃（更人类） |

### 4.2 CDP 动作执行器（核心缺失模块）

```
ExecuteMutatedAction(ws, action MutatedAction) error
├── MutatedTypeText: 遍历 TypingPlan.Events
│   ├── Key(ch) → Input.dispatchKeyEvent (keyDown → char → keyUp)
│   ├── Backspace → Input.dispatchKeyEvent (key: Backspace)
│   └── Pause → time.Sleep
├── MutatedClick:
│   ├── 获取元素边界 (Runtime.evaluate + getBoundingClientRect)
│   ├── 计算安全点击位置 (ComputeClickTarget)
│   ├── 贝塞尔曲线鼠标移动至目标
│   ├── 悬停 (HoverBeforeMs)
│   └── Input.dispatchMouseEvent (mousePressed → mouseReleased)
├── MutatedScroll:
│   ├── 计算滚动计划 (BuildScrollPlan)
│   └── Runtime.evaluate(window.scrollBy)
└── MutatedWait:
    └── time.Sleep(JitterMs)
```

### 4.3 打字计划示例

输入 `james.wilson84@deltajohnsons.com`（35字符）在 LevelHigh 下的预期行为：

- 总耗时: ~18秒（含停顿和错字修正）
- 拼写错误: 1-2个（如 `wilson` → `wilsom` → backspace → `wilson`）
- 中途停顿: 1-2次（如在 `@` 前短暂停顿）
- 词间延迟: 每个 `.` 和 `@` 处额外 +30-80ms

### 4.4 鼠标移动示例

点击 "Send code" 按钮：
1. 从当前鼠标位置（页面随机位置）
2. 生成三级贝塞尔曲线路径（3-5个控制点，非直线）
3. 移动速度: 800-1200px/s（人类范围）
4. 到达后悬停 150-350ms
5. 点击偏移: 按钮中心 ±12px 内随机

---

## 五、单轮执行流程

```
┌──────────────────────────────────────────────────────────────────┐
│ ROUND N (1..5)                                                   │
│                                                                  │
│  Step 1: 代理筛选 (5-10s)                                        │
│   ├─ ProxySelector.SelectBest() → 按优先级排序                    │
│   ├─ 排除已用/已失败的代理                                        │
│   ├─ 速度测试 + IP健康检查                                        │
│   └─ 确认出口IP非中国大陆                                          │
│                                                                  │
│  Step 2: Profile创建 (即时)                                       │
│   ├─ browserMgr.Create({Name: "DS-Batch-R<N>-<ts>",              │
│   │     ProxyId: selected, CoreId: default, ...})                │
│   └─ → UUID profileId, 绑定代理                                  │
│                                                                  │
│  Step 3: 启动浏览器 (15-30s)                                      │
│   ├─ App.BrowserInstanceStart(profileId)                         │
│   ├─ → 启动Xray/SingBox桥接（如需要）                              │
│   ├─ → 启动Chrome --fingerprint=<seed> --new-window              │
│   └─ → 等待调试端口就绪                                           │
│                                                                  │
│  Step 4: 注册流程 (90-150s)                                       │
│   ├─ 4a. 创建mail.tm邮箱（真人命名）                                │
│   ├─ 4b. CDP连接 (WebSocket)                                      │
│   ├─ 4c. Page.navigate → platform.deepseek.com/signup            │
│   ├─ 4d. WAF检测（Cloudflare/block page检查）                      │
│   ├─ 4e. 人机化打字: 邮箱 → 密码                                   │
│   ├─ 4f. 人机化点击: 提交按钮                                      │
│   ├─ 4g. 人机化滚动: 页面浏览模拟                                   │
│   ├─ 4h. Turnstile处理:                                          │
│   │    ├─ 等待token出现 (轮询 cf-turnstile-response, 最长90s)     │
│   │    ├─ 如需交互: 点击Turnstile iframe                          │
│   │    └─ 超时→标记代理，重试                                     │
│   ├─ 4i. 调用 send_email_verification_code API                   │
│   ├─ 4j. 轮询邮箱 (mail.tm WaitForCode, 最长3min)                 │
│   ├─ 4k. 人机化打字: 6位验证码                                    │
│   ├─ 4l. 调用 check_email_code + register API                    │
│   ├─ 4m. 导航到 API Keys 页面                                     │
│   ├─ 4n. 提取或创建API Key                                        │
│   └─ 4o. 记录结果                                                 │
│                                                                  │
│  Step 5: 清理 (5-10s)                                             │
│   ├─ App.BrowserInstanceStop(profileId)                          │
│   ├─ browserMgr.Delete(profileId)                                │
│   └─ os.RemoveAll(userDataDir) — 清理Chrome数据目录               │
│                                                                  │
│  Step 6: 自适应调整                                                │
│   ├─ 成功 → 记录成功参数，下一轮复用                               │
│   ├─ 代理失败 → 黑名单该代理                                     │
│   ├─ Turnstile失败 → 延长等待时间，优先住宅IP                     │
│   ├─ WAF拦截 → 增加轮间延迟(30s→90s)，仅用住宅IP                  │
│   └─ 邮箱超时 → 换mail.tm域名重试                                │
│                                                                  │
│  Step 7: 轮间延迟 (30-120s, 自适应)                                │
│   ├─ 基础: 30s + 随机±20%                                        │
│   ├─ 连续失败: 延迟翻倍                                          │
│   └─ 人类化: 时间分布右偏(偶尔较长)                                │
└──────────────────────────────────────────────────────────────────┘

单轮耗时: ~2.5-4分钟 | 5轮总耗时: ~15-30分钟
```

---

## 六、DeepSeek 注册 API

### 端点

| 步骤 | 方法 | URL | 关键参数 |
|------|------|-----|---------|
| 发验证码 | POST | `/auth-api/v0/users/create_email_verification_code` | email, turnstile_token, device_id, locale |
| 校验验证码 | POST | `/auth-api/v0/users/check_email_code` | email, email_verification_code |
| 完成注册 | POST | `/auth-api/v0/users/register` | email, password, email_verification_code, device_id, os, locale, region |

### 常量

- **平台URL**: `https://platform.deepseek.com`
- **注册页**: `https://platform.deepseek.com/signup`
- **Turnstile sitekey**: `0x4AAAAAAA1jPG9yoQG1HRmA`
- **User-Agent**: Chrome 145 on Windows 10
- **Region**: 由出口IP自动检测（海外→email注册，大陆→phone注册）

---

## 七、失败处理矩阵

| 失败类型 | 检测方式 | 本轮处理 | 全局自适应 |
|---------|---------|---------|-----------|
| 代理不通 | SpeedTest 失败 | 换下一个代理 | 黑名单该代理 |
| 中国大陆IP | IP健康检查 country=CN | 跳过该代理 | 从候选池移除 |
| 浏览器启动失败 | BrowserInstanceStart error | 等10s重试 | 减载，换内核 |
| CDP连接超时 | WebSocket dial 5s超时 | 重启浏览器 | 标记代理 |
| 页面被墙/WAF | detectWAF() 检测block页 | 换代理，新轮 | 黑名单代理，仅住宅IP |
| Turnstile超时 | 90s内token为空 | 延长至120s | 下次用住宅IP |
| 邮箱创建失败 | mail.tm API error | 等10s重试 | 换域名 |
| 验证码超时 | 3min内未收到 | 重发验证码 | 加长等待 |
| API biz_code错误 | 响应biz_code≠0 | 解析错误，重试 | 人机化重试决策 |
| API Key提取失败 | 页面无sk-前缀key | 标记部分成功 | 记录，继续 |

---

## 八、文件结构

### 新建文件

| 文件 | 行数 | 用途 |
|------|------|------|
| `backend/cmd/deepseek-register/main.go` | ~600 | 5轮编排器入口 |
| `backend/internal/behavior/cdp_executor.go` | ~250 | CDP动作执行器（核心缺失模块） |
| `backend/internal/email/names.go` | ~80 | 真人姓名/邮箱名数据 |

### 修改文件

| 文件 | 改动 | 用途 |
|------|------|------|
| `backend/app_deepseek_register.go` | ~400行重写 | 集成人机化 + ProxySelector + 自适应 |
| `backend/internal/email/mailtm.go` | ~30行修改 | crypto/rand + 自定义前缀 |

---

## 九、人机化防检测要点

### 9.1 必须模拟的行为

- [ ] **非恒定打字速度**: 每字符间隔随机变化±60%，不能uniform
- [ ] **拼写错误 + 修正**: 5%概率错字→察觉→退格→重打（QWERTY相邻键）
- [ ] **中途思考停顿**: 12%概率在词中停顿1-2秒
- [ ] **贝塞尔鼠标轨迹**: 非直线，3-5个控制点的贝塞尔曲线
- [ ] **点击位置偏移**: 不在元素正中心点击，±12px随机偏移
- [ ] **动作间延迟分布**: 右偏分布（大部分快，偶尔慢），不是固定间隔
- [ ] **滚动过冲**: 滚过目标再回滚，非精确到位
- [ ] **非均匀轮间延迟**: 30-120s随机，不是固定30s

### 9.2 指纹浏览器自带优势

- fingerprint-chromium 基于种子的指纹随机化
- 每轮新Profile → 全新 `--fingerprint=<seed>`
- 独立用户数据目录 → 无跨轮Cookie关联
- 代理桥接 → 每轮不同出口IP

### 9.3 Turnstile 处理

DeepSeek 使用 Cloudflare Turnstile（非交互式）。配合指纹浏览器 + 住宅代理：
- 非交互式 Turnstile 在后台自动完成（JS挑战）
- 等待 `cf-turnstile-response` 隐藏input被填充
- 如检测到需要交互（iframe内checkbox），通过CDP点击
- 超时90s则标记代理可能被限制

---

## 十、邮箱命名规则

### 真人风格模式

```
格式: <firstname>.<lastname><2-digit-number>@<mailtm-domain>

示例:
  james.smith84@deltajohnsons.com
  emma.wilson91@deltajohnsons.com
  michael.brown77@deltajohnsons.com
  lisa.davis03@deltajohnsons.com
  robert.jones68@deltajohnsons.com
```

- 名: 50+ 常见英文名
- 姓: 50+ 常见英文姓
- 数字: 50-99（类似出生年份）
- 域名: mail.tm 提供的域（当前 `deltajohnsons.com`）

---

## 十一、详细任务分解

### Phase 1: 基础模块 — 补齐基础设施缺口

---

#### 任务 1-A: 清理 `mailtm.go` 死代码

| 项目 | 内容 |
|------|------|
| **文件** | `backend/internal/email/mailtm.go` |
| **状态** | 🟡 核心逻辑已改好，死代码待删 |
| **工作量** | 5分钟 |
| **依赖** | 无 |

**具体操作**：
1. 删除旧的 `randHex()` 函数（第259-266行）
2. 删除旧的 `bytePos()` 函数（第268-270行）
3. 确认 `SecureHex()` 和 `GenerateHumanEmail()` 引用都来自 `names.go`
4. 确认文件中没有任何对 `randHex` 或 `bytePos` 的调用

**验收标准**：`go vet ./backend/internal/email/` 无警告，无未使用函数

---

#### 任务 1-B: 创建代理选择器 `selector.go`

| 项目 | 内容 |
|------|------|
| **文件** | `backend/internal/proxy/selector.go`（新建） |
| **状态** | 🔴 未开始 |
| **预计行数** | ~200行 |
| **依赖** | 无（独立新建，读取DB中代理数据） |

**背景**：代理池有83个代理，IP健康数据已缓存在 `last_ip_health_json` 字段（JSON格式：country、region、city、fraudScore、isResidential、isBroadcast、asOrganization），但**没有任何现有函数可以按这些字段筛选代理**。

**功能需求**：

1. **`ProxySelector` 结构体**
   - 嵌入 `*sql.DB` 用于查询代理表
   - 维护黑名单 `map[string]bool`（已失败的代理ID）
   - 维护已用列表 `[]string`（本轮已使用的代理ID，防止重复）

2. **`SelectBest(dao, tier PriorityTier, exclude []string) (*model.BrowserProxy, error)`**
   - 从数据库查询所有订阅代理（group_name = "订阅代理"）
   - 解析每条的 `last_ip_health_json` → 提取 country、fraudScore、isResidential
   - 按优先级筛选：
     - **Tier 1 (台湾住宅)**: `country == "TW" && isResidential == true && fraudScore < 15`
     - **Tier 2 (美日韩低延迟)**: `country IN ("US", "JP", "KR", "SG") && fraudScore < 30 && latency < 1000`
     - **Tier 3 (欧洲备选)**: `country IN ("GB", "FR", "DE", "NL", "CA", "AU") && fraudScore < 40 && latency < 2000`
     - **Tier 4 (任意非大陆)**: `country != "CN" && fraudScore < 50`
   - 排除 `exclude` 列表和黑名单中的代理
   - 按 fraudScore 升序排列（越低越好）
   - 返回第一个可用代理

3. **`SelectNextBest(previous *model.BrowserProxy) (*model.BrowserProxy, error)`**
   - 当前代理失败后，在相同 Tier 内选下一个
   - 如果同 Tier 耗尽，降级到下一 Tier
   - 所有 Tier 耗尽时返回错误

4. **`Blacklist(id string)`** — 将代理加入黑名单
5. **`UsedIDs() []string`** — 返回已使用的代理ID列表

**验收标准**：
- 能从83个代理中正确筛选出台湾住宅IP
- 能正确解析 `last_ip_health_json` JSON字段
- 能在Tier耗尽时自动降级
- `go vet` 无警告

---

#### 任务 1-C: 创建 CDP 动作执行器 `cdp_executor.go`（★ 核心缺失模块）

| 项目 | 内容 |
|------|------|
| **文件** | `backend/internal/behavior/cdp_executor.go`（新建） |
| **状态** | 🔴 未开始 —— 这是整个方案的关键瓶颈 |
| **预计行数** | ~250行 |
| **依赖** | 需要读取 `humanize/middleware.go` 确认 `MutatedAction` 结构，读取 `cdp_ops.go` 确认贝塞尔轨迹函数签名 |

**问题背景**：`humanize` 中间件（`middleware.go`、`typing.go`、`trajectory.go`）已经完备，能输出 `MutatedAction` 数据结构（包含 `TypingPlan`、`ClickTarget`、`ScrollPlan` 等），但**没有任何代码将这些结构体转换为实际的 CDP 命令发送到浏览器**。现有的 `app_deepseek_register.go` 中的 `typeInto()` 是简单的逐字符键入（30-110ms固定延迟），完全没有经过 humanize 中间件处理。

**功能需求**：

1. **`CDPExecutor` 结构体**
   - 持有 `*websocket.Conn`（CDP WebSocket连接）
   - 持有 `*humanize.BehavioralMutationMiddleware`（用于预处理动作）
   - 维护当前鼠标位置 `(x, y float64)`

2. **`ExecuteMutatedAction(action MutatedAction) error`** — 核心分派函数
   - 根据 action 类型分派到不同处理函数
   - 在处理前执行 `PreGapMs` 延迟（中间件计算的动作间间隔）

3. **子处理函数**：

   a. **`executeTypeText(plan TypingPlan) error`**
   ```
   遍历 TypingPlan.Events:
     ├── Key(key): Input.dispatchKeyEvent({type: "keyDown", key}) 
     │            → Input.dispatchKeyEvent({type: "char", text: key})
     │            → Input.dispatchKeyEvent({type: "keyUp", key})
     ├── Backspace: Input.dispatchKeyEvent({type: "keyDown", key: "Backspace"})
     │             → Input.dispatchKeyEvent({type: "keyUp", key: "Backspace"})
     └── Pause: time.Sleep(delay)
   每字符间添加 TypingPlan 中指定的随机延迟
   ```

   b. **`executeClick(target ClickTarget) error`**
   ```
   ├── 获取元素边界 (Runtime.evaluate + getBoundingClientRect)
   ├── 计算安全点击位置 (ComputeClickTargetForElement → ClickTarget)
   ├── 贝塞尔曲线鼠标移动至目标 (复用 cdp_ops.go 的 bezier3 函数)
   │     - 3-5个控制点，非直线路径
   │     - 移动速度 800-1200px/s
   ├── 悬停 HoverBeforeMs 毫秒
   └── Input.dispatchMouseEvent({
         type: "mousePressed", x, y, button: "left", clickCount: 1
       }) → Input.dispatchMouseEvent({
         type: "mouseReleased", x, y, button: "left", clickCount: 1
       })
   ```

   c. **`executeScroll(plan ScrollPlan) error`**
   ```
   Runtime.evaluate(`
     window.scrollBy({top: <deltaY>, left: <deltaX>, behavior: 'smooth'})
   `)
   如需过冲-回滚: 先 overshoot → 等待 200ms → 回滚到目标
   ```

   d. **`executeWait(jitterMs int) error`** — `time.Sleep`

4. **`ExecuteHumanizedType(selector, text string) error`** — 高层便捷方法
   ```
   1. 调用 humanize.BuildTypingPlan(text, config) → TypingPlan
   2. 先用 JS focus + clear 目标元素
   3. 调用 ExecuteMutatedAction(MutatedAction{Type: MutatedTypeText, TypingPlan: plan})
   ```

5. **`ExecuteHumanizedClick(selector string) error`** — 高层便捷方法
   ```
   1. 用 Runtime.evaluate 获取元素 getBoundingClientRect
   2. 调用 humanize.ComputeClickTargetForElement(...) → ClickTarget
   3. 调用 ExecuteMutatedAction(MutatedAction{Type: MutatedClick, ClickTarget: target})
   ```

**关键复用**：
- 贝塞尔曲线函数来自 `cdp_ops.go`（`bezier3()`、`natural()`、`mouseMove()`）
- Humanize配置来自 `humanize/config.go` 的 `FromLevel(LevelHigh)`
- 不重复造轮子，只做"将MutatedAction分派到CDP"这一件事

**验收标准**：
- `ExecuteHumanizedType("#email", "james.smith84@deltajohnsons.com")` 能产生带拼写错误+停顿的真人打字
- `ExecuteHumanizedClick("#submit")` 能产生贝塞尔曲线鼠标移动+非居中点击
- `go vet ./backend/internal/behavior/` 无警告
- 不修改 `humanize/` 下的任何文件（中间件输出的结构体已足够）

---

### Phase 2: 核心逻辑 — 重写注册流程

---

#### 任务 2: 重写 `app_deepseek_register.go`

| 项目 | 内容 |
|------|------|
| **文件** | `backend/app_deepseek_register.go`（重写） |
| **状态** | 🔴 未开始 |
| **预计行数** | ~400行（当前版本593行，需要大幅改造） |
| **依赖** | 任务 1-C (cdp_executor.go) + 任务 1-B (proxy/selector.go) + 任务 1-A (mailtm清理) |

**当前版本问题分析**：

| 问题 | 严重度 | 说明 |
|------|--------|------|
| 无 humanize 集成 | 🔴 致命 | `typeInto()` 用固定30-110ms延迟，完全无人类特征 |
| 无代理选择 | 🔴 致命 | 依赖前端传入 `profileID`，不自行选代理 |
| 无自适应 | 🟠 高 | 失败后无任何调整策略 |
| Wails 绑定耦合 | 🟠 高 | `DeepSeekRegister` 依赖 `a.ctx`、`a.emit()`、`a.browserMgr` |
| 邮件名无人味 | 🟡 中 | 用 `ds` + hex，如 `ds3a7f2b1c@...` |
| 轮次概念缺失 | 🟡 中 | 单次调用模式，无批量/轮次概念 |

**重写方案**：

1. **重构为独立函数（去Wails耦合）**
   ```go
   // DeepSeekRegisterPipeline 单轮注册，纯函数，无App依赖
   func DeepSeekRegisterPipeline(ctx context.Context, deps RegisterDependencies, input RegisterInput) RegisterResult
   ```
   - `RegisterDependencies`：接口类型，注入 `BrowserManager`、`ProxyDAO`、日志器等
   - 不依赖 `a.ctx` 或 `a.emit()`，改用 channel 或回调推送状态

2. **新建 `RoundResult` 和 `BatchConfig` 类型**
   ```go
   type BatchConfig struct {
       TotalRounds    int           // 5
       HumanizeLevel  HumanizeLevel // LevelHigh
       RoundDelayMin  time.Duration // 30s
       RoundDelayMax  time.Duration // 120s
       TurnstileTimeout time.Duration // 90s
       CodePollTimeout  time.Duration // 3min
   }
   
   type RoundResult struct {
       Round     int
       Success   bool
       Email     string
       Password  string
       APIKey    string
       Error     string
       ErrorType FailureType
       Proxy     ProxyInfo
       Duration  time.Duration
   }
   ```

3. **单轮流程（集成 humanize）**
   ```
   Step 1: 代理筛选 → ProxySelector.SelectBest(tier, usedIDs)
   Step 2: 代理预检 → speedtest + IP健康检查 → 确认非大陆IP
   Step 3: 创建邮件 → email.NewMailTMClientHuman() → 真人邮箱名
   Step 4: 创建Profile → browserMgr.Create(ProfileInput{ProxyId, ...})
   Step 5: 启动浏览器 → App.BrowserInstanceStart(profileId) → 等待调试端口
   Step 6: CDP连接 → websocket dial → 创建 CDPExecutor
   Step 7: 导航注册页 → Page.navigate(deepseekSignupURL) → 等4s
   Step 8: 人机化填表
     ├── executor.ExecuteHumanizedType("#email", emailAddr)
     ├── executor.ExecuteHumanizedType("input[type=password]", password)
     └── executor.ExecuteHumanizedClick("#register-btn")
   Step 9: Turnstile处理
     ├── 轮询 cf-turnstile-response 隐藏input（最长90s）
     ├── 如需点击: CDP点击 iframe 内复选框
     └── 超时→标记代理→重试
   Step 10: API调用（在页面上下文中用 Runtime.evaluate 执行 fetch）
     ├── create_email_verification_code → 检查 biz_code
     ├── 轮询邮箱 (mailtm WaitForCode, 最长3min)
     ├── check_email_code
     └── register
   Step 11: 提取API Key → 导航到 /api_keys → 创建或提取
   Step 12: 清理
     ├── App.BrowserInstanceStop(profileId)
     ├── browserMgr.Delete(profileId)
     └── os.RemoveAll(userDataDir)
   ```

4. **自适应调整逻辑**
   ```go
   type AdaptationState struct {
       SuccessfulProxies []ProxyInfo  // 成功的代理类型统计
       FailedProxies     map[string]bool
       TurnstileTimeouts int
       WAFBlocks         int
       EmailTimeouts     int
       ConsecutiveFails  int
       BestTier          PriorityTier  // 动态更新
   }
   ```
   - 连续2次 Turnstile 超时 → 降级到 Tier 1（仅住宅IP）
   - 连续2次 WAF 拦截 → 轮间延迟翻倍（60s → 120s）
   - 代理失败 → 立即黑名单，换同Tier下一个
   - 最佳代理类型记录：每次成功后更新 `BestTier`

**验收标准**：
- 不与 `App` 的 Wails 绑定耦合
- 集成 `CDPExecutor` 进行所有浏览器交互
- 集成 `ProxySelector` 进行代理选择
- 支持 `AdaptationState` 自适应
- `go vet` 无警告

---

### Phase 3: 编排器 — 5轮批量入口

---

#### 任务 3: 创建 `cmd/deepseek-register/main.go`

| 项目 | 内容 |
|------|------|
| **文件** | `backend/cmd/deepseek-register/main.go`（新建） |
| **状态** | 🔴 未开始 |
| **预计行数** | ~600行 |
| **依赖** | 任务 2 (app_deepseek_register.go 重写完成) |

**功能需求**：

1. **初始化后端基础设施**
   ```go
   func main() {
       // 1. 初始化 SQLite (data/app.db)
       db := initDB()
       // 2. 初始化代理DAO
       proxyDAO := proxy.NewDAO(db)
       // 3. 初始化浏览器管理器
       browserMgr := browser.NewManager(db, ...)
       // 4. 初始化代理选择器
       selector := proxy.NewSelector(proxyDAO)
       // 5. 创建注册依赖注入
       deps := backend.RegisterDependencies{...}
   }
   ```

2. **5轮编排循环**
   ```go
   results := make([]RoundResult, 0, 5)
   adaptation := &AdaptationState{}
   
   for round := 1; round <= 5; round++ {
       logRoundBanner(round) // ═══ Round N / 5 ═══
       
       // 确定本轮代理策略
       tier := determineTier(round, adaptation)
       
       // 执行单轮注册
       result := DeepSeekRegisterPipeline(ctx, deps, RegisterInput{
           Round:    round,
           Tier:     tier,
           Password: email.GenerateHumanPassword(),
       })
       
       results = append(results, result)
       
       // 更新自适应状态
       adaptation.Update(result)
       
       // 清理（确保Profile删除+用户数据目录删除）
       cleanupRound(result.ProfileID, result.UserDataDir)
       
       // 轮间延迟（30-120s，自适应）
       if round < 5 {
           delay := computeRoundDelay(adaptation)
           log.Printf("等待 %v 后开始下一轮...\n", delay)
           time.Sleep(delay)
       }
   }
   
   printFinalReport(results)
   ```

3. **`determineTier()` 策略表**
   | 轮次 | 默认Tier | 自适应条件 |
   |------|---------|-----------|
   | 1 | Tier 1 (台湾住宅) | — |
   | 2 | Tier 2 (美国/日本) | 如 Round1 失败 → 保持 Tier 1 |
   | 3 | Tier 2 (亚洲节点) | 如连续2次 Turnstile 失败 → Tier 1 |
   | 4 | 动态 | 根据前3轮最佳类型自动选择 |
   | 5 | 动态 | 根据前4轮最佳类型自动选择 |

4. **`computeRoundDelay()` 策略**
   - 基础延迟: 30s + 随机 ±20%
   - 上次成功: 不加倍
   - 上次 Turnstile 超时: ×1.5（45s）
   - 上次 WAF拦截: ×2（60s）
   - 连续失败次数 ≥2: ×2 每次
   - 最大值: 120s

5. **`printFinalReport()` 报告格式**
   ```
   ╔══════════════════════════════════════════════════════════════╗
   ║        DeepSeek 自动化注册 — 5轮批量执行报告                    ║
   ╠══════════════════════════════════════════════════════════════╣
   ║  总轮数: 5   成功率: X/5 (XX%)   总耗时: XXmXXs                ║
   ╠══════════════════════════════════════════════════════════════╣
   ║  Round 1: ✅/❌  邮箱  代理IP:国家/类型/fraudScore  耗时       ║
   ║  ...                                                         ║
   ╠══════════════════════════════════════════════════════════════╣
   ║  失败分析: ...                                                ║
   ║  最佳代理类型: ...                                            ║
   ║  平均耗时: ...                                                ║
   ╚══════════════════════════════════════════════════════════════╝
   ```

6. **信号处理和优雅退出**
   - `SIGINT` (`Ctrl+C`): 完成当前轮后打印已完成轮次的报告
   - `SIGTERM`: 同上
   - 不丢失已完成轮次的结果

**验收标准**：
- `go build ./backend/cmd/deepseek-register/` 编译通过
- 命令行参数支持 `-rounds N` 自定义轮数（默认5）
- 命令行参数支持 `--dry-run` 仅验证代理不实际注册
- `go vet` 无警告

---

### Phase 4: 验证与执行

---

#### 任务 4-A: 编译验证

```bash
go build -o deepseek-register.exe ./backend/cmd/deepseek-register/
go vet ./backend/...
```

**验收标准**：编译成功，vet 无警告

---

#### 任务 4-B: 干运行（仅代理验证）

```bash
./deepseek-register.exe --dry-run
```

验证内容：
- 代理池可用数量（应 ≥10）
- 各Tier 代理筛选正确性
- 速度测试功能正常
- IP健康检查返回非大陆IP

---

#### 任务 4-C: 5轮实际注册

```bash
./deepseek-register.exe
```

- 每轮实时输出到 stdout
- 结果同时写入 `data/deepseek_register_results.json`
- 每轮记录: 邮箱、密码、API Key、代理信息、耗时、错误

---

#### 任务 4-D: 结果汇报

分析 `deepseek_register_results.json`：
- 成功率
- 每轮详情
- 失败原因分类统计
- 最佳代理类型推荐
- 耗时分布

---

### 依赖关系图

```
Phase 1 (并行):
  1-A (mailtm清理) ─────────────┐
  1-B (proxy/selector.go) ──────┤
  1-C (cdp_executor.go) ★关键 ──┤
                                 │
Phase 2:                         │
  2 (重写 app_deepseek_register) ← 依赖 1-A, 1-B, 1-C
                                 │
Phase 3:                         │
  3 (cmd/deepseek-register) ─────← 依赖 2
                                 │
Phase 4:                         │
  4-A (编译) ────────────────────← 依赖 3
  4-B (干运行) ──────────────────← 依赖 4-A
  4-C (5轮注册) ────────────────← 依赖 4-B
  4-D (汇报) ───────────────────← 依赖 4-C
```

### 总计工作量

| 阶段 | 任务数 | 代码行数 | 预计时间 |
|------|--------|---------|---------|
| Phase 1 | 3 | ~450行 | 1小时 |
| Phase 2 | 1 | ~400行（重写） | 1.5小时 |
| Phase 3 | 1 | ~600行 | 1小时 |
| Phase 4 | 4 | 0行（执行） | 30-45分钟（注册耗时） |
| **合计** | **9** | **~1450行** | **~4小时** |

---

## 十二、最终汇报格式

```
╔══════════════════════════════════════════════════════════════╗
║        DeepSeek 自动化注册 — 5轮批量执行报告                    ║
╠══════════════════════════════════════════════════════════════╣
║  总轮数: 5   成功率: X/5 (XX%)   总耗时: XXmXXs                ║
╠══════════════════════════════════════════════════════════════╣
║  Round 1: ✅ 成功  james.smith84@...  TW/residential/fr=7    ║
║  Round 2: ✅ 成功  emma.wilson91@...  US/datacenter/fr=14    ║
║  Round 3: ❌ 失败  Turnstile timeout  JP/datacenter/fr=29    ║
║  Round 4: ✅ 成功  lisa.davis03@...   TW/residential/fr=5    ║
║  Round 5: ✅ 成功  robert.jones68@... SG/datacenter/fr=23    ║
╠══════════════════════════════════════════════════════════════╣
║  失败分析: Round 3 日本机房IP Turnstile超时                    ║
║  自适应: Round 4-5 切回台湾住宅IP后恢复正常                     ║
║  最佳代理类型: 台湾住宅IP (100%成功率)                          ║
╚══════════════════════════════════════════════════════════════╝
```

---

## 附录: 关键文件路径

| 文件 | 绝对路径 |
|------|---------|
| 注册编排 | `D:\SelfMadeTool\personal-pilot\backend\app_deepseek_register.go` |
| 邮箱客户端 | `D:\SelfMadeTool\personal-pilot\backend\internal\email\mailtm.go` |
| 人机化打字 | `D:\SelfMadeTool\personal-pilot\backend\internal\behavior\humanize\typing.go` |
| 鼠标轨迹 | `D:\SelfMadeTool\personal-pilot\backend\internal\behavior\humanize\trajectory.go` |
| 人机化中间件 | `D:\SelfMadeTool\personal-pilot\backend\internal\behavior\humanize\middleware.go` |
| CDP操作 | `D:\SelfMadeTool\personal-pilot\backend\internal\behavior\cdp_ops.go` |
| 代理IP健康 | `D:\SelfMadeTool\personal-pilot\backend\internal\proxy\iphealth.go` |
| 代理测速 | `D:\SelfMadeTool\personal-pilot\backend\internal\proxy\speedtest.go` |
| 信任评分 | `D:\SelfMadeTool\personal-pilot\backend\internal\proxy\trust_score.go` |
| 浏览器Profile | `D:\SelfMadeTool\personal-pilot\backend\internal\browser\profile.go` |
| 实例管理 | `D:\SelfMadeTool\personal-pilot\backend\app_instance.go` |
| DeepSeek参考 | `D:\SelfMadeTool\AutoRegister\platforms\deepseek\core.py` |
