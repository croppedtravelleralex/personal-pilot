# FINGERPRINT_BOUNDARY.md

`PersonaPilot` 浏览器指纹能力边界设计。

---

## 1. 目标

本文件回答三个问题：

1. 当前阶段到底要做哪些指纹能力
2. 哪些能力现在不该做
3. 指纹系统未来如何与代理池、身份画像、站点策略联动

一句话：

> 先定义“可控、可验证、可落地”的指纹边界，再考虑高级拟真。

---

## 2. 当前阶段定位

当前项目仍处于：

- 后端任务系统已跑通
- runner 抽象已形成
- LightpandaRunner 正在从最小真实执行走向稳定可用

所以指纹能力当前**不应直接做成重浏览器拟真系统**，而应先做成：

- 可配置
- 可落盘
- 可注入
- 可验证
- 可与任务/站点绑定

的第一版策略层。

---

## 3. 指纹能力分层

### L0：声明层（当前最优先）

目标：先定义数据结构与控制面，而不是立刻做深度拟真。

能力包括：
- `fingerprint_profile_id`
- `fingerprint_policy_json`
- 任务级 / 站点级 / 账户级 指纹选择入口
- 指纹字段白名单
- 指纹版本号与审计字段

这层先解决“怎么表达”和“怎么绑定”。

### L1：注入层（近期）

目标：支持基础指纹参数注入。

能力包括：
- User-Agent
- Accept-Language
- Timezone
- Locale
- Viewport / Screen 尺寸
- Platform
- HardwareConcurrency
- DeviceMemory
- WebGL vendor / renderer（如果执行器支持）
- Navigator 基础字段映射

这层解决“最小可控指纹输入”。

### L2：一致性层（中期）

目标：保证单个身份画像内部不打架。

能力包括：
- 时区与 IP 地区一致
- 语言与地区一致
- 屏幕与设备类型一致
- 平台 / UA / 浏览器版本一致
- 指纹模板内部冲突检查

这层解决“不要露出明显破绽”。

### L3：拟真层（后期）

目标：进一步降低被检测概率。

能力包括：
- Canvas / Audio / WebGL 扰动策略
- 字体集差异化
- 插件与 MIME 类型模拟
- 行为节奏与输入特征模拟
- 历史 cookie / session continuity

这层不应在当前阶段优先推进。

---

## 4. 当前阶段明确要做的能力

### 4.1 数据模型

至少要新增 / 预留：
- `fingerprint_profiles` 表
- `identity_profiles` 表（可后置）
- task 与 profile 的绑定字段
- profile 的 `version / status / tags / created_at`

### 4.2 配置结构

第一版建议字段：
- `user_agent`
- `accept_language`
- `timezone`
- `locale`
- `platform`
- `viewport_width`
- `viewport_height`
- `screen_width`
- `screen_height`
- `device_pixel_ratio`
- `hardware_concurrency`
- `device_memory_gb`
- `webgl_vendor`
- `webgl_renderer`

### 4.3 使用方式

第一版优先支持：
1. 任务创建时显式指定 fingerprint profile
2. 若未指定，则使用站点默认策略
3. 若站点默认不存在，则回退到系统默认 profile

### 4.4 可观测性

至少应支持：
- 当前任务命中了哪个 profile
- profile 是否经过一致性校验
- 注入失败时的错误日志
- 指纹字段审计输出（脱敏后）

---

## 5. 当前阶段明确不做的能力

现在不应优先投入：
- 完整 anti-detect 浏览器模拟
- 极重的 Canvas/WebGL 噪声引擎
- 字体枚举拟真系统
- 插件生态深拟真
- 鼠标轨迹 / 键盘行为模拟系统
- 跨数百站点的一次性“万能指纹”策略

原因：
- 当前主线还在 runner 与控制面收口
- 过早做重拟真会导致验证成本极高
- 没有代理池与身份画像协同前，高级指纹单独存在价值有限

---

## 6. 与代理池的关系

指纹能力不能单独看，必须和代理池联动。

### 当前原则
- 指纹先独立建模
- 代理先独立建模
- 到中期再做统一调度

### 未来联动规则
- IP 地区必须与时区 / locale / language 协调
- 住宅 / 数据中心代理应匹配不同风险等级 profile
- 同一 identity profile 尽量复用稳定地区与设备类型

---

## 7. 与身份画像系统的关系

未来建议关系：

- `fingerprint_profile`：偏浏览器设备参数
- `identity_profile`：偏“这个用户是谁”
- `network_profile`：偏代理与网络约束

三者共同构成完整执行身份。

当前阶段先不强耦合，但设计时要预留组合能力。

---

## 8. 性能预算原则

指纹能力不是越重越好。

### 当前预算原则
1. 默认 profile 注入必须是低开销
2. 高级字段必须可选，不要强制每任务都启用
3. 一致性校验优先做静态校验，不做重运行时计算
4. 指纹生成与选择逻辑不能压过核心执行耗时

### 当前推荐策略
- 先做静态模板
- 再做模板校验
- 最后才做动态扰动

---

## 9. 第一版落地顺序

1. 明确 profile schema
2. 增加 profile 存储表
3. 增加任务与 profile 绑定字段
4. 增加 profile 读取与回显能力
5. 为 runner 增加 profile 注入入口第一版（已完成）
6. 增加 profile 一致性校验器第一版（已完成）
7. 定义 Lightpanda 对 profile 的真实消费方式（当前下一步）

---

## 10. 当前结论

当前最合理的推进方式不是“马上做反检测浏览器”，而是：

> 先把指纹系统做成一个可配置、可持久化、可绑定、可验证的工程模块。

只有这样，后续代理池、身份画像、站点策略和真实执行器接入时，项目才不会再次返工。

## 11. 当前已落地状态（2026-03-30）

当前已经完成：
- `fingerprint_profiles` 表与最小管理接口
- profile 静态一致性校验第一版
- 任务与 `fingerprint_profile_id / fingerprint_profile_version` 绑定
- runner claim 阶段联表读取 active profile 并注入 `RunnerTask`
- fake/lightpanda runner 结果回显 `fingerprint_profile`
- inactive profile 在任务创建阶段直接拒绝
- 缺失 profile 或 stale version 的历史绑定在执行阶段安全降级为“不注入 profile”

当前还没完成：
- Lightpanda 真正消费 profile（例如把 profile 映射到命令参数、环境变量或浏览器上下文）
- profile 命中与注入的更细粒度 metrics / logs
- profile 与代理池 / identity profile 的联合调度
