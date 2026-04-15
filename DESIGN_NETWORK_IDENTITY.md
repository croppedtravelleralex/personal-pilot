# DESIGN_NETWORK_IDENTITY.md

`PersonaPilot` 的网络与身份层设计文档。

聚焦三部分：

1. 指纹模型（Fingerprint）
2. 代理池模型（Proxy Pool）
3. 任务网络策略（Task Network Policy）

---

## 1. 设计目标

本设计文档用于约束后续实现，确保系统在以下目标下演进：

- 高并发场景下仍保持较小的额外性能开销
- 高级指纹能力是正式模块，而非零散参数
- 所有访问强制经过代理池
- 代理池具备验证、轮换、自生长、地区感知能力
- fake runner 与 real runner 共用统一抽象
- 任务在创建时即可绑定网络与身份策略

---

## 2. 顶层设计原则

### 2.1 强制代理原则
所有外部访问都必须通过代理池分配的代理执行。

包括但不限于：
- 页面访问
- 资源请求
- 健康验证请求
- 代理验证请求
- 预热/探测请求

系统不应保留默认直连路径。

### 2.2 指纹正式建模原则
指纹不是“附加参数字典”，而是独立建模对象。

### 2.3 地区优先原则
代理分配必须支持地区感知：
- 目标地区
- 代理地区
- 匹配策略

### 2.4 低开销优先原则
高级指纹能力必须带性能预算，不允许默认引入不可控开销。

### 2.5 验证优先原则
代理可用性要靠真实验证结果确认，而不是只靠连通性判断。

---

## 3. 指纹模型设计

### 3.1 FingerprintProfile

建议作为静态配置对象：

```text
FingerprintProfile
- id
- name
- description
- browser_family
- user_agent
- locale
- timezone
- viewport_width
- viewport_height
- screen_width
- screen_height
- platform
- color_scheme
- hardware_concurrency
- device_memory
- webgl_profile
- canvas_profile
- audio_profile
- fonts_profile
- client_hints_profile
- anti_detection_flags
- perf_budget_tag
- enabled
```

### 3.2 FingerprintStrategy

用于决定“如何使用 profile”：

```text
FingerprintStrategy
- id
- name
- mode                # fixed | rotate | adaptive
- candidate_profiles  # 可选 profile 列表
- selection_rule      # 选择规则
- reuse_policy        # 重用策略
- validation_rule     # 指纹验证策略
```

### 3.3 性能预算

建议给指纹能力打标签：

- `light`
- `medium`
- `heavy`

约束：
- 高并发优先使用 `light` / `medium`
- `heavy` 仅用于特定任务或实验场景
- 每种 profile 应可评估其性能开销

### 3.4 指纹实现分层

建议分为三层：

#### Layer 1: 轻量基础层
- UA
- locale
- timezone
- viewport
- platform

#### Layer 2: 中等层
- client hints
- hardware_concurrency
- device_memory
- color scheme

#### Layer 3: 高级层
- canvas
- webgl
- audio
- fonts
- webdriver 痕迹处理

这样可以在高并发时按层降级，而不是整体关闭。

---

## 3.5 持续抓取代理工具（Proxy Harvester）

代理池不能只依赖手工导入，系统应具备一个可持续运行的代理抓取工具。

### 目标
- 周期性从多个来源抓取代理
- 优先基于开源项目改造实现
- 抓取后完成清洗、去重、标准化
- 将结果写入候选池
- 触发后续验证流程
- 为代理池自生长提供持续供给

### 最小能力
- source adapter
- parser / normalizer
- dedupe
- candidate ingest
- harvest run log

### 与代理池关系
- 它是代理池的上游供给模块
- 不等于代理池本身
- 不直接等于可用代理
- 抓取结果必须经过验证后才能进入正式可用池

## 4. 代理池模型设计

### 4.1 ProxyEndpoint

```text
ProxyEndpoint
- id
- provider
- protocol           # http | https | socks5
- host
- port
- username
- password_ref
- region_country
- region_area
- region_city
- isp
- tags
- status             # unknown | available | degraded | unavailable | banned
- source_type        # static | imported | discovered | replenished
- success_rate
- fail_count
- consecutive_failures
- last_check_at
- last_success_at
- last_failure_at
- cooldown_until
- enabled
```

### 4.2 ProxyPoolPolicy

```text
ProxyPoolPolicy
- min_available_ratio
- max_available_ratio
- min_available_total
- min_available_per_region
- concurrency_scaling_rule
- replenish_trigger_rule
- failure_evict_rule
- cooldown_rule
- validation_rule
```

### 4.3 ProxyValidationResult

```text
ProxyValidationResult
- proxy_id
- validator
- runner_type
- target_kind
- target_region
- started_at
- finished_at
- success
- error_type
- latency_ms
- notes
```

### 4.4 ProxyAllocation

```text
ProxyAllocation
- allocation_id
- task_id
- run_id
- proxy_id
- selected_at
- region_match_score
- selection_reason
- released_at
- outcome
```

---

## 5. 任务网络策略设计

### 5.1 TaskNetworkPolicy

每个任务应能绑定网络策略：

```text
TaskNetworkPolicy
- require_proxy              # 固定 true
- proxy_strategy             # fixed | rotate | adaptive | region-preferred
- target_region_country
- target_region_area
- target_region_city
- proxy_region_match_mode    # strict | preferred | fallback
- fingerprint_profile_id
- fingerprint_strategy_id
- network_timeout_ms
- max_proxy_retries
- allow_region_fallback
```

### 5.2 规则说明

#### require_proxy
必须固定为 `true`。

#### proxy_region_match_mode
- `strict`：必须匹配目标地区
- `preferred`：优先匹配，匹配不到再降级
- `fallback`：允许跨区兜底

#### allow_region_fallback
低库存时是否允许地区降级。

---

## 6. 并发与动态阈值设计

### 6.1 可用比例目标
代理池健康区间目标：
- **40% - 60%** 可用代理占比

### 6.2 动态规则

#### 低并发
- 降低库存压力
- 但必须保证即时可分配
- 可用比例可贴近下边界

#### 高并发
- 提高最低库存要求
- 提高按地区保有量
- 提高补池触发灵敏度
- 可用比例目标偏向上边界

### 6.3 建议策略表达

```text
if concurrency <= low_threshold:
    min_available_ratio = 0.40
elif concurrency >= high_threshold:
    min_available_ratio = 0.60
else:
    min_available_ratio = interpolate(0.40, 0.60)
```

同时加：
- `min_available_per_region`
- `min_hot_spare_count`

避免总池量看似够，但某个地区断供。

---

## 7. 代理池自生长机制

### 7.1 触发条件
建议以下情况触发补池：

- 总可用比例低于阈值
- 某地区可用量低于阈值
- 高并发进入高水位
- 某类代理连续失效较多

### 7.2 自生长阶段划分

#### Phase 1
先定义接口与策略，不实现真实自动采购/抓取。

#### Phase 2
接入补池来源：
- 外部供应源
- 导入源
- 内部生成/回收逻辑

#### Phase 3
形成闭环：
- 自动发现不足
- 自动补充
- 自动验证
- 自动淘汰

---

## 8. 代理验证机制

### 8.1 验证层级

#### Level 1: 基础连通
- 代理是否能连接

#### Level 2: HTTP 成功
- 能否通过代理完成基础请求

#### Level 3: Runner 成功
- 能否通过浏览器执行链路成功访问目标

系统应以 **Level 3** 作为最终可用判断的主要依据。

### 8.2 验证结果使用方式

验证结果应影响：
- status
- success_rate
- cooldown
- evict / recover
- region inventory decision

---

## 9. runner 接口约束

无论 fake runner 还是真实 runner，都建议接受统一的执行输入：

```text
RunnerExecutionContext
- task
- task_payload
- network_policy
- fingerprint_profile
- proxy_allocation
- execution_limits
```

这样 fake runner 与 real runner 可以共享相同上层协议。

---

## 10. fake runner 阶段的落地建议

在 fake runner 阶段，不需要真实改浏览器内核，但建议先模拟：

- 指纹 profile 选择
- 代理分配逻辑
- 地区匹配逻辑
- 代理验证结果回写
- 成功/失败/超时

目标是先把“调度与策略层”跑通。

---

## 11. 风险与注意点

### 风险 1
高级指纹过早做太重，会拖慢早期闭环。

### 风险 2
代理池如果只有“列表”而无验证系统，后续会极不稳定。

### 风险 3
如果不提前把地区作为一等字段，后续返工会很大。

### 风险 4
如果允许直连兜底，会导致代理池设计失真。

---

## 12. 推荐下一步

1. 把本设计文档拆成具体数据结构草案
2. 设计数据库表：fingerprint / proxy / validation / allocation
3. 在 Rust 工程骨架里预留 network_identity 模块
4. fake runner 阶段先实现策略模拟，不急于真实注入
