# 07 Local Parallel Module Split

Updated: 2026-04-15 (Asia/Shanghai)

## Goal

把 `PersonaPilot` 从当前的本地桌面骨架，拆成可以并行推进的本地指纹浏览器工作台模块。

本轮拆分只面向：

- `Windows 11`
- `Tauri 2 + Vite + React + TypeScript`
- 单窗口、单实例
- 本地项目，不含任何远程 Ubuntu / 远程运维 / 云端控制面板范围

## Current Mapping

当前代码里的实际承载关系先固定为：

- `OverviewPage` 临时承载 `Dashboard`
- `TasksPage` 临时承载 `Automation -> Runs`
- `LogsPage` 继续承载 `Logs`
- `SettingsPage` 继续承载 `Settings`
- 新增独立一级入口：
  - `Profiles`
  - `Proxies`
  - `Synchronizer`

这意味着我们先把产品信息架构和文件入口切开，再分别往里填真实能力。

## Parallel Modules

| Module | Scope | Primary Ownership | Depends On | Parallel Status |
| --- | --- | --- | --- | --- |
| M0 Shell And IA | 一级导航、路由、标题、页面入口、模块边界冻结 | `src/app/*`, `src/components/AppShell.tsx`, `src/components/NavRail.tsx`, `src/hooks/useHashRoute.ts` | 无 | 本轮先完成 |
| M1 Dashboard | 本地运行态、队列概览、健康度、快速入口 | `src/pages/DashboardPage.tsx`, `src/pages/OverviewPage.tsx`, `src/features/status/*`, `src/features/runtime/*` | M0 | 可并行 |
| M2 Profiles Workbench | 列表、过滤、抽屉、批量动作、创建编辑向导 | `src/pages/ProfilesPage.tsx`, `src/features/profiles/*` | M0 | 可并行 |
| M3 Proxy Center | 代理列表、状态、批量检测、出口 IP、被谁使用 | `src/pages/ProxiesPage.tsx`, `src/features/proxies/*` | M0 | 可并行 |
| M4 Automation Runs | 运行记录、队列视图、批量执行、运行反馈 | `src/pages/AutomationPage.tsx`, `src/pages/TasksPage.tsx`, `src/features/tasks/*`, `src/features/automation/*` | M0 | 可并行 |
| M5 Recorder Templates | 行为录制、步骤时间线、变量抽取、模板保存与复用 | `src/features/recorder/*`, `src/features/templates/*` | M0, M4 | 可并行但依赖契约 |
| M6 Synchronizer | 窗口矩阵、主窗口、布局控制、聚焦动作 | `src/pages/SynchronizerPage.tsx`, `src/features/synchronizer/*` | M0 | 可并行 |
| M7 Logs And Settings | Runtime Logs / Action Logs、本地设置写入、恢复与开关 | `src/pages/LogsPage.tsx`, `src/pages/SettingsPage.tsx`, `src/features/logs/*`, `src/features/settings/*` | M0 | 可并行 |
| M8 Desktop Contracts | `desktop.ts`、类型、Tauri 命令、只读/动作契约 | `src/services/desktop.ts`, `src/types/desktop.ts`, `src-tauri/src/*`, `src/desktop/*` | M0 | 共享底座 |

## Ownership Rules

为了支持同时推进，先冻结这些规则：

1. `M0` 之外的模块，不改一级导航、路由协议和页面命名。
2. 所有桌面能力继续只从 `src/services/desktop.ts` 暴露。
3. 页面不允许直接 `invoke`。
4. 模块新增状态，必须放进各自 `src/features/<domain>/`，不要回流到 `pages/`。
5. `M8 Desktop Contracts` 是共享底座，任何模块新增本地动作都要先补类型，再补 `desktop.ts`，再补 Tauri 命令。
6. `Logs` 与 `Settings` 只做本地范围，不带入任何云端、远程、团队协作能力。

## Recommended Concurrent Lanes

推荐并发推进方式如下：

| Lane | Recommended Scope | File Boundary |
| --- | --- | --- |
| Lane A | Shell polish + Dashboard | `src/app/*`, `src/components/*`, `src/pages/DashboardPage.tsx`, `src/pages/OverviewPage.tsx` |
| Lane B | Profiles workbench | `src/pages/ProfilesPage.tsx`, `src/features/profiles/*`, `src/components/profiles/*` |
| Lane C | Proxy center | `src/pages/ProxiesPage.tsx`, `src/features/proxies/*`, `src/components/proxies/*` |
| Lane D | Automation runs + recorder templates | `src/pages/AutomationPage.tsx`, `src/features/automation/*`, `src/features/recorder/*`, `src/features/templates/*` |
| Lane E | Synchronizer | `src/pages/SynchronizerPage.tsx`, `src/features/synchronizer/*`, `src/components/synchronizer/*` |
| Lane F | Logs + Settings enhancement | `src/pages/LogsPage.tsx`, `src/pages/SettingsPage.tsx`, `src/features/logs/*`, `src/features/settings/*` |
| Lane G | Desktop contracts | `src/services/desktop.ts`, `src/types/desktop.ts`, `src-tauri/src/*`, `src/desktop/*` |

## Dependency Order

真正的依赖顺序固定为：

1. 先完成 `M0 Shell And IA`
2. 同时打开 `M2 / M3 / M4 / M6 / M7`
3. `M8 Desktop Contracts` 按模块需求穿插提供只读模型和动作契约
4. `M5 Recorder Templates` 在 `Automation` 页面稳定后并入

## Recorder Template Boundary

“录制行为作为模板”这块单独冻结边界如下：

- 不直接塞进 `Profiles` 详情逻辑里
- 一级归属放在 `Automation`
- `Profiles` 页只保留 `Record Template` 快捷入口
- 数据结构拆成三层：
  - `Template metadata`
  - `Recorded steps`
  - `Run bindings`
- 默认不持久化敏感信息：
  - 密码
  - Cookie 原文
  - 2FA
  - 明文 token

## First Executable Slice

在本轮模块拆分完成后，建议先并行推进这一批：

- `Profiles` 主工作台骨架
- `Proxies` 主列表骨架
- `Automation` 下的 `Templates / Recorder` 骨架
- `Synchronizer` 可视化控制台骨架
- `Logs` 的 `Runtime Logs / Action Logs` 双视角切分

## Completion Signal

模块拆分完成的判断标准：

- 一级导航已经切成 `Dashboard / Profiles / Proxies / Automation / Synchronizer / Logs / Settings`
- 各模块已经有独立页面入口
- 现有 `Dashboard / Runs / Logs / Settings` 能继续工作
- 并行推进时不需要先重写整套壳层
- 后续模块新增动作仍遵守 `pages -> features -> services/desktop.ts -> tauri`
