# PersonaPilot

PersonaPilot 是一个面向 Windows 11 的本地桌面 operator console，用一个 Tauri 应用管理 browser-profile 工作面、代理状态、自动化运行、同步器、日志、设置和 validation evidence。

本仓库固定使用 **Tauri 2 + Vite + React + TypeScript**。除非用户明确批准，否则不引入 Electron、Node 后端服务、内嵌 Python runtime 或多窗口 embedded-browser 架构。

## 当前状态

- 主线交付：`100% / 0% / green`。
- 整体终态：`40% / 60% / yellow`。
- Win11 release gate 已于 2026-05-23 通过。
- 当前 release build 会生成 `src-tauri/target/release/bundle/nsis/PersonaPilot_0.1.0_x64-setup.exe`。
- Validation Board 已进入桌面导航，并严格区分 `declared / applied / observed` evidence。
- P13 已新增外部分发前检查入口：`docs/24-external-distribution-readiness.md`。

维护真相源在 `docs/`。接手、汇报、规划时先读 `/docs/README.md`、`/docs/root-entrypoint-map.md` 和 `/docs/02-current-state.md`，不要把根 README 当成唯一事实来源。

## 已包含能力

- Dashboard、Profiles、Proxies、Automation、Synchronizer、Logs、Settings、Validation 页面。
- Native desktop 能力统一经 `src/services/desktop.ts` 暴露。
- Provider-aware proxy rotation contract，包含 rollback、cooldown、retry 语义。
- Native-first recorder/template flow，fallback 只作为恢复路径。
- Synchronizer read/focus/set-main/layout native desktop contract。
- Validation Board 覆盖 detector、leak、DNS、WebRTC、canvas、audio、worker、transport evidence 类别。
- Provider readiness、behavior audit、runtime posture 和 release measurement pending 可在 operator surface 查看。

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
  app/
  pages/
  components/
  features/
  hooks/
  store/
  services/
  types/
  utils/
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
pages/components -> features/hooks/store -> src/services/desktop.ts -> tauri
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
- `docs/24-external-distribution-readiness.md`

状态变化时，先更新对应 canonical docs，再更新根入口摘要。

## 下一步

1. 按 `docs/24-external-distribution-readiness.md` 执行外部分发前人工 operator smoke。
2. 运行 `scripts/release_performance_smoke.ps1`、`scripts/external_distribution_smoke.ps1`、`scripts/session_bundle_portability_smoke.ps1` 和 `scripts/taxonomy_audit.ps1` 生成 evidence report。
3. 验证 `SessionBundle` 跨机器 profile portability。
4. 补齐 CAPTCHA/SMS/Email production manager wiring、provider acceptance 和 CDP detect/fill。
