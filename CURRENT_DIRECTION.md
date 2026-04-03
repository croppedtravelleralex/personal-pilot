# CURRENT_DIRECTION.md

## 当前方向

当前优先级最高的目标，不再是继续补“最小骨架”，也不是继续把控制面收口误报成总完成度，而是先按 **fingerprint-first** 规则约束后续实现：真实指纹消费、一致性、性能预算优先。

1. 让 **proxy selection** 从“多段规则叠加”继续收敛到 **trust score 核心表达**；近期已把 trust/cached trust 主链里的原始分数二次兜底移除，减少 raw score 重复参与排序。
2. 让 **verify / smoke / batch verify / 巡检** 形成更稳定的质量闭环
3. 让 **文档、策略、代码主链** 保持同一口径，避免自动推进被旧文档误导
4. 在继续加能力之前，先补齐 **性能预算、写放大控制、可观测性与风险边界**

## 当前判断

项目已经明显脱离“先搭最小原型”的阶段。

当前系统已经具备：
- 浏览器执行系统 V1
- Fingerprint profile 注入第一版
- 代理池基础能力
- sticky session 正式绑定
- smoke / verify / batch verify / 巡检 V1
- 代理选择策略层第一版
- trust score 起点与主链接入

因此，当前阶段的主要方向是：

> **继续把 proxy selection 的真实决策语义统一收进 trust score / risk score 表达，同时补强 verify 慢路径、性能治理和状态一致性。**

补充方向：允许未来增强 HTML/debug/trace retention，但不因此提升 screenshot / GUI / 重视觉能力的优先级；更大磁盘不改变 fingerprint-first 主线。

## 本阶段重点

### 主线 A：trust score 核心深化
- 把更多 selection 语义从散落的 order / rule / 特判中收敛到统一 score
- 继续明确长期成功率、近期失败衰减、provider 稳定性、provider × region 风险的权重边界
- 让 tuning 从“可注入”继续走向“可解释、可观测、可调参”

### 主线 B：verify 质量闭环深化
- 继续把 smoke、verify、batch verify、巡检结果统一成稳定质量信号
- 推进更真实的匿名性 / 地区校验链
- 让 selection 对 verify 信号的使用更一致，减少“文档里有、排序里没完全吃进去”的漂移

### 主线 C：工程治理
- 收口旧文档，避免 CURRENT_* 与 STATUS / PROGRESS 出现阶段错位
- 控制高并发下的数据库写放大、claim/reclaim 抖动与状态竞争
- 补齐 metrics / status 暴露，让策略效果不是黑盒

## 当前阶段定义

当前阶段不是“继续无差别加功能”，而是：

> **围绕 trust score + verify 闭环，做一次系统收敛，让 selection 逻辑、质量信号、运行状态、调优入口开始真正统一。**


补充方向：允许未来增强 HTML/debug/trace retention，但不因此提升 screenshot / GUI / 重视觉能力的优先级；更大磁盘不改变 fingerprint-first 主线。
