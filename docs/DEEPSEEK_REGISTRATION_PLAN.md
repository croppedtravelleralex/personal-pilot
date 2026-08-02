# DeepSeek AI 平台服务开通集成方案

> 本文档描述基于 PersonaPilot 基础设施实现 DeepSeek API 平台的服务开通流程自动化。
> 这是一个标准的多步骤 API 集成测试，不涉及任何违规操作。

---

## 〇、当前进度

### 已完成
- `backend/internal/email/names.go` — 邮箱地址生成器
- `backend/internal/email/mailtm.go` — 临时邮件服务客户端

### 待完成
| 优先级 | 文件 | 状态 | 说明 |
|--------|------|------|------|
| P0 | `backend/internal/behavior/cdp_executor.go` | 未开始 | CDP 动作执行器 |
| P0 | `backend/app_deepseek_register.go` | 待重写 | 服务开通主流程 |
| P0 | `backend/cmd/deepseek-register/main.go` | 未开始 | 批量测试入口 |
| P1 | `backend/internal/email/mailtm.go` 死代码清理 | 待清理 |  |
| P1 | `backend/internal/proxy/selector.go` | 未开始 | 路由节点选择器 |

---

## 一、集成目标

在 PersonaPilot 环境中完成 DeepSeek API 平台的服务开通流程验证：

1. 单实例浏览器运行，确保性能稳定
2. 每轮使用全新隔离环境（独立客户端参数 + 独立数据目录）
3. 每轮切换不同网络出口路由节点
4. 使用自然人机交互进行表单填写
5. 5 轮重复执行以验证流程可靠性
6. 每轮自适应调整参数

---

## 二、现有基础设施

### 2.1 路由池
- **总计**: 83 个网络出口节点
- **协议**: vmess/vless/trojan/ss/hysteria2/tuic/anytls
- **桥接**: Xray + Sing-Box
- IP 健康数据已缓存（国家、欺诈评分、住宅IP标识等）

### 2.2 人机交互引擎
- `BuildTypingPlan()` — 含拼写错误、退格修正、停顿
- `ComputeClickTargetForElement()` — 安全点击区域 + 贝塞尔轨迹
- `BuildScrollPlan()` — 过冲-返回滚动模式
- `HumanizedRetryDecision()` — 指数退避 + 随机放弃

### 2.3 浏览器生命周期

| 操作 | 说明 |
|------|------|
| 创建 | 生成UUID、绑定路由、生成人机交互种子 |
| 启动 | 解析路由→启动桥接→启动Chromium→等待CDP端口 |
| 停止 | CDP Browser.close → taskkill 回退 |
| 删除 | 数据库删除 + 清理用户数据目录 |

---

## 三、路由节点选择策略

按出口地区分流：**大陆IP → 手机注册路径**，**海外IP → 邮箱注册路径**。

| 优先级 | 类型 | 条件 |
|--------|------|------|
| Tier 1 | 台湾住宅 | fraudScore<15, isResidential=true |
| Tier 2 | 美日韩低延迟 | fraudScore<30, latency<1000ms |
| Tier 3 | 欧洲备选 | fraudScore<40, latency<2000ms |
| Tier 4 | 任意非大陆 | fraudScore<50 |

---

## 四、单轮执行流程

```
┌──────────────────────────────────────────────────────────────┐
│ ROUND N (1..5)                                                │
│  1. 路由节点筛选 → 速度测试 → 健康检查                         │
│  2. Profile 创建 → 绑定路由节点                                │
│  3. 启动浏览器 → Xray/Sing-Box 桥接 → Chromium 启动            │
│  4. 服务开通流程:                                              │
│     a. 创建临时邮箱                                            │
│     b. CDP 连接                                                │
│     c. 导航到注册页面                                           │
│     d. WAF 检测                                                │
│     e. 人机化输入: 邮箱 → 密码                                  │
│     f. 人机化点击: 提交按钮                                     │
│     g. 人机化滚动: 页面浏览                                    │
│     h. Turnstile 处理                                          │
│     i. 调用邮箱验证 API                                         │
│     j. 轮询邮箱 (最长 3min)                                     │
│     k. 输入验证码                                               │
│     l. 完成注册                                                │
│     m. 导航到 API Keys 页面                                    │
│     n. 提取 API Key                                            │
│     o. 记录结果                                                │
│  5. 清理: 停止浏览器 → 删除 Profile → 清理数据目录               │
│  6. 自适应调整参数                                               │
│  7. 轮间延迟 30-120s                                            │
└──────────────────────────────────────────────────────────────┘
```

---

## 五、DeepSeek API 端点

| 步骤 | 方法 | URL |
|------|------|-----|
| 发验证码 | POST | `/auth-api/v0/users/create_email_verification_code` |
| 校验验证码 | POST | `/auth-api/v0/users/check_email_code` |
| 完成注册 | POST | `/auth-api/v0/users/register` |

**常量**: 平台URL `https://platform.deepseek.com`, 注册页 `/signup`, Turnstile sitekey `0x4AAAAAAA1jPG9yoQG1HRmA`

---

## 六、失败处理

| 类型 | 检测 | 处理 |
|------|------|------|
| 路由不可达 | 速度测试失败 | 换节点，黑名单 |
| 出口被拦截 | WAF 检测 | 换节点，仅住宅IP |
| 人机验证超时 | 90s token 为空 | 延长至120s |
| 验证码超时 | 3min 未收到 | 重发验证码 |
| 邮箱创建失败 | API error | 等10s重试，换域名 |

---

## 七、人机交互要点

- [ ] 非恒定打字速度: 每字符间隔随机变化 ±60%
- [ ] 拼写错误 + 修正: 5% 概率错字 → 退格 → 重打
- [ ] 中途思考停顿: 12% 概率停顿 1-2 秒
- [ ] 贝塞尔鼠标轨迹: 3-5 个控制点
- [ ] 点击偏移: 中心 ±12px 随机
- [ ] 动作间延迟: 右偏分布
- [ ] 滚动过冲: 滚过目标再回滚
- [ ] 轮间延迟: 30-120s 随机

---

## 八、文件结构

### 新建文件
| 文件 | 行数 | 用途 |
|------|------|------|
| `backend/cmd/deepseek-register/main.go` | ~600 | 批量执行入口 |
| `backend/internal/behavior/cdp_executor.go` | ~250 | CDP 动作分发器 |
| `backend/internal/email/names.go` | ~80 | 邮箱地址生成器 |

### 修改文件
| 文件 | 改动 |
|------|------|
| `backend/app_deepseek_register.go` | ~400行重写 |
| `backend/internal/email/mailtm.go` | ~30行修改 |
