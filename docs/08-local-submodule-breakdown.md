# 08 Local Submodule Breakdown

Updated: 2026-04-15 (Asia/Shanghai)

## Goal

在一级模块已经切开的前提下，把每个一级模块继续拆成可直接并行开发的二级子模块、文件边界和共享契约。

这一版拆解的目的不是讨论概念，而是让不同 lane 可以直接开始实现，不必再重新切边界。

## Global Rules

所有二级子模块继续遵守：

1. `pages/components -> features/hooks/store -> services/desktop.ts -> tauri`
2. UI 不直接 `invoke`
3. 共享类型先落 `src/types/desktop.ts`
4. 桌面命令先落 `src/services/desktop.ts`
5. Tauri 命令与状态落 `src-tauri/src/*`
6. 大表默认虚拟滚动，日志默认分页，搜索默认防抖

## Module Breakdown

### M1 Dashboard

| Submodule | Scope | File Ownership | Dependency |
| --- | --- | --- | --- |
| D-01 Runtime Health | 运行时状态、启动、停止、健康展示 | `src/pages/DashboardPage.tsx`, `src/pages/OverviewPage.tsx`, `src/features/runtime/*` | existing |
| D-02 Queue KPIs | 队列概览、成功/失败计数、最近任务摘要 | `src/features/status/*` | existing |
| D-03 Quick Actions | 打开目录、跳转模块、后续快捷入口 | `src/components/*`, `src/pages/DashboardPage.tsx` | M0 |

### M2 Profiles Workbench

| Submodule | Scope | File Ownership | Dependency |
| --- | --- | --- | --- |
| P-01 Toolbar | `New / Quick Create / Import / Batch Edit / Start / Stop / Open / Sync / Record Template` | `src/features/profiles/*`, `src/components/profiles/*` | M8 |
| P-02 Filter Rail | `Group / Tag / Status / Platform` 左侧筛选 | `src/features/profiles/*`, `src/components/profiles/*` | M8 |
| P-03 Profiles Table | 高密度列表与列显示控制 | `src/features/profiles/*`, `src/components/profiles/*` | M8 |
| P-04 Selection And Batch Actions | 多选、批量动作反馈、状态回写 | `src/features/profiles/*` | P-03 |
| P-05 Details Drawer | `Overview / Proxy / Platform / Fingerprint / Advanced / Runtime / Logs` | `src/features/profiles/*`, `src/components/profiles/*` | M8 |
| P-06 Create/Edit Wizard | `General / Proxy / Platform / Fingerprint / Advanced` | `src/features/profiles/*`, `src/components/profiles/*` | P-05 |

### M3 Proxy Center

| Submodule | Scope | File Ownership | Dependency |
| --- | --- | --- | --- |
| X-01 Proxy Table | 代理列表、状态、标签、来源、出口 IP / 地区 | `src/features/proxies/*`, `src/components/proxies/*` | M8 |
| X-02 Batch Check | 批量检测代理健康、出口 IP、地区信息 | `src/features/proxies/*` | X-01 |
| X-03 Usage Mapping | 某代理被哪些 profile 使用 | `src/features/proxies/*` | M8 |
| X-04 Change-IP Actions | 支持时暴露变更 IP 动作 | `src/features/proxies/*` | M8 |
| X-05 Filtering | 按状态、地区、来源、标签过滤 | `src/features/proxies/*`, `src/components/proxies/*` | X-01 |

### M4 Automation

| Submodule | Scope | File Ownership | Dependency |
| --- | --- | --- | --- |
| A-01 Runs Board | 当前 runs 列表、分页、搜索、防抖 | `src/pages/AutomationPage.tsx`, `src/pages/TasksPage.tsx`, `src/features/tasks/*` | existing |
| A-02 Templates Board | 模板列表、状态、更新时间、适用范围 | `src/features/automation/*`, `src/features/templates/*` | M8 |
| A-03 Run Launcher | 选择模板、绑定 profiles、发起执行 | `src/features/automation/*` | A-02, M8 |
| A-04 Run Detail | 单次运行步骤回放、失败点、结果摘要 | `src/features/automation/*` | A-01, M8 |
| A-05 Local API Surface | 后续本地 API 管理入口 | `src/features/automation/*` | later |

### M5 Recorder + Templates

| Submodule | Scope | File Ownership | Dependency |
| --- | --- | --- | --- |
| R-01 Recorder Session | 开始录制、停止录制、录制状态机 | `src/features/recorder/*` | M8 |
| R-02 Step Capture | 捕获 `visit / click / input / select / scroll / wait / tab` | `src/features/recorder/*` | R-01 |
| R-03 Step Timeline | 步骤列表、排序、删除、编辑 | `src/features/recorder/*`, `src/components/automation/*` | R-02 |
| R-04 Variable Extraction | 把输入值抽为变量占位符 | `src/features/templates/*`, `src/features/recorder/*` | R-03 |
| R-05 Sensitive Data Guard | 不持久化密码、cookie 原文、2FA、明文 token | `src/features/recorder/*`, `src/features/templates/*` | R-04 |
| T-01 Template Metadata | 模板名、标签、适用平台、说明 | `src/features/templates/*` | M8 |
| T-02 Run Bindings | 模板与 profiles、变量值、批量目标绑定 | `src/features/templates/*` | T-01 |
| T-03 Template Compiler | 把模板编译成执行请求 | `src/features/templates/*`, `src/features/automation/*` | T-02 |

### M6 Synchronizer

| Submodule | Scope | File Ownership | Dependency |
| --- | --- | --- | --- |
| S-01 Window Matrix | 当前已打开窗口矩阵 | `src/features/synchronizer/*`, `src/components/synchronizer/*` | M8 |
| S-02 Main Window | 主窗口设定与切换 | `src/features/synchronizer/*` | S-01 |
| S-03 Layout Controls | `Grid / Overlap / Uniform Size` 布局动作 | `src/features/synchronizer/*`, `src/components/synchronizer/*` | S-01, M8 |
| S-04 Focus Actions | 聚焦指定窗口、跳转到 profile 窗口 | `src/features/synchronizer/*` | S-01, M8 |
| S-05 Action Feedback | 布局/聚焦结果反馈，接到 Action Logs | `src/features/synchronizer/*`, `src/features/logs/*` | S-03, S-04 |

### M7 Logs And Settings

| Submodule | Scope | File Ownership | Dependency |
| --- | --- | --- | --- |
| L-01 Runtime Logs | 运行日志列表、分页、过滤 | `src/features/logs/*`, `src/pages/LogsPage.tsx` | existing |
| L-02 Action Logs | profile / proxy / runtime / template / settings 动作日志 | `src/features/logs/*`, `src/pages/LogsPage.tsx` | M8 |
| G-01 Path Panels | 本地目录、打包目录、打开动作 | `src/features/settings/*`, `src/pages/SettingsPage.tsx` | existing |
| G-02 Runtime Settings Form | 浏览器路径、运行参数、调试开关 | `src/features/settings/*` | M8 |
| G-03 Restore Defaults | 恢复动作、错误提示、写回结果 | `src/features/settings/*` | G-02 |
| G-04 Local API Panel | 本地 API 与调试开关控制区 | `src/features/settings/*` | later |

### M8 Desktop Contracts

| Submodule | Scope | File Ownership | Dependency |
| --- | --- | --- | --- |
| C-01 Read Models | profile/proxy/template/window/log/settings 只读模型 | `src/types/desktop.ts`, `src/services/desktop.ts`, `src-tauri/src/*` | shared |
| C-02 Write Commands | start/stop/open/check/save/run/layout 等动作命令 | `src/services/desktop.ts`, `src-tauri/src/*` | shared |
| C-03 Error Normalization | 所有命令统一错误结构 | `src/services/desktop.ts` | existing |
| C-04 Request Guards | stale-result、分页、搜索、防抖契约 | `src/features/*`, `src/services/desktop.ts` | shared |

## Recommended Parallel Lanes

| Lane | Primary Submodules | Notes |
| --- | --- | --- |
| Lane A | `P-01 ~ P-04` | Profiles 主工作台表层 |
| Lane B | `P-05 ~ P-06` | Profiles 详情抽屉与向导 |
| Lane C | `X-01 ~ X-05` | Proxy Center |
| Lane D | `A-01 ~ A-04` | Automation 主页面与运行中心 |
| Lane E | `R-01 ~ R-05`, `T-01 ~ T-03` | Recorder + Templates |
| Lane F | `S-01 ~ S-05` | Synchronizer |
| Lane G | `L-01 ~ L-02`, `G-01 ~ G-04` | Logs + Settings |
| Lane H | `C-01 ~ C-04` | 共享 Desktop Contracts |

## Merge Order

1. 共享 `C-01` read models 先行
2. `Profiles / Proxies / Automation / Synchronizer` 页面壳可并行
3. `Recorder + Templates` 依赖 `Automation` 页面壳稳定后接入
4. `Action Logs` 在 `Profiles / Proxies / Automation / Synchronizer` 动作命令稳定后接入

## Current Completion Signal

这轮二级拆分完成后的判断标准：

- 每个一级模块已经有明确二级子模块表
- 每个二级子模块已经有文件归属
- 并行 lane 不再依赖口头约定
- `Recorder` 与 `Templates` 已经从 `Automation` 中逻辑分离，但产品入口仍归 `Automation`
- 共享 Desktop Contracts 已经被单独拉出来，不再混入页面实现讨论
