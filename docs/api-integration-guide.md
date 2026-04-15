# API 接入文档（Integration Guide）

这份文档面向 **外部接入方 / 二次开发方**，目标是让人拿到服务地址后，能快速完成：

- 鉴权
- 健康检查
- 创建任务
- 查询任务状态 / runs / logs
- 调用 Browser 5 个接口
- 管理指纹配置与代理池
- 理解关键返回字段

> 说明：仓库默认的 `data/proxy_sources.json` / `data/proxy_candidates_*` 目前是 **demo seed smoke 配置**，只用于 parser/dedupe/verify 结构烟雾，不保证产生 live active proxy。若需要真实 active proxy，请改用真实 source 配置（可参考 `docs/proxy_sources.real.template.json`）。

---

## 1. 基础信息

### Base URL

默认控制面地址：

```bash
BASE_URL=http://127.0.0.1:3000
```

### Content-Type

所有写接口统一使用：

```http
Content-Type: application/json
```

### 鉴权

如果服务端配置了 API Key，则所有接口都需要带上以下任一请求头：

**方式 1：**

```http
x-api-key: <YOUR_API_KEY>
```

**方式 2：**

```http
Authorization: Bearer <YOUR_API_KEY>
```

如果服务端没有配置 API Key，则可直接访问。

建议客户端统一封装：

```bash
API_KEY="your-api-key"
AUTH_HEADER="x-api-key: $API_KEY"
```

---

## 2. 快速接入流程

推荐接入顺序：

1. 调 `/health` 看服务是否活着
2. 调 `/status` 看系统状态、队列、最近任务
3. 调 Browser API 或 `/tasks` 创建任务
4. 轮询 `/tasks/:id`
5. 必要时查看 `/tasks/:id/runs` 和 `/tasks/:id/logs`
6. 若任务异常，按需 `/retry` 或 `/cancel`

---

## 3. 健康检查与系统状态

### 3.1 GET /health

用于最基础的健康检查。

```bash
curl -s "$BASE_URL/health"
```

典型返回：

```json
{
  "status": "ok",
  "service": "persona_pilot",
  "queue_len": 0,
  "counts": {
    "total": 12,
    "queued": 0,
    "running": 0,
    "succeeded": 10,
    "failed": 1,
    "timed_out": 0,
    "cancelled": 1
  }
}
```

### 3.2 GET /status

用于查看更完整的运行态快照。

```bash
curl -s "$BASE_URL/status"
```

返回内容通常包括：

- 队列长度
- 任务状态计数
- worker 状态
- 指纹指标
- 代理指标
- verify 指标
- 最近任务
- 最近 browser 任务
- 最近执行摘要

适合用于：

- 面板展示
- 运维巡检
- 自动监控接入

---

## 4. 任务 API

任务 API 是最通用的入口。你可以直接创建标准任务，也可以用更语义化的 Browser API。

### 4.1 POST /tasks

创建一个任务。

请求体：

```json
{
  "kind": "open_page",
  "url": "https://example.com",
  "timeout_seconds": 30,
  "priority": 0,
  "fingerprint_profile_id": "fp-desktop-chrome",
  "proxy_id": "proxy-us-1",
  "network_policy_json": {
    "mode": "required_proxy"
  }
}
```

字段说明：

- `kind`：任务类型
- `url`：目标地址
- `timeout_seconds`：超时秒数
- `priority`：优先级
- `fingerprint_profile_id`：指定指纹配置
- `proxy_id`：指定代理
- `network_policy_json`：网络策略

常见 `kind`：

- `open_page`
- `get_html`
- `get_title`
- `get_final_url`
- `extract_text`

创建示例：

```bash
curl -s -X POST "$BASE_URL/tasks" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "kind": "open_page",
    "url": "https://example.com",
    "timeout_seconds": 20,
    "priority": 0,
    "network_policy_json": {
      "mode": "required_proxy"
    }
  }'
```

典型返回：

```json
{
  "id": "task_123",
  "kind": "open_page",
  "status": "queued",
  "priority": 0,
  "started_at": null,
  "finished_at": null,
  "summary_artifacts": [],
  "fingerprint_profile_id": null,
  "fingerprint_profile_version": null,
  "fingerprint_resolution_status": null,
  "proxy_id": null,
  "proxy_provider": null,
  "proxy_region": null,
  "proxy_resolution_status": "pending",
  "trust_score_total": null,
  "selection_reason_summary": null,
  "selection_explain": null,
  "fingerprint_runtime_explain": null,
  "execution_identity": null,
  "identity_network_explain": null,
  "winner_vs_runner_up_diff": null,
  "failure_scope": null,
  "browser_failure_signal": null,
  "title": null,
  "final_url": null,
  "content_preview": null,
  "content_length": null,
  "content_truncated": null,
  "content_kind": null,
  "content_source_action": null,
  "content_ready": null
}
```

### 4.2 GET /tasks/:id

查询单个任务详情。

```bash
curl -s "$BASE_URL/tasks/$TASK_ID" -H "$AUTH_HEADER"
```

这个接口最重要，接入方通常靠它判断：

- 当前任务状态
- 是否成功 / 失败 / 超时 / 取消
- 标题 / 最终 URL / 内容摘要
- 使用了哪个代理 / 指纹
- 为什么这么选（explainability）

### 4.3 GET /tasks/:id/runs

查看任务的执行尝试记录。

```bash
curl -s "$BASE_URL/tasks/$TASK_ID/runs" -H "$AUTH_HEADER"
```

适合用于：

- 查重试历史
- 分析失败重跑情况
- 看每次 attempt 的执行差异

### 4.4 GET /tasks/:id/logs

查看任务日志。

```bash
curl -s "$BASE_URL/tasks/$TASK_ID/logs" -H "$AUTH_HEADER"
```

适合用于：

- 调试
- 错误排查
- 外部平台展示原始执行日志

### 4.5 POST /tasks/:id/retry

重试任务。

```bash
curl -s -X POST "$BASE_URL/tasks/$TASK_ID/retry" -H "$AUTH_HEADER"
```

### 4.6 POST /tasks/:id/cancel

取消任务。

```bash
curl -s -X POST "$BASE_URL/tasks/$TASK_ID/cancel" -H "$AUTH_HEADER"
```

---

## 5. Browser API

如果你的调用方只关心“打开网页 / 拿 HTML / 拿标题 / 拿最终 URL / 抽正文”，推荐直接用 Browser API，而不是手写 `kind`。

### 共同请求字段

Browser 5 个接口都支持：

```json
{
  "url": "https://example.com",
  "timeout_seconds": 15,
  "priority": 0,
  "fingerprint_profile_id": "fp-desktop-chrome",
  "proxy_id": "proxy-us-1",
  "network_policy_json": {
    "mode": "required_proxy"
  }
}
```

注意：

- Browser 任务会强制走代理策略
- 如果显式传 `mode=direct`，服务端会拒绝

### 5.1 POST /browser/open

打开页面。

```bash
curl -s -X POST "$BASE_URL/browser/open" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "url": "https://example.com",
    "timeout_seconds": 15,
    "network_policy_json": {
      "mode": "required_proxy"
    }
  }'
```

### 5.2 POST /browser/html

获取页面 HTML。

```bash
curl -s -X POST "$BASE_URL/browser/html" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "url": "https://example.com/page",
    "timeout_seconds": 15,
    "network_policy_json": {
      "mode": "required_proxy"
    }
  }'
```

### 5.3 POST /browser/title

获取标题。

```bash
curl -s -X POST "$BASE_URL/browser/title" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "url": "https://example.com/page",
    "timeout_seconds": 10,
    "network_policy_json": {
      "mode": "required_proxy"
    }
  }'
```

### 5.4 POST /browser/final-url

获取跳转后的最终 URL。

```bash
curl -s -X POST "$BASE_URL/browser/final-url" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "url": "https://example.com/redirect",
    "timeout_seconds": 10,
    "network_policy_json": {
      "mode": "required_proxy"
    }
  }'
```

### 5.5 POST /browser/text

提取正文文本。

```bash
curl -s -X POST "$BASE_URL/browser/text" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "url": "https://example.com/article",
    "timeout_seconds": 15,
    "network_policy_json": {
      "mode": "required_proxy"
    }
  }'
```

### Browser 返回怎么读

Browser API 本质上也是创建任务，所以返回优先看这些字段：

- `id`
- `status`
- `title`
- `final_url`
- `content_preview`
- `content_length`
- `content_kind`
- `content_ready`
- `failure_scope`
- `browser_failure_signal`

如果你要最终内容，通常需要继续查询：

```bash
curl -s "$BASE_URL/tasks/$TASK_ID" -H "$AUTH_HEADER"
```

---

## 6. 指纹配置 API

### 6.1 POST /fingerprint-profiles

创建指纹配置。

```bash
curl -s -X POST "$BASE_URL/fingerprint-profiles" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "id": "fp-desktop-chrome",
    "name": "Desktop Chrome",
    "profile_json": {
      "browser": {"name": "chrome", "version": "123"},
      "os": {"name": "macos", "version": "14.4"},
      "headers": {"accept_language": "en-US,en;q=0.9"}
    }
  }'
```

### 6.2 GET /fingerprint-profiles

列出指纹配置。

```bash
curl -s "$BASE_URL/fingerprint-profiles" -H "$AUTH_HEADER"
```

### 6.3 GET /fingerprint-profiles/:id

查询单个指纹配置。

```bash
curl -s "$BASE_URL/fingerprint-profiles/fp-desktop-chrome" -H "$AUTH_HEADER"
```

典型返回包含：

- `id`
- `name`
- `version`
- `status`
- `validation_ok`
- `validation_issues`
- `profile_json`

适合用于接入方做：

- profile 管理页
- profile 可用性校验
- profile 版本绑定

---

## 7. 代理池 API

### 7.1 POST /proxies

创建代理。

```bash
curl -s -X POST "$BASE_URL/proxies" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "id": "proxy-us-1",
    "scheme": "http",
    "host": "1.2.3.4",
    "port": 8080,
    "region": "us-east",
    "country": "US",
    "provider": "seed-provider",
    "status": "candidate",
    "score": 0.8
  }'
```

### 7.2 GET /proxies

列出代理。

```bash
curl -s "$BASE_URL/proxies" -H "$AUTH_HEADER"
```

### 7.3 GET /proxies/:id

查询单个代理。

```bash
curl -s "$BASE_URL/proxies/proxy-us-1" -H "$AUTH_HEADER"
```

典型返回会包含：

- 基础身份：`id / host / port / provider / region / country`
- 状态：`status / score / cooldown_until`
- smoke 信息：`last_smoke_status / last_smoke_upstream_ok`
- verify 信息：
  - `last_verify_status`
  - `last_verify_geo_match_ok`
  - `last_verify_confidence`
  - `last_verify_score_delta`
  - `last_probe_error_category`

### 7.4 POST /proxies/:id/smoke

快速探活。

```bash
curl -s -X POST "$BASE_URL/proxies/proxy-us-1/smoke" -H "$AUTH_HEADER"
```

### 7.5 POST /proxies/:id/verify

深度验证代理可用性。

```bash
curl -s -X POST "$BASE_URL/proxies/proxy-us-1/verify" -H "$AUTH_HEADER"
```

返回重点字段：

- `reachable`
- `protocol_ok`
- `upstream_ok`
- `exit_ip`
- `exit_country`
- `exit_region`
- `geo_match_ok`
- `region_match_ok`
- `identity_fields_complete`
- `risk_level`
- `risk_reasons`
- `failure_stage`
- `verification_confidence`
- `verification_class`
- `recommended_action`
- `status`
- `message`

### 7.6 POST /proxies/verify-batch

批量创建 verify 任务。

```bash
curl -s -X POST "$BASE_URL/proxies/verify-batch" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d '{
    "provider": "seed-provider",
    "region": "us-east",
    "limit": 20,
    "only_stale": true,
    "stale_after_seconds": 3600,
    "task_timeout_seconds": 15
  }'
```

### 7.7 GET /proxies/verify-batch

查看 verify batch 列表。

```bash
curl -s "$BASE_URL/proxies/verify-batch" -H "$AUTH_HEADER"
```

### 7.8 GET /proxies/verify-batch/:id

查看单个 verify batch。

```bash
curl -s "$BASE_URL/proxies/verify-batch/$BATCH_ID" -H "$AUTH_HEADER"
```

---

## 8. 代理 explainability / trust cache API

### 8.1 GET /proxies/:id/explain

解释为什么某个代理被选中。

```bash
curl -s "$BASE_URL/proxies/proxy-us-1/explain" -H "$AUTH_HEADER"
```

返回重点：

- `selection_reason_summary`
- `trust_score_total`
- `trust_score_components`
- `candidate_rank_preview`
- `winner_vs_runner_up_diff`
- `provider_risk_version_status`

适合接入方做：

- 管理后台 explain 卡片
- 调度审计
- 故障解释页

### 8.2 GET /proxies/:id/trust-cache-check

检查 trust cache 是否漂移。

```bash
curl -s "$BASE_URL/proxies/proxy-us-1/trust-cache-check" -H "$AUTH_HEADER"
```

### 8.3 POST /proxies/:id/trust-cache-repair

修复单个代理 trust cache。

```bash
curl -s -X POST "$BASE_URL/proxies/proxy-us-1/trust-cache-repair" -H "$AUTH_HEADER"
```

### 8.4 GET /proxies/trust-cache-scan

批量扫描 trust cache。

```bash
curl -s "$BASE_URL/proxies/trust-cache-scan?limit=50&only_drifted=true" -H "$AUTH_HEADER"
```

### 8.5 POST /proxies/trust-cache-repair-batch

批量修复 trust cache。

```bash
curl -s -X POST "$BASE_URL/proxies/trust-cache-repair-batch" -H "$AUTH_HEADER"
```

### 8.6 POST /proxies/trust-cache-maintenance

执行 trust cache 维护。

```bash
curl -s -X POST "$BASE_URL/proxies/trust-cache-maintenance" -H "$AUTH_HEADER"
```

---

## 9. 常见任务状态

任务状态通常包括：

- `queued`
- `running`
- `succeeded`
- `failed`
- `timed_out`
- `cancelled`

接入方建议采用下面的终态判断：

- **终态**：`succeeded / failed / timed_out / cancelled`
- **非终态**：`queued / running`

轮询示例：

```bash
while true; do
  RES=$(curl -s "$BASE_URL/tasks/$TASK_ID" -H "$AUTH_HEADER")
  STATUS=$(echo "$RES" | python3 -c 'import sys, json; print(json.load(sys.stdin)["status"])')
  echo "$STATUS"
  if [ "$STATUS" = "succeeded" ] || [ "$STATUS" = "failed" ] || [ "$STATUS" = "timed_out" ] || [ "$STATUS" = "cancelled" ]; then
    echo "$RES"
    break
  fi
  sleep 1
done
```

---

## 10. 关键返回字段怎么理解

### 通用字段

- `id`：任务 / 资源 ID
- `status`：当前状态
- `priority`：优先级
- `started_at / finished_at`：执行时间

### Browser 内容字段

- `title`：页面标题
- `final_url`：跳转后的最终地址
- `content_preview`：内容预览
- `content_length`：内容长度
- `content_kind`：内容类型，如 `text/html` / `text/plain`
- `content_ready`：内容是否已准备好

### 指纹 / 代理身份字段

- `fingerprint_profile_id`
- `fingerprint_profile_version`
- `fingerprint_resolution_status`
- `proxy_id`
- `proxy_provider`
- `proxy_region`
- `proxy_resolution_status`

### Explainability 字段

- `selection_reason_summary`：一句话说明为什么选这个代理
- `selection_explain`：更详细的代理选择解释
- `fingerprint_runtime_explain`：运行时指纹解释
- `execution_identity`：执行时采用的身份视图
- `identity_network_explain`：身份与网络选择的综合解释
- `trust_score_total`：代理信任总分
- `winner_vs_runner_up_diff`：当前选中代理与次优候选的差异
- `summary_artifacts`：关键摘要证据

### 错误 / 失败分析字段

- `failure_scope`：失败归因范围
- `browser_failure_signal`：浏览器侧失败信号
- `error_message`：runs 级别的错误信息

---

## 11. 推荐给接入方的最小接入方式

如果你只是想把它接进自己的系统，推荐最小方案：

### 方案 A：Browser 调用型

适合：
- 内容抓取
- 页面打开
- 标题 / 最终 URL / 正文提取

接法：
1. 调 `/browser/*`
2. 拿到 `task id`
3. 轮询 `/tasks/:id`
4. 读取 `title/final_url/content_preview/content_kind`

### 方案 B：任务调度型

适合：
- 自己已有调度系统
- 想统一重试 / 取消 / runs / logs

接法：
1. 调 `/tasks`
2. 保存 `task id`
3. 查 `/tasks/:id`
4. 出问题查 `/tasks/:id/runs` 和 `/tasks/:id/logs`
5. 必要时 `/retry` 或 `/cancel`

### 方案 C：代理治理型

适合：
- 你自己要维护代理池
- 想接 verify / explain / trust cache

接法：
1. `/proxies` 入池
2. `/proxies/:id/smoke`
3. `/proxies/:id/verify`
4. `/proxies/:id/explain`
5. `/proxies/trust-cache-*`
6. `/proxies/verify-batch*`

---

## 12. 常见错误与接入建议

### 401 Unauthorized

原因：
- 没带 API Key
- API Key 不正确

解决：
- 补 `x-api-key`
- 或补 `Authorization: Bearer ...`

### 400 Bad Request

原因：
- JSON 字段缺失
- 字段类型不对
- Browser 任务显式传了 `mode=direct`

解决：
- 检查请求体
- Browser 任务统一走 `required_proxy`

### 404 Not Found

原因：
- 资源不存在
- task id / proxy id / fingerprint id 写错

### 任务一直 queued / running

排查顺序：
1. 先看 `/status`
2. 再看 `/tasks/:id`
3. 再看 `/tasks/:id/runs`
4. 最后看 `/tasks/:id/logs`

### Browser 内容为空

排查字段：
- `content_ready`
- `failure_scope`
- `browser_failure_signal`
- `summary_artifacts`

---

## 13. 文档索引

建议同时看：

- `docs/api-ops.md`：偏运维 / 验收视角
- `docs/browser-api-v1-examples.md`：偏 Browser API 示例
- `docs/control-plane-and-visibility-mainline.md`
- `docs/selection-explainability.md`
- `docs/proxy-verification-v2.md`

---

## 14. 给接入方的最终建议

如果你是第一次接：

1. **先接 `/health` + `/status`**
2. **再接 `/browser/text` 或 `/browser/html`**
3. **拿到 task id 后统一走 `/tasks/:id` 轮询**
4. **如果业务要稳定上量，再接代理 explain / verify / trust cache**

这样接，路径最短，返工最少。
