# Release Performance Mitigation Plan（CANCELLED / diagnostic-only）

Updated: 2026-06-22 (Asia/Shanghai)

## 结论

2026-06-22 范围重置后，release performance 预算达标已取消。M5 release smoke、health gate、budget report 和 mitigation hints 只保留为本机启动诊断材料，不再作为 blocker、下一步或 green gate。

不要把 `passed_with_budget_overrun` 写成 performance green；也不要把 over budget 写成当前必须修复的问题。只有用户明确重新开启性能预算目标时，才按本文件重建优化计划。

## 历史证据

最新本机诊断 report：

- Release smoke：`data/reports/release-smoke/release-performance-smoke-1781933571814.json`
- M5 health gate：`data/reports/m5-release-health/m5-release-health-gate-1781941776163.json`
- Gate status：`passed_with_budget_overrun`

| 指标 | 历史目标 | 最新实测 | 当前口径 |
| --- | ---: | ---: | --- |
| cold start | `<= 2000ms` (`<= 2200ms` drift) | `3899ms` | diagnostic-only |
| idle RSS | `<= 220MB` (`<= 242MB` drift) | `433MB` | diagnostic-only |
| process count | `<= 4` (`<= 5` drift) | `9` | diagnostic-only |

## 只在诊断时参考的检查顺序

1. 确认 smoke 目标 exe：优先测 Tauri GUI exe `personal-pilot-tauri.exe`，避免测到立即退出的辅助 exe。
2. 拆分 process tree：记录 Tauri 主进程、WebView2 子进程、sidecar/辅助进程数量。
3. 确认 single-instance：启动前清理旧实例，启动后确认没有重复 PersonaPilot/WebView2 残留。
4. 检查首屏数据加载：避免启动即拉取大列表、历史报告或日志全量数据。
5. 检查日志和 report 初始化：日志分页/增量，validation history 不应启动即全量读取。
6. 如需复测，只把结果写成诊断数据，不写成 release performance gate。

## 当前退出条件

无。该计划已取消为目标。
