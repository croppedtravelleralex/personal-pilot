# 01 项目章程

## 项目目标

`PersonaPilot` 是一个围绕 Browser API、代理池、指纹配置、任务编排与 explainability 的自动化控制面项目。

当前外部合同以现有 Browser API 和控制面主入口为准，本轮整改不改外部主入口名称。

## 本轮边界

本轮明确做：

- 双模代理口径：`demo_public` 与 `prod_live`
- 指纹 canonical schema 与 runtime 真值落地
- `/status` / task detail / runs / longrun / release 报告统一 mode 与 continuity 口径
- `prod_live` 的真实源、连续性、release profile 验收

本轮明确不做：

- Browser API 对外重命名
- L3 反检测拟真全面补齐
- 为了“文档看起来完整”而进行大范围无关重构

## 成功标准

- 公开源只作为 `public-smoke` / `demo_public` 验证链，不再为 production-live 背书
- `prod_live` 验收必须以私有/受控 source、active pool、browser success、continuity 观测为准
- explainability 与 release/profile 报告能直接说明：
  - 当前 mode
  - 来源层级
  - 验证路径
  - continuity level
  - 指纹消费真源
