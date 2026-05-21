# STATUS.md

This root `STATUS.md` is a compatibility entrypoint.
Canonical status now lives in `/docs/02-current-state.md`.

## 当前真实标记

- 主线汇报基线：`100% / 0% / green`（Mainline P0 已全部闭环）
- 整体终态基线：`30% / 70% / yellow`
- `80` 个已声明控制不等同于 `80` 个运行时应用字段；当前运行时投影仍为 `12`
- 当前行为运行时仍为 `13` 个原语
- cookie/localStorage/sessionStorage 重启连续性已落地
- AdsPower 追赶、`50+`、`450+` 属于整体轨道
- 详细阶段计划和评分卡见 `/docs/19-phase-plan-and-scorecard.md`
- `运行时存活` 不等同于交付闭环

## Follow These Docs

1. `/docs/02-current-state.md`
2. `/docs/final-goal-progress-breakdown.md`
3. `/docs/19-phase-plan-and-scorecard.md`
4. `/docs/03-roadmap.md`
5. `/docs/04-improvement-backlog.md`
6. `/docs/17-full-app-audit-progress-reset.md`
7. `/docs/13-adspower-deep-comparison.md`
8. `/docs/18-external-browser-integration-plan.md`

## 2026-04-16 Mainline Delta

- `Tasks` 表面合并已完成
- `changeProxyIp` 在本地桌面合约层已支持 provider-aware / sticky-aware
- synchronizer 现在支持实时桌面读取 + 原生焦点，不支持的写入已明确降级为 staged
- recorder 现在支持桌面步骤写入
- Rust 门禁已全部绿色，包括 `integration_api` / `integration_lightpanda_runner`
- 路由级代码分割已清理旧的 Vite 块警告

## 2026-05-21 Mainline 闭环 (Multi-Agent)

- **Mainline P0 已全部闭环**：剩余 3 项（provider proxy API、synchronizer 原生写入、recorder/templates 降级闭环）全部完成
- `changeProxyIp` 从本地桩升级为真实 provider 级 HTTP POST/PUT/PATCH 轮换，含重试/冷却/回滚
- synchronizer 原生广播写入落地：物理 `SetWindowPos` 窗口排布，确定性排序
- recorder/templates native-first 降级闭环：desktop session 守卫，空状态模板选择修复
- engineering hygiene：SQLite 路径 env var 降级、CI workflow (cargo test+clippy+pnpm)、`.env` gitignore、删除 `package-lock.json`
