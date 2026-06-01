# Release Performance Mitigation Plan

Updated: 2026-06-01 (Asia/Shanghai)

## 当前证据

M5 已把 `scripts/release_performance_smoke.ps1` 升级为 v2：报告继续保留旧 `measured*` 字段，同时新增 `budgetStatus`、`budgetResults`、`healthSummary`、`mitigationHints` 和 10% drift reason 边界。最新本机 report 仍是 `warning`：

| 指标 | 目标 | 当前实测 | 状态 |
| --- | ---: | ---: | --- |
| cold start | `<= 2000ms` (`<= 2200ms` drift) | `941ms` | 通过 |
| idle RSS | `<= 220MB` (`<= 242MB` drift) | `467MB` | 超预算 |
| process count | `<= 4` (`<= 5` drift) | `9` | 超预算 |

最新证据：

- Release smoke：`data/reports/release-smoke/release-performance-smoke-1780323308586.json`
- M5 health gate：`data/reports/m5-release-health/m5-release-health-gate-1780323678612.json`
- Gate status：`passed_with_budget_overrun`

该结果不能写成 release performance green。它只能证明 release artifact 可被测量、health schema 可被 gate 检查，并暴露了性能债。

## 先做的优化顺序

1. 确认 smoke 目标 exe：优先测 Tauri GUI exe `personal-pilot-tauri.exe`，避免测到立即退出的辅助 exe。
2. 拆分 process tree：记录 Tauri 主进程、WebView2 子进程、辅助进程数量；当前最新 clean breakdown 是 WebView2 `6` 个进程 / `367MB` RSS、`personal-pilot-core` `1` 个进程 / `23MB` RSS、Tauri 主进程 `67MB` RSS。
3. 确认 single-instance：启动前清理旧实例，启动后确认没有重复 PersonaPilot/WebView2 残留。
4. 优化首屏数据加载：避免启动即拉取大列表、历史报告或日志全量数据。
5. 优化日志和 report 初始化：日志必须分页/增量，validation history 不应启动即全量读取。
6. 复测 release artifact，不使用 dev-mode 指标替代。

## 退出条件

- `scripts/release_performance_smoke.ps1` 生成 v2 report。
- `scripts/m5_release_health_gate.ps1` 通过，状态可为 `passed_with_budget_overrun`，但不能被写成 performance green。
- cold start、RSS、process count 至少有一项实质改善，并记录剩余超预算原因。
- 若无法达到默认预算，必须在 `docs/02-current-state.md` 和 `docs/24-external-distribution-readiness.md` 保留例外说明。
