# PersonaPilot

PersonaPilot 是一个面向 Windows 11 的本地桌面 operator console，用一个 Tauri 应用管理 browser-profile 工作面、代理状态、自动化运行、同步器、日志、设置和 validation evidence。

本仓库固定使用 **Tauri 2 + Vite + React + TypeScript**。除非用户明确批准，否则不引入 Electron、Node 后端服务、内嵌 Python runtime 或多窗口 embedded-browser 架构。

## 当前状态

- 主线交付：`100% / 0% / green`。
- 整体终态：`40% / 60% / yellow`。
- Win11 release gate 已于 2026-05-23 通过。
- 当前主线入口只保留根目录 `personal-pilot-tauri.exe`；release build 的 target 产物只作为临时构建输出，完成后必须同步到根目录并清理。除该 root exe 外，不允许保留其他用户可打开 GUI exe 或旁路 UI。
- Validation Board 已进入 Dashboard / evidence history surface，并严格区分 `declared / applied / observed` evidence。
- 当前范围已收缩为本机自用：不再追求外部分发 smoke、干净 Win11/第二机验收、跨机器 SessionBundle portability 或 release performance 预算达标。
- release / external / cross-machine 相关 report 和脚本只保留为历史诊断材料，不再作为待办、阻塞项或成功标准。

维护真相源在 `docs/`。接手、汇报、规划时先读 `/docs/README.md`、`/docs/root-entrypoint-map.md` 和 `/docs/02-current-state.md`，不要把根 README 当成唯一事实来源。

## 已包含能力

- Dashboard、Browser List、Workbench/Synchronizer、Recording、Automation、Core、Proxy、Bookmarks、Tags、Monitor、Settings、Tutorial、Logs 和 API docs 页面；Validation evidence 作为 Dashboard/报告面板呈现，不是独立导航页。
- Native desktop 能力统一经 `src/services/desktop.ts` 暴露。
- Provider-aware proxy rotation contract，包含 rollback、cooldown、retry 语义。
- Native-first recorder/template flow，fallback 只作为恢复路径。
- Synchronizer read/focus/set-main/layout native desktop contract。
- Validation Board 覆盖 detector、leak、DNS、WebRTC、canvas、audio、worker、transport evidence 类别。
- Provider readiness、behavior audit、runtime posture 和历史 release measurement 诊断可在 operator surface 查看。

## 重要边界

- 当前 fingerprint depth 仍未完整：`80` declared controls / `26` runtime projected fields (`25` control-supported + derived `platform`) / `450` taxonomy seed / full observed coverage pending。
- Validation observed evidence 包含 DNS/transport report、desktop WebView probes、profile runtime probe contract、report history、profile evidence export 和 P5 Lightpanda/CDP smoke；report 中的 WebRTC/audio warning、canvas failure 必须保留。
- CAPTCHA/SMS/Email handler、route、readiness checklist 已部分落地，但 production manager wiring、真实 provider acceptance、CDP detect/fill 和 operator 闭环未完成。
- `450` fingerprint signal taxonomy seed 与 `450` behavior event taxonomy seed 已有机器可读清单，但 full observed coverage 和 full replay runtime 仍未交付。
- P12 AdsPower refresh 结论是 deferred；没有 B1-B5 新证据时不得重算 score 或宣称追平。

## 快速开始

前置条件：

- Windows 11
- Node.js 和 pnpm
- 兼容 Tauri 2 的 Rust toolchain

安装依赖：

```powershell
pnpm install
```

启动 Web dev shell：

```powershell
pnpm dev
```

构建前端：

```powershell
pnpm build
```

构建 Windows 桌面 release：

```powershell
pnpm desktop:release
```

运行项目基线检查：

```powershell
powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot D:\SelfMadeTool\personal-pilot
```

## 质量门禁

每个 meaningful code change 至少应通过：

- `pnpm typecheck`
- `pnpm build`
- Win11/Tauri baseline enforcement
- release 相关改动还要跑 `pnpm desktop:release`

更完整的本地验证入口：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\windows_local_verify.ps1 -SkipContinuityTest
```

## 项目结构

```text
src/
  App.tsx
  main.tsx
  modules/
    dashboard/
    browser/
    monitor/
    settings/
    synchronizer/
    charts/        # 未进导航的内部 demo/sample route: /charts
  services/
  types/
  shared/
  api/             # Rust crate modules
  desktop/         # Rust crate modules
  runner/          # Rust crate modules
src-tauri/
  src/
  capabilities/
  tauri.conf.json
data/
scripts/
docs/
```

Native 和 system-capability 调用必须走固定链路：

```text
src/modules/** 或 app shell -> src/services/desktop.ts -> tauri
```

UI 代码不得直接调用 Tauri。

## 维护文档

核心入口：

- `docs/README.md`
- `docs/02-current-state.md`
- `docs/03-roadmap.md`
- `docs/04-improvement-backlog.md`
- `docs/05-ai-maintenance-playbook.md`
- `docs/root-entrypoint-map.md`
- `docs/24-external-distribution-readiness.md`（historical-only / cancelled）

状态变化时，先更新对应 canonical docs，再更新根入口摘要。

## 下一步

1. 继续按本机自用路线收缩 `tauriWailsBridge` 和剩余 settings/core/proxy bridge API。
2. 继续扩大本机 Validation / fingerprint observed coverage，保留真实 warning/failure reason。
3. 继续补齐本机 workflow/task run history、取消/暂停 controls 和行为 replay 证据。
4. 按需补齐 CAPTCHA/SMS/Email 本机 operator 配置和 provider dry-run；真实 provider smoke 只在用户实际提供凭证时再做。
