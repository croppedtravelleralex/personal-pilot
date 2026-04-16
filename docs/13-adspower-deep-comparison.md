# AdsPower 深度对比
Updated: 2026-04-16 (Asia/Shanghai)

## 统一口径

- 主线交付：`95% / 7% / green`
- 整体终局：`30% / 70% / yellow`
- 本文属于 `整体终局` 轨道，不是当前主线 closeout 进度板

详细阶段任务、评分板、以及 `当前 / 目标 / AdsPower` 的统一汇报入口，见 `docs/19-phase-plan-and-scorecard.md`。

## 对比前的已核实基线

- first-family 控制面已声明 `80` 个核心控制字段
- 当前 `Lightpanda` 运行时只实际投影 `12` 个 env-backed 指纹字段，包含派生 `platform`
- 当前行为运行时只有 `13` 个 shipped primitives
- cookie / localStorage / sessionStorage continuity 已支持关软件重启后恢复
- Dashboard / Profiles / Proxies / Automation / Logs / Settings 等主工作台已经落到真实桌面操作面

这些事实说明当前已经不是“没页面、没程序入口”的阶段，但也还没有到 AdsPower 级指纹真实性、内核深度和自动化广度。

## 和 AdsPower 的当前边界

| 维度 | PersonaPilot 当前事实 | AdsPower 边界 | 判断 |
| --- | --- | --- | --- |
| 指纹 / 内核 | 有 `80` 控制字段的 first-family 起点，但运行时只投影 `12` 个字段 | 更深的运行时 materialization、headed 内核深度、更多真实设备族和成熟验证体系 | AdsPower 明显领先 |
| 指纹真实性 | 已有一致性规则和 explain contract，但真实性证据板还没建完 | 更成熟的真实性、泄漏、检测对抗与长期验证资产 | 我们仍在补 observation / validation 层 |
| Profiles / Session | 已有真实工作台和重启后 continuity | 更成熟的 profile groups、import/export、团队协作、跨环境迁移 | 我们基础已成型，但生态仍弱 |
| Proxies / IP | 读侧和本地 contract 已成型，provider-grade 写侧仍未收口 | provider 生态、稳定轮换、地理一致性与批量治理更成熟 | 当前差距集中在 provider semantics 与治理能力 |
| Automation / RPA | 当前只有 `13` 个 shipped primitives，已有 recorder / templates / task surface | 更丰富的事件语法、调试工具、模板生态、批量自动化能力 | 差距很大，`450+` 事件仍属终局目标 |
| Operator Surface | 已有像样的桌面页面和程序入口，不是空壳 | 更完整的产品矩阵和团队管理能力 | 我们已像产品，但还没到同层级 |

## 为什么不是“已经追平”

- 当前主线 `95% / 7%` 只代表 Win11 本地桌面 App closeout，不能外推成 AdsPower 级整体成熟度
- `50+` 现在应理解为最低控制面门槛，而当前 first-family 已声明 `80` 个控制字段
- 这 `80` 个字段不等于都已经被 runtime 消费；当前 runtime 投影仍只有 `12`
- `450+` 指纹信号和 `450+` 事件类型都还是整体终局目标，不是当前 shipped depth
- 外部浏览器研究和整合方案已经完成，但方案不等于已落地的内核能力

## 追评 AdsPower 应该并到哪条线

AdsPower 追评属于 `整体终局 30% / 70% / yellow` 的范围，主要覆盖：

1. 更深的指纹 / 内核 materialization
2. 更强的真实性 observation / validation board
3. 更成熟的 proxy / IP provider 生态和一致性治理
4. 从 `13` 个 primitives 扩到 `450+` 事件 taxonomy
5. 更完整的 session bundle、profile portability、operator tooling
6. 外部项目高价值能力的无损吸收与长期维护落地

## 当前结论

- 如果问“当前桌面 App 主线是否接近收口”，答案是：`95% / 7% / green`
- 如果问“是否已经做到 AdsPower 级丰富、真实、完整”，答案是：还没有，这部分应按 `30% / 70% / yellow` 口径理解
- 当前和 AdsPower 的差距，核心不在“有没有页面”，而在真实性、运行时深度、验证证据、代理生态和自动化广度
