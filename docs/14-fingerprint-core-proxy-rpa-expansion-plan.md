# 指纹 / 内核 / 代理 / RPA 扩张方案
Updated: 2026-04-16 (Asia/Shanghai)

## 统一口径

- 主线交付：`100% / 0% / green`
- 整体终局：`35% / 65% / yellow`
- 本文属于 `整体终局` 扩张方案，不属于当前主线 `7%` closeout

详细阶段任务、评分口径、和 `当前 / 目标 / AdsPower` 汇总入口，见 `docs/19-phase-plan-and-scorecard.md`。

## 当前基线

- first-family 控制面已声明 `80` 个核心控制字段
- 当前 `Lightpanda` 运行时只实际投影 `12` 个 env-backed 指纹字段
- 当前行为运行时只有 `13` 个 shipped primitives
- cookie / localStorage / sessionStorage continuity 已支持关软件重启后恢复
- 当前缺口不在“有没有桌面壳”，而在真实性、运行时深度、validation 和事件广度

## 目标

把当前工作台从“可用的本地桌面 App”推进到“真实设备谱系驱动的高一致性平台”。

核心原则不是随机化，而是：

- 真实设备族谱
- 约束一致性
- 有限熵预算
- 可解释回放
- 可批量复用

## 总体路线

### 第一层：指纹 / 内核

目标不是“字段更多”，而是“字段更真实、组合更稳定、解释更清楚”。

建议做成 4 个子系统：

1. `Profile Spec`：定义每个 profile 的设备族、熵预算、生命周期。
2. `Consistency Graph`：定义字段间的硬约束和软约束。
3. `Runtime Policy`：把 profile spec 映射为浏览器运行时策略。
4. `Explainability`：每次生成 / 变更都能说清为什么。

当前 `50+` 应理解成最低控制面门槛，而不是上限。
在 first-family 已经有 `80` 个核心控制字段的前提下，建议仍按 8 组组织，不做扁平堆字段：

| 组别 | 目标 | 示例 |
| --- | --- | --- |
| 身份层 | 浏览器身份基础 | UA、UA-CH、平台、品牌、版本通道 |
| Locale 层 | 地域语言一致性 | locale、accept-language、时区、地区 |
| 屏幕层 | 显示设备一致性 | 分辨率、DPR、色深、缩放、可视区 |
| 硬件层 | 设备形态一致性 | CPU、内存、触摸、设备类、功耗形态 |
| 渲染层 | 图形/字体/媒体一致性 | canvas、webgl、audio、fonts、media codecs |
| 权限层 | 浏览器能力暴露 | permissions、storage、clipboard、battery、sensor |
| 网络层 | 网络出口和地域一致性 | proxy、exit region、latency profile、DNS posture |
| 行为层 | 人机行为轮廓 | 输入节奏、滚动曲线、页面停留、操作密度 |

关键做法：

- 用 `device family` 生成 profile，而不是给每个字段独立随机。
- 给每个 profile 设 `entropy budget`，预算不够就不允许继续变异。
- 所有字段必须通过 `consistency graph`，例如时区、语言、代理区域、屏幕形态、硬件形态不能互相打架。
- 对每个 profile 输出 `coherence score` 和 `risk reasons`，而不是只有一个真假结果。

### 第二层：Proxies / IP

目标不是“换 IP 次数更多”，而是“代理状态和指纹状态同步”。

建议把代理层做成一个独立编排器，包含：

- `lease`：代理租约生命周期
- `sticky session`：短期稳定驻留
- `cooldown`：切换冷却
- `health`：连通性、延迟、失败率
- `reputation`：历史质量分
- `region coherence`：代理区域必须和指纹区域策略联动
- `fallback`：失败时按规则回退，而不是裸切换

代理层的真实能力应该是：

1. 代理不是单条记录，而是“可用出口 + 健康状态 + 归属 profile + 轮换策略”。
2. `changeProxyIp` 不是一个按钮，而是一个受约束的状态机。
3. 每次切换都要记录原因、结果、冷却窗口和回滚路径。

### 第三层：RPA / 事件扩张

目标不是把事件名堆到 `450+`，而是把事件语法扩展到足够丰富、可回放、可组合。

建议采用：

- `事件原语`：点击、输入、滚动、聚焦、切换、等待、校验、提交、撤销
- `上下文`：页面、表单、窗口、任务、代理、profile、录制器
- `阶段`：preflight、dispatch、monitoring、retry、recovery、cleanup
- `结果`：success、blocked、failed、degraded、manual_gate

这样 `450+` 事件不是人工枚举，而是从语法生成：

- 导航类
- 表单类
- 选择类
- 输入类
- 窗口类
- 网络类
- 代理类
- 任务类
- 录制类
- 回放类
- 错误恢复类
- 审计类

RPA 层应优先支持：

- workflow graph
- trace replay
- failure reason capture
- 可复用模板
- 断点恢复

## 推荐实施顺序

### Phase 1: 指纹 / 内核底座

交付物：

- `Profile Spec` 数据结构
- `80` 核心控制字段的分组 schema 与一致性图
- 一版 consistency graph
- profile 解释输出

验收：

- 每个 profile 能被归类到明确设备族
- 生成结果可复现
- 任何高风险组合都能被拦截并解释

### Phase 2: 代理 / IP 编排

交付物：

- 代理租约和冷却状态机
- 代理健康评分
- region / locale / timezone 联动
- change IP 失败回滚

验收：

- 切换可追踪
- 可回滚
- 与 profile 一致性联动

### Phase 3: RPA / 事件语法

交付物：

- `450+` 事件 taxonomy
- workflow graph
- replay / debug / audit

验收：

- 事件可组合
- 事件可回放
- 失败可定位

### Phase 4: release hardening

交付物：

- release build 验证
- 低噪声日志
- 默认 profile / proxy / automation 的稳定路径

## 建议的默认策略

- 默认优先真实一致性，不优先极端随机。
- 默认优先稳定 profile，不优先高频切换。
- 默认优先可解释与可回放，不优先黑盒自动化。
- 默认优先单个高质量 device family，再扩展到多族谱。

## 默认决策

基于你已经选定的方向，这里先锁三个默认值：

1. 第一版内核：`高真实性 / 低批量`。
2. 第一批设备族：`Win11 商务办公本 / 主流轻薄本` 作为主族，后续再扩 `家用台式机`、`创作者工作站`、`2-in-1 触屏设备`。
3. 代理策略：`稳定驻留优先`，只有健康恶化、地域不一致、任务切换或人工策略才触发换 IP。

为什么先选商务办公本：

- 覆盖面大，是真实世界里最常见的 Win11 日常设备形态之一。
- 形态相对稳定，适合先把一致性图做厚，而不是先处理太多硬件变体。
- 它和“稳定驻留代理”天然匹配，适合长会话、长任务、长生命周期 profile。
- 后续往桌面机、创作者机、触屏机扩展时，字段差异会更清晰，迁移也更平滑。

第一族先重点做的字段不是“最多”，而是“最像”：

- 浏览器身份与版本通道
- locale / accept-language / timezone / region
- 屏幕分辨率、DPR、缩放、可视区
- CPU / 内存 / 设备功耗形态
- 输入设备与交互节奏
- 渲染层特征（canvas / webgl / audio / fonts）
- 网络出口与代理地域一致性

详细字段级清单见 [docs/15-first-family-core-controls.md](/D:/SelfMadeTool/persona-pilot/docs/15-first-family-core-controls.md)。

代理驻留的默认建议：

- 先做 `session-level sticky`，不要默认每个动作都换。
- 换 IP 只在明确策略点发生，不在普通页面跳转时发生。
- 每次换 IP 都必须写入原因、结果、冷却窗口和回滚路径。
- 健康分数低于阈值后再退火，不做无条件高频轮换。

## 450+ 指纹怎么做才真实

`450+` 不应该理解成 `450` 个都要手工暴露的随机开关，而应该理解成 `450+` 个指纹信号总量。

更稳的做法是三层分离：

1. `控制面`：约 `60-80` 个真正可控的核心字段，决定 profile 族谱和主要一致性。
2. `派生面`：约 `120-160` 个由控制面推导出的字段或约束结果，用于补足细节。
3. `观测面`：约 `200+` 个用于解释、审计、回放、评分、风险提示的信号。

这样做的好处是：

- 不会把产品做成“450 个旋钮”的灾难。
- 真实 profile 只需加载一小部分核心控制，其他信号由族谱和规则生成。
- 你要的“覆盖全一些”能实现，但不会破坏一致性。

### 建议的 450+ 信号分布

| 域 | 目标 | 建议规模 |
| --- | --- | --- |
| 浏览器身份与版本族 | UA / UA-CH / 平台 / 渠道 / 兼容表达 | 35-45 |
| OS / Shell / 硬件族 | Windows build、设备形态、CPU/内存档位 | 45-60 |
| 屏幕 / 窗口 / 缩放 | 分辨率、DPR、缩放、可视区、多屏习惯 | 45-55 |
| 渲染 / 字体 / 媒体 | canvas、webgl、audio、fonts、codec、显卡族 | 60-75 |
| Locale / IME / 文本 | locale、语言序列、输入法、区域习惯 | 35-45 |
| 输入 / 行为节奏 | 鼠标、键盘、滚动、停留、切换节奏 | 55-70 |
| 网络 / 代理 / DNS | proxy、exit region、延迟姿态、DNS/posture | 40-50 |
| Storage / Cookie / 权限 | 持久化、权限面、隔离域、恢复态 | 30-40 |
| 安全 / 扩展 / Policy | policy、extension posture、权限边界 | 25-35 |
| 生命周期 / 会话 / 恢复 | 启动、暂停、恢复、退场、回滚、审计 | 30-40 |

### 450+ 的生成原则

- 只让少数核心字段真正参与“人类可见配置”。
- 大量细节字段由 `device family`、`build lane`、`render lane`、`behavior lane` 自动派生。
- 所有字段必须回到同一条 `coherence score` 链路上，不允许各自独立随机。
- profile 变更要区分 `control change` 和 `derived refresh`，避免一个字段改动导致整组漂移。

### 第一族的默认扩展重点

既然第一族先选 `Win11 商务办公本 / 主流轻薄本`，那 450+ 的前期重点不是铺满所有设备，而是把这族做厚：

- 先把该族的显示、输入、locale、渲染、网络、生命周期做完整。
- 再补同族的不同档位，如 `Intel/AMD`、`13/14/15/16 inch`、`1x/2x DPR`、`办公/会议/长驻` 场景差异。
- 最后再扩到台式机、创作者工作站、2-in-1 触屏设备。

## `450+` 事件怎么长出来

`450+` 事件不要手写堆名，建议用“原语 × 场景 × 阶段 × 结果”的方式生成。

一个更现实的结构是：

- 12 到 15 个事件原语
- 8 个场景组
- 4 个生命周期阶段
- 4 个结果态

这样基础组合就已经超过 `450`，再补上审计、恢复、控制、回放类事件即可。

建议优先覆盖的场景组：

- 导航与页面生命周期
- 表单录入与校验
- 列表与筛选
- 窗口与焦点
- 代理切换与健康检查
- 任务启动与监控
- 录制与回放
- 错误恢复与人工接管

## 当前默认收敛方向

1. `450+` 定义为“信号总量”，不是 `450` 个手工开关
2. 第一版内核优先“高真实性 / 低批量”
3. 第一批设备族优先 `Win11 商务办公本 / 主流轻薄本`
4. 代理策略优先“稳定驻留”，只有健康恶化、地域不一致、任务切换或人工策略才触发换 IP
5. 事件扩张优先从任务 / 代理 / 回放与表单 / 列表 / 窗口两条主线同时铺 taxonomy，再回收为统一语法

## 相关本地文件

- [src/network_identity/fingerprint_policy.rs](/D:/SelfMadeTool/persona-pilot/src/network_identity/fingerprint_policy.rs)
- [src/network_identity/fingerprint_consistency.rs](/D:/SelfMadeTool/persona-pilot/src/network_identity/fingerprint_consistency.rs)
- [src/network_identity/validator.rs](/D:/SelfMadeTool/persona-pilot/src/network_identity/validator.rs)
- [src/services/desktop.ts](/D:/SelfMadeTool/persona-pilot/src/services/desktop.ts)
- [src/features/proxies/model.ts](/D:/SelfMadeTool/persona-pilot/src/features/proxies/model.ts)
- [src/features/automation/model.ts](/D:/SelfMadeTool/persona-pilot/src/features/automation/model.ts)
