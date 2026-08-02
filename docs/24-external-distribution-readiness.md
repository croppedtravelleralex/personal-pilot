# 外部分发准备说明（CANCELLED / historical-only）

Updated: 2026-06-22 (Asia/Shanghai)

## 结论

2026-06-22 范围重置后，PersonaPilot 只按本机自用维护。外部分发 smoke、干净 Win11 安装/启动/卸载、第二机验收和人工 operator smoke 已取消，不再作为当前成功标准、阻塞项或下一步。

本文件只保留为历史上下文。除非用户明确重新开启外部分发目标，否则不要运行 `external_distribution_smoke.ps1`，不要按本文件创建 release gate，也不要把缺少外部分发报告写成当前风险。

## 仍然有效的本机事实

- Win11 主线入口：`personal-pilot-tauri.exe`。
- 当前仓库只保留这一份用户可打开 GUI exe；安装包和 `src-tauri/target/release/*.exe` 只允许作为临时构建输出，不作为持久入口。
- 基础技术栈：Tauri 2 + Vite + React + TypeScript。
- Native / system capability 统一通过 `src/services/desktop.ts` 暴露。
- Validation Board、Settings provider readiness、Automation behavior audit、Overview runtime posture 均已有本机 operator surface。

## 历史限制说明

以下内容过去用于外部分发前限制说明，现在只作诊断上下文：

1. Provider closure：CAPTCHA / SMS / Email 已有 readiness surface 和 blockers，但真实 provider smoke、manager wiring、CDP detect/fill 和 operator 闭环未完成。
2. Profile restore：`SessionBundle` export/import/preflight/dry-run/confirmed local restore 已落地；跨机器 portability smoke 已取消。
3. Runtime measurement：M5 release health v2 report 和 gate 只保留历史诊断价值；`passed_with_budget_overrun` 不代表 performance green，也不再要求优化到预算 green。
4. Validation evidence：P5 Lightpanda/CDP smoke 可重复生成 profile runtime evidence report，但 WebRTC/audio warning、canvas failure 等 failure reason 必须保留。
5. Fingerprint depth：当前是 `80` declared controls / `26` runtime-projected fields / `450` taxonomy seed，不是全量 observed fingerprint coverage。
6. Behavior depth：当前是 `13` shipped primitives / `8` page archetypes / `450` taxonomy seed，不是完整 replay taxonomy。
7. AdsPower boundary：P12 结论是 refresh deferred；没有 B1-B5 新证据前不得重算 score 或宣称追平。

## 已取消的检查

不要把以下项目列为当前待办：

- 已取消：外部分发前 manual operator smoke。
- 已取消：干净 Win11 机器安装、启动和卸载。
- 已取消：第二机或跨机器 `SessionBundle` portability。
- 已取消：release performance 预算达标。
- 对外发布说明验收。

## 如果未来重新开启外部分发

需要先由用户明确确认新目标，再重建当前版 checklist。旧 P13/P20 脚本和报告可以作为参考，但不能直接当作当前验收标准。
