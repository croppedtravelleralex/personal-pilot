# agent.alexstudio.top v0 private gateway design

## Goal
做一版**本人测试可用**、**风险可控**的 API 中转站，入口固定为 `agent.alexstudio.top`。

## Positioning
- 这是**受控私用网关**，不是公开 API 平台。
- 只服务你自己的 agent / 脚本 / 本地工具。
- 上游额度链必须被 gateway 隔离，不能直接下放给客户端。

## Minimal request path
`client -> agent.alexstudio.top -> Cloudflare -> private gateway -> upstream quota path`

## v0 hard requirements
1. 强制鉴权
2. 基础限流
3. 来源标识
4. header 清洗
5. 最小日志
6. 不暴露上游凭据
7. 失败分类

## Auth model
### Downstream token
- 使用你本地签发的下游 token。
- 只接受你自己的 token，不开放匿名访问。
- token 最少要支持：
  - 唯一 id / label
  - enabled 开关
  - revoke（撤销）

### Accepted auth header
- `Authorization: Bearer <token>`
- 可兼容 `x-api-key`，但 Bearer 为主。

## Rate limiting
### v0 simplest safe version
- 每个 token 单独限流。
- 先做一个轻量固定窗口：
  - 每分钟请求数
  - 并发数上限
- 命中后返回 `429`。

## Logging policy
### Keep
- request id
- token label / client label
- path
- model
- upstream target label
- status code
- latency
- timestamp

### Do not keep
- 上游 cookie / session
- 上游真实 auth header
- 完整 Authorization
- 完整 prompt
- 完整 response body

## Header sanitization
### Strip before upstream
- downstream Authorization
- x-api-key
- cf-* headers not needed upstream
- x-forwarded-* except those explicitly retained

### Inject toward upstream
- only upstream-required auth
- internal request id
- internal source label if needed

## Endpoint compatibility
### v0 recommendation
先兼容 OpenAI-style minimal surface：
- `POST /v1/chat/completions`
- `GET /v1/models`
- optional later: `POST /v1/responses`

### Response principle
- 尽量向 OpenAI 风格靠拢
- 但错误要分类清楚：
  - auth_failed
  - rate_limited
  - upstream_unavailable
  - upstream_timeout
  - upstream_rejected

## Suggested env vars
- `GATEWAY_BIND=127.0.0.1:8787`
- `GATEWAY_PUBLIC_BASE_URL=https://agent.alexstudio.top`
- `GATEWAY_ADMIN_TOKEN=<local-admin-token>`
- `GATEWAY_DOWNSTREAM_TOKENS_JSON=<token config json>`
- `GATEWAY_RATE_LIMIT_PER_MINUTE=30`
- `GATEWAY_CONCURRENCY_PER_TOKEN=3`
- `UPSTREAM_BASE_URL=<hidden upstream>`
- `UPSTREAM_AUTH_MODE=<cookie|bearer|custom>`
- `UPSTREAM_AUTH_SECRET=<hidden>`

## v0 storage recommendation
- 先不做数据库依赖。
- v0 可以先用：
  - env + local json file
- 只要能支持：
  - token list
  - revoke
  - usage append-only log

## Immediate implementation advice
1. 在当前仓库旁边或内部单独开一个轻量 gateway 模块/目录。
2. 先跑本地监听，不急着公网暴露。
3. 本地 curl 测通后，再把 `agent.alexstudio.top` 指到 gateway。
4. Cloudflare 只做入口代理与 TLS，不承担业务鉴权。

## What not to do
- 不要把上游 auth 直接返回给客户端。
- 不要做透明裸转发。
- 不要默认保存原始 prompt/response。
- 不要一开始就支持多人共享 token。
- 不要一开始就做复杂账单系统。

## Recommended v0 milestone
### Milestone A
- 本地跑起来
- Bearer token 校验
- `/v1/models`
- `/v1/chat/completions` skeleton
- 限流
- 请求日志

### Milestone B
- 接上真实上游额度链
- 错误分类
- 熔断/超时
- `agent.alexstudio.top` 接入口

### Milestone C
- 你自己的 agent/client 实测
- 再决定要不要扩接口面
