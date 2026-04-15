# Findings

## Current task
- 用户当前新主线已从 `PersonaPilot` 的 browser/status 展示收口，临时切到一个更外部的工程问题：
  - 使用 `agent.alexstudio.top` 作为 API 中转站入口
  - 目标是把“从 Codex/Web 会话中反代出来的额度链路”包装成**本人测试可用、风险可控**的 API 网关
- 用户明确要求：先做**自己测试可用**的版本，不是先做公开平台。

## Cloudflare assets confirmed
- 已用 Cloudflare Global API Key 查到当前账号下有两个 zone：
  - `alexstudio.top`
  - `chihuolingrang.de5.net`
- 已为 `alexstudio.top` 创建 5 个 AI 相关子域名，并全部为橙云 CNAME 指向主域：
  - `agent.alexstudio.top`
  - `model.alexstudio.top`
  - `chat.alexstudio.top`
  - `vector.alexstudio.top`
  - `lab.alexstudio.top`
- 其中当前最相关的入口是：`agent.alexstudio.top`

## Product judgment
- 这件事技术上可做，但风险不在“域名能不能指过来”，而在：
  1. 上游额度来源是否稳定/长期可承受
  2. 网关是否会把上游凭据暴露给客户端
  3. 网关是否具备本人测试阶段所需的最小鉴权、限流、撤销、日志控制
- 当前最合理定位应是：
  **受控私用测试网关**，而不是开放 API 平台。

## Minimum safe architecture judgment
- 当前最推荐入口链：
  - `client`
  - → `agent.alexstudio.top` (Cloudflare)
  - → `private gateway service`
  - → `upstream quota path`
- Cloudflare 适合作为：
  - TLS 终止
  - 入口代理
  - 基础规则/WAF/速率保护
- 真正的安全边界仍应在自建 gateway：
  - 签发本地下游 token
  - 校验 token
  - 限流
  - 记录最小 usage
  - 清洗 header
  - 隔离上游凭据

## Must-have controls for test-only gateway
- 下游 token 由本地签发，但这本身不代表整体安全；还必须同时具备：
  - token 可撤销
  - token 有 client 标识
  - token 有权限范围
  - token 有速率限制
  - token 有使用量统计
- gateway 不应把以下内容原样下发或记录：
  - 上游 cookies/session
  - 上游真实鉴权头
  - 完整请求体/响应体日志
  - 未清洗的错误堆栈与调试日志

## Anti-patterns to avoid
- 不要把 `agent.alexstudio.top` 裸开放给任意客户端。
- 不要把上游凭据直接发给客户端，让客户端直连上游。
- 不要一开始就做多租户、公开售卖或弱鉴权共享入口。
- 不要默认记录完整 prompt、响应正文和 Authorization 头。

## Recommended first version scope
- v0 只做“本人测试可用”：
  - 1~2 个本地下游 token
  - 固定白名单客户端
  - 严格限流
  - 最小请求/错误日志
  - 单入口 `agent.alexstudio.top`
  - 单一路由协议（优先兼容 OpenAI 风格接口）
- 暂不推荐：
  - 多人共享
  - 公网开放注册
  - 全量日志
  - 复杂配额计费

## Immediate next best move
- 当前最值动作不是直接开写网关代码，而是先把：
  1. 鉴权模型
  2. 限流模型
  3. 日志边界
  4. header 清洗规则
  5. 上游/下游责任边界
  这五件事设计清楚。
