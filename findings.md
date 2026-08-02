# Findings

> **归档说明（2026-06-29）**：本文件为 **agent.alexstudio.top 外部 Gateway** 调研笔记，与 Personal Pilot 主线无关。Personal Pilot 当前 findings 见 `docs/02-current-state.md`、`docs/04-improvement-backlog.md`、**`PLAN.md`**（49–56 执行轨道）。

---

# Findings（历史：外部 Gateway）

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

（后续条目未再同步，见 git 历史。）
