# Proxy & Fingerprint Requirements

## 核心目标

基于目标开源浏览器项目（即后续接入的真实浏览器引擎），实现：

- 小体积 / 小头浏览器模式下的高并发运行
- 即使启用高级指纹能力，也保持较小性能开销
- 所有访问默认强制经过代理池
- 代理池通过实际验证结果动态演进
- 代理池规模可自生长，并维持健康可用代理占比

## 指纹要求

### 最终目标
- 在高并发场景下，指纹能力不能显著拖垮性能
- 高级指纹配置应尽量走模板化 / 策略化，而不是每次现场拼装
- fake runner 阶段可先模拟，real runner 阶段要落到真实浏览器配置

### 约束
- 指纹能力是正式模块，不是零散参数
- 需要支持 profile / strategy / task binding
- 性能预算必须纳入设计考量

## 代理池要求

### 最终目标
- 通过开源项目实际验证代理可用性
- 形成代理池自生长机制
- 保持代理池中可用代理比例在 **40% - 60%** 之间
- 高并发时阈值可适当提高
- 低并发时阈值可降低，但必须保证“随时有可用代理可供分配”

### 地区要求
- 不同访问目标可能需要不同地区代理
- 代理池需要按地区维护基础可用存量
- 当任务指向不同地区目标时，应优先匹配对应区域代理

### 强制要求
- 每一次访问都必须走代理池分配的代理
- 不允许直连绕过代理池

## 代理池机制要求

### 1. 健康度验证
- 代理不是只看“能连通”，而要看“通过目标开源项目验证成功”
- 需要有可追踪的成功/失败记录

### 2. 自生长
- 当某地区、某类型代理不足时，系统应触发补充策略
- 补充策略可先设计为接口/占位，不必第一阶段全部做完

### 3. 动态阈值
- 高并发：提高池中保有量要求
- 低并发：维持较低但安全的库存
- 重点不是固定数字，而是“供给不断档”

### 4. 地区感知
- 代理实体必须带地区属性
- 任务模型应支持声明目标地区 / 代理地区约束
- 代理选择器需要具备 region-aware 能力

## 建议的数据模型方向

### FingerprintProfile
- id
- name
- browser family
- ua / locale / timezone / viewport
- advanced fingerprint options
- perf budget tag

### ProxyEndpoint
- id
- host / port / protocol
- auth
- region
- provider
- status
- last_check_at
- success_rate
- fail_count

### ProxyPoolPolicy
- min_available_ratio
- max_available_ratio
- min_available_per_region
- concurrency_scaling_rule
- replenish_rule
- validation_rule

### TaskNetworkPolicy
- require_proxy = true
- target_region
- proxy_region_match_mode
- fingerprint_profile_id
- proxy_strategy

## 设计原则

1. 先把代理池 / 指纹抽象做好，再往 runner 里塞实现
2. 所有访问强制走代理池，不要留直连旁路
3. 代理是否可用，以真实验证结果为准
4. 地区维度必须是一等公民
5. 并发变化要能驱动池量策略变化
6. 高级指纹不能成为性能黑洞
