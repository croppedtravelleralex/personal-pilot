# 外部分发准备说明

Updated: 2026-06-01 (Asia/Shanghai)

## 结论

PersonaPilot 当前具备 Win11 本地安装包生成能力，可以进入受控外部分发前检查；但不应直接宣称完整生产闭环。当前主线交付口径仍是 `100% / 0% / green`，整体终态口径仍是 `40% / 60% / yellow`。

## 可分发资产

- Win11 主线入口：`personal-pilot-tauri.exe`
- 当前仓库只保留这一份用户可打开 GUI exe；安装包和 `src-tauri/target/release/*.exe` 只允许作为临时构建输出，不作为持久入口。
- 自动化 release gate 已通过：`scripts/windows_local_verify.ps1 -SkipContinuityTest`
- 基础技术栈符合项目规则：Tauri 2 + Vite + React + TypeScript
- Native / system capability 统一通过 `src/services/desktop.ts` 暴露
- Validation Board、Settings provider readiness、Automation behavior audit、Overview runtime posture 均已有 operator surface

## 分发前必须保留的限制

1. Provider closure：CAPTCHA / SMS / Email 已有 readiness surface 和 blockers，但真实 provider smoke、manager wiring、CDP detect/fill 和 operator 闭环未完成。
2. Profile portability：`SessionBundle` export/import/preflight/dry-run/confirmed local restore 已落地，但跨机器 portability smoke 未完成。
3. Runtime measurement：M5 已有 release health v2 report 和 gate；当前 `personal-pilot-tauri.exe` clean smoke 仍为 warning / `budgetStatus=over_budget`，`941ms` cold start passed，但 `467MB` idle RSS 和 `9` processes 超过默认预算。外部分发前应先做性能 mitigation 或明确记录例外，不能把 `passed_with_budget_overrun` 写成 performance green。
4. Validation evidence：P5 Lightpanda/CDP smoke 可重复生成 profile runtime evidence report，但 WebRTC/audio warning、canvas failure 等 failure reason 必须保留。
5. Fingerprint depth：当前是 `80` declared controls / `26` runtime-projected fields / `450` taxonomy seed，不是全量 observed fingerprint coverage。
6. Behavior depth：当前是 `13` shipped primitives / `8` page archetypes / `450` taxonomy seed，不是完整 replay taxonomy。
7. AdsPower boundary：P12 结论是 refresh deferred；没有 B1-B5 新证据前不得重算 score 或宣称追平。

## 手工 operator smoke 清单

发布给外部用户前，至少手工确认以下路径：

1. 安装包可在干净 Win11 机器安装、启动和卸载。
2. 首屏 Dashboard、Profiles、Proxies、Automation、Synchronizer、Logs、Settings、Validation 页面可打开。
3. Settings 的 CAPTCHA / SMS / Email readiness 能展示 blockers，不把未配置 provider 显示为 ready。
4. Validation Board 能读取最近 report history，并能导出 profile evidence package。
5. Overview/Dashboard runtime posture 能显示 release health/budget warning，而不是把 dev-mode 指标或 `passed_with_budget_overrun` 当成 release performance green。
6. Logs 页面分页或增量加载正常，大量日志不会一次性全量渲染。
7. Profile / SessionBundle 的 export/import preflight/dry-run/confirmed local restore 在测试 profile 上可控执行，不误写真实生产 profile。

## 发布说明建议

对外说明应使用以下事实边界：

- 这是 Win11 本地桌面 operator console。
- 已有 validation evidence、profile/session bundle、provider readiness、automation audit 和 runtime posture 的本地可视化能力。
- 当前还不是完整 AdsPower 级 antidetect/browser-kernel platform。
- 当前不托管 Chromium / Firefox fork，外部浏览器能力只能通过 adapter contract 逐步接入。
- Provider、profile portability、release performance mitigation 和 `450` taxonomy seed 到 observed/replay runtime 的转化都属于下一阶段验收项；M5 health gate 只是本机 release budget report contract，不是性能达标证明。

## P20 外部条件 smoke gate

`external_distribution_smoke.ps1` v2 会区分本机资产检查和外部条件检查：安装包、文档限制和 release report 属于 local asset gate；manual operator smoke、干净 Win11 安装/启动/卸载、页面导航、provider readiness 和 SessionBundle smoke 属于 external gate。外部 gate 未通过时状态必须保留为 `blocked_external_smoke_required`，不能写成外部分发完成。

`session_bundle_portability_smoke.ps1` v2 会输出 cross-machine gateResults 和 failureReason。未提供第二台干净 Win11 证据时只能算 `local_contract_passed` 或 `blocked_requires_second_machine_evidence`，不能写成 profile portability closure。
## P13 退出条件

- 分发前限制、手工 smoke、发布说明边界已经写入维护文档。
- 根入口和 docs 入口能指向本文件。
- 阶段状态、TODO、roadmap、backlog 与本文件一致。
- 文档同步后通过 JSON、stage entry consistency、diff check、build/check 和 Win11/Tauri enforcement。
