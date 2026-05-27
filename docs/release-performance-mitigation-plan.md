# Release Performance Mitigation Plan

Updated: 2026-05-27 (Asia/Shanghai)

## 当前证据

P14 release artifact smoke 已通过 `scripts/release_performance_smoke.ps1` 生成真实 report，但结果是 `warning`：

| 指标 | 目标 | 当前实测 | 状态 |
| --- | ---: | ---: | --- |
| cold start | `<= 2000ms` | `10274ms` | 超预算 |
| idle RSS | `<= 220MB` | `672MB` | 超预算 |
| process count | `<= 4` | `22` | 超预算 |

该结果不能写成 release performance green。它只能证明 release artifact 可被测量，并暴露了性能债。

## 先做的优化顺序

1. 确认 smoke 目标 exe：优先测 Tauri GUI exe `personal-pilot-tauri.exe`，避免测到立即退出的辅助 exe。
2. 拆分 process tree：记录 Tauri 主进程、WebView2 子进程、辅助进程数量，确认 `14` 个进程是否来自 WebView2 多进程模型或重复实例。
3. 确认 single-instance：启动前清理旧实例，启动后确认没有重复 PersonaPilot/WebView2 残留。
4. 优化首屏数据加载：避免启动即拉取大列表、历史报告或日志全量数据。
5. 优化日志和 report 初始化：日志必须分页/增量，validation history 不应启动即全量读取。
6. 复测 release artifact，不使用 dev-mode 指标替代。

## 退出条件

- `scripts/release_performance_smoke.ps1` 生成新的 report。
- cold start、RSS、process count 至少有一项实质改善，并记录剩余超预算原因。
- 若无法达到默认预算，必须在 `docs/02-current-state.md` 和 `docs/24-external-distribution-readiness.md` 保留例外说明。
