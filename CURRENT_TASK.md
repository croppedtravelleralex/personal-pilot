# CURRENT_TASK.md

## 当前任务

当前任务已进入 **绝对指纹优先第一批真实实现**。当前已完成指纹字段优先级分层、指纹-代理-地区一致性检查、性能/并发预算第一轮接线、explain/status 可观测性补齐、并发预算行为回归测试补强、fingerprint runtime explain 闭环，以及 explainability/API 聚合接线；当前主线继续向 **trust score 核心化** 收口，近期已去掉 trust/cached trust 主链中的原始分数二次兜底。

---

## 任务目标

围绕 `lightpanda-automation`，当前阶段先完成以下四件事：

1. **继续收敛代理选择主链**
   - 检查 selection 中仍分散存在的排序项、特判项、兜底项
   - 判断哪些应该继续并入 trust score / risk score
   - 近期已完成 trust/cached trust 主链移除原始分数二次兜底，继续降低“规则很多，但真实主排序语义分散”的维护成本

2. **补强代理质量信号闭环**
   - 继续完善 verify 慢路径
   - 强化匿名性 / 地区 / 出口真实性相关信号
   - 让 smoke、verify、batch verify、巡检结果对 selection 的影响更一致

3. **补一轮性能与稳定性治理**
   - 检查高并发下的 SQL 写放大、claim/reclaim 抖动、状态竞争
   - 检查 status 聚合、批量巡检、代理回写对数据库的额外压力
   - 继续压 panic 风险点与 flaky 测试点

4. **同步文档与真实阶段**
   - 更新 `CURRENT_DIRECTION.md`、`CURRENT_TASK.md`、`TODO.md`
   - 确保文档反映当前真实主线，而不是历史阶段目标
   - 保证后续自动推进围绕当前主线行动

---

## 当前阶段交付物

本阶段应优先补齐：

- [x] 代理选择策略层第一版
- [x] `ProxySelectionTuning` 注入入口
- [x] trust score 起点与主链接入
- [x] `verify_proxy` task kind
- [x] `POST /proxies/verify-batch`
- [x] 巡检批次查询与结果回看
- [x] `proxy_session_bindings` 正式 sticky 绑定
- [x] 当前阶段文档初步总结（`STATUS / PROGRESS / STAGE_SUMMARY`）
- [ ] CURRENT_* 与 TODO 文档同步到当前真实阶段
- [ ] trust score 语义继续核心化
- [ ] 更真实的 verify 慢路径
- [ ] 高并发写放大与性能预算收口
- [ ] 代理质量评分系统正式化
- [ ] Identity Profile / SessionIdentity 设计落地第一版

---

## 下一步优先级

### P0
1. **继续推进 trust score 核心化**，把更多 selection 语义统一进 score 表达
2. **推进 verify 慢路径**，补更真实的匿名性 / 地区 / 出口真实性校验链
3. **整理 TODO / CURRENT_* 文档**，让文档与代码阶段保持一致
4. **补一轮高并发性能治理**，重点看写放大、claim/reclaim、批量验证压力
5. **设计代理质量评分系统**，明确 verify / smoke / 历史成功率 / provider 风险的合成口径
6. **设计 Identity Profile / SessionIdentity**，把 proxy + fingerprint + region + risk_level 收到统一身份表达

### P1
7. 继续补 selection / verify / batch verify 的 metrics 与 explainability
8. 继续压测 proxy selection 查询、status 聚合 SQL 与 verify 批次链路
9. 继续收口 panic 风险点、锁竞争风险点与 flaky 测试
10. 继续完善 API / 运维 / 能力说明文档

### P2
11. 设计策略引擎正式形态
12. 设计行为层模拟机制
13. 设计会话连续性机制
14. 设计实验记录系统
15. 评估高级指纹能力的性能预算与真实接入边界

---

## 判定标准

如果一个推进动作不能帮助回答下面任一问题，就应降低优先级：

- 它是否让 **trust score / selection** 更统一？
- 它是否让 **verify 质量信号** 更可信？
- 它是否让 **并发稳定性 / 写放大 / 可观测性** 更稳？
- 它是否让 **proxy + fingerprint + identity** 的主线更清晰？
- 它是否让 **文档与代码** 更一致，减少自动推进跑偏风险？


补充约束：当前规则已明确，磁盘扩容不再是主限制，但功能排序仍默认服从“绝对指纹优先”；后续实现必须优先服务真实指纹消费、地区一致性和吞吐不塌陷的性能预算。
