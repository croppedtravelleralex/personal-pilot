# Stage Summary — 2026-03-31

## 本阶段结论
当前 `PersonaPilot` 已经完成了 **浏览器执行系统 V1 控制面 + 代理验证 V1 验证面** 的一轮系统性收口，已经明显脱离“散装原型”阶段，进入 **可持续扩展的工程骨架**。

## 本阶段已完成的核心收口

### 1. 调度与执行控制面
- DB-first queue
- 原子 claim
- heartbeat / reclaim / retry / cancel
- idle backoff / jitter / error cap
- reclaim 相关 flaky 测试去除后台 worker 干扰

### 2. Fingerprint profile
- profile 创建 / 查询
- task 引用 profile id + version
- active + version 匹配解析
- task detail / status 暴露 resolution 状态

### 3. Proxy pool / selection
- proxy CRUD 基础能力
- provider / region / min_score / cooldown 过滤
- score + last_used_at 基础排序
- sticky session 正式绑定表 `proxy_session_bindings`

### 4. Proxy verification V1
- smoke: TCP connect
- smoke: HTTP CONNECT response validation
- upstream ip echo signal
- anonymity classification: `transparent` / `anonymous` / `elite`
- smoke verification signal persistence:
  - `last_smoke_status`
  - `last_smoke_protocol_ok`
  - `last_smoke_upstream_ok`
  - `last_exit_ip`
  - `last_anonymity_level`
  - `last_smoke_at`

### 5. Observability
- `/status` counts 改为单条聚合 SQL
- fingerprint metrics summary
- proxy metrics summary
- worker backoff 参数暴露
- task / status 暴露 proxy identity 与 resolution 状态

## 当前阶段性能判断
- 当前轻量性能评分：**8.5 / 10**
- 最新集成测试规模：**33 tests**
- 当前主要瓶颈不在 runner 本身，而在：
  1. proxy 选择仍为轻量查库排序
  2. 真实外部匿名性/地区校验链尚未落地
  3. metrics 仍偏应用层派生
  4. reclaim 后半段仍可继续一体化

## 本阶段关键风险与教训
- reclaim 测试出现过一次 flaky，根因更像是 **测试环境内后台 worker 并发干扰**，而不是 reclaim 主逻辑本身损坏。
- 后续涉及状态机 / reclaim / queue 的测试，应优先区分：
  - 需要真实后台 worker 的集成场景
  - 只需要纯状态转移验证的无 worker 场景

## 下一阶段推荐主线
1. 更真实的匿名性 / 地区校验链
2. 更正式的 API / 运维文档
3. proxy 选择索引与策略正式化
4. `verify` 慢路径设计落地

## 代表性提交
- `271173e` — protocol check
- `a49866d` — reclaim/backoff 收口
- `f405cd7` — upstream ip signal
- `82b7a79` — anonymity classification
- `1e783f7` — persist smoke verification signals
- `c5d6b2f` — runtime tuning / verification docs
- `6bc9d75` — capabilities summary
- `9d03ddf` — external proxy verification flow doc
- `397a45a` — remove worker race from reclaim test
