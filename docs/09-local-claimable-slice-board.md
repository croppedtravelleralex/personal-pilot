# 09 Local Claimable Slice Board

Updated: 2026-04-15 (Asia/Shanghai)

## Goal

把已经完成的一级模块、二级子模块，继续拆成可被单个 Codex 对话直接认领的最小切片。

每个切片默认要求：

- 能在 `0.5 ~ 1.5` 天内完成
- 文件边界尽量单一
- 不要求先重构全局
- 不跨越多个业务域
- 验收标准明确

## Slice Rules

1. 一个 Codex 对话默认只认领 `1` 个 slice。
2. 如果两个 slice 强耦合，最多只允许同一对话认领 `2` 个相邻 slice。
3. 共享契约 slice 优先独立认领，不和 UI slice 混领。
4. 没有明确文件边界的工作，不允许直接认领。

## Claimable Slices

### Dashboard

| Slice ID | Scope | File Boundary | Acceptance |
| --- | --- | --- | --- |
| `D-01A` | Runtime 状态卡片整理 | `src/pages/DashboardPage.tsx`, `src/pages/OverviewPage.tsx` | 页面不改契约，仅优化 Dashboard 呈现 |
| `D-01B` | Runtime 操作按钮反馈态 | `src/features/runtime/*`, `src/pages/OverviewPage.tsx` | 启停/刷新反馈清晰且不破坏现有能力 |
| `D-02A` | Queue KPI 区块整理 | `src/features/status/*`, `src/pages/OverviewPage.tsx` | 指标区块结构清晰、类型不回退 |

### Profiles

| Slice ID | Scope | File Boundary | Acceptance |
| --- | --- | --- | --- |
| `P-01A` | ProfilesToolbar 骨架 | `src/components/profiles/*`, `src/pages/ProfilesPage.tsx` | 工具条静态结构落地 |
| `P-02A` | FilterRail 骨架 | `src/components/profiles/*`, `src/features/profiles/*` | `Group / Tag / Status / Platform` 筛选面板落地 |
| `P-03A` | ProfilesTable 骨架 | `src/components/profiles/*`, `src/features/profiles/*` | 高密度表格骨架落地 |
| `P-03B` | 列配置与行选择状态 | `src/features/profiles/*` | 选择态和列展示状态可用 |
| `P-04A` | BatchBar 动作条 | `src/components/profiles/*`, `src/features/profiles/*` | 多选后出现批量条 |
| `P-05A` | Drawer 外壳 | `src/components/profiles/*`, `src/features/profiles/*` | 右侧抽屉和 tab 外壳落地 |
| `P-06A` | Wizard 外壳 | `src/components/profiles/*`, `src/features/profiles/*` | `General / Proxy / Platform / Fingerprint / Advanced` 步骤骨架落地 |

### Proxies

| Slice ID | Scope | File Boundary | Acceptance |
| --- | --- | --- | --- |
| `X-01A` | ProxyTable 骨架 | `src/components/proxies/*`, `src/features/proxies/*`, `src/pages/ProxiesPage.tsx` | 列表骨架落地 |
| `X-02A` | BatchCheck 工具条 | `src/components/proxies/*`, `src/features/proxies/*` | 批量检测入口落地 |
| `X-03A` | UsagePanel 骨架 | `src/components/proxies/*`, `src/features/proxies/*` | 被哪些 profile 使用的侧栏或面板落地 |
| `X-05A` | ProxyFilterBar | `src/components/proxies/*`, `src/features/proxies/*` | 按状态、地区、标签过滤入口落地 |

### Automation

| Slice ID | Scope | File Boundary | Acceptance |
| --- | --- | --- | --- |
| `A-01A` | RunsBoard 标题与布局升级 | `src/pages/AutomationPage.tsx`, `src/pages/TasksPage.tsx` | 原 runs board 不回退、进入 Automation 风格 |
| `A-02A` | TemplatesBoard 骨架 | `src/components/automation/*`, `src/features/automation/*`, `src/features/templates/*` | 模板列表骨架落地 |
| `A-03A` | RunLauncher 骨架 | `src/components/automation/*`, `src/features/automation/*` | 模板运行发起面板落地 |
| `A-04A` | RunDetailPanel 骨架 | `src/components/automation/*`, `src/features/automation/*` | 单次运行详情侧栏落地 |

### Recorder + Templates

| Slice ID | Scope | File Boundary | Acceptance |
| --- | --- | --- | --- |
| `R-01A` | Recorder session store 骨架 | `src/features/recorder/*` | 录制状态机基础态定义完成 |
| `R-02A` | Step action model | `src/features/recorder/*`, `src/features/templates/*` | `visit / click / input / select / scroll / wait / tab` 类型模型落地 |
| `R-03A` | RecorderTimeline 骨架 | `src/components/automation/*`, `src/features/recorder/*` | 步骤时间线 UI 骨架落地 |
| `R-04A` | Variable extraction model | `src/features/templates/*` | 变量占位符模型和绑定结构落地 |
| `R-05A` | SensitiveDataGuard 规则模型 | `src/features/recorder/*`, `src/features/templates/*` | 敏感字段不持久化规则结构落地 |
| `T-01A` | Template metadata store | `src/features/templates/*` | 模板元数据状态和类型落地 |
| `T-02A` | Run bindings store | `src/features/templates/*` | 模板与 profiles 绑定结构落地 |
| `T-03A` | Template compiler interface | `src/features/templates/*`, `src/features/automation/*` | 编译接口和请求结构落地 |

### Synchronizer

| Slice ID | Scope | File Boundary | Acceptance |
| --- | --- | --- | --- |
| `S-01A` | WindowMatrix 骨架 | `src/components/synchronizer/*`, `src/features/synchronizer/*`, `src/pages/SynchronizerPage.tsx` | 窗口矩阵骨架落地 |
| `S-02A` | MainWindow 状态 | `src/features/synchronizer/*` | 主窗口选择状态模型落地 |
| `S-03A` | LayoutToolbar 骨架 | `src/components/synchronizer/*`, `src/features/synchronizer/*` | `Grid / Overlap / Uniform Size` 入口落地 |
| `S-04A` | Focus action state | `src/features/synchronizer/*` | 聚焦动作状态模型落地 |

### Logs + Settings

| Slice ID | Scope | File Boundary | Acceptance |
| --- | --- | --- | --- |
| `L-02A` | Action Logs 标签页骨架 | `src/pages/LogsPage.tsx`, `src/features/logs/*` | Runtime / Action 双视角入口出现 |
| `G-02A` | Runtime settings form 骨架 | `src/pages/SettingsPage.tsx`, `src/features/settings/*` | 浏览器路径、参数、调试开关表单骨架落地 |
| `G-03A` | Restore defaults action shell | `src/features/settings/*`, `src/pages/SettingsPage.tsx` | 恢复按钮与反馈框架落地 |

### Desktop Contracts

| Slice ID | Scope | File Boundary | Acceptance |
| --- | --- | --- | --- |
| `C-01A` | Profiles read model types | `src/types/desktop.ts`, `src/services/desktop.ts` | Profile 列表与详情类型落地 |
| `C-01B` | Proxies read model types | `src/types/desktop.ts`, `src/services/desktop.ts` | Proxy 列表与健康类型落地 |
| `C-01C` | Templates / recorder read model types | `src/types/desktop.ts`, `src/services/desktop.ts` | 模板与录制器类型落地 |
| `C-01D` | Synchronizer read model types | `src/types/desktop.ts`, `src/services/desktop.ts` | 窗口矩阵与布局类型落地 |
| `C-02A` | Profiles write commands | `src/services/desktop.ts`, `src-tauri/src/*` | start/stop/open/sync/check command wrapper 落地 |
| `C-02B` | Proxy write commands | `src/services/desktop.ts`, `src-tauri/src/*` | proxy check / change-ip command wrapper 落地 |
| `C-02C` | Template / recorder write commands | `src/services/desktop.ts`, `src-tauri/src/*` | save/run/record command wrapper 落地 |
| `C-02D` | Synchronizer write commands | `src/services/desktop.ts`, `src-tauri/src/*` | main window / layout / focus command wrapper 落地 |

## Recommended Claim Order

1. `C-01A ~ C-01D`
2. `P-01A / P-02A / P-03A`
3. `X-01A / X-02A`
4. `A-02A / A-03A / R-03A`
5. `S-01A / S-03A`
6. `L-02A / G-02A`

## Do Not Bundle Together

以下组合默认不要放到同一个 Codex 对话里：

- `Profiles UI slice` + `Desktop contracts slice`
- `Proxy UI slice` + `Recorder slice`
- `Settings form slice` + `Synchronizer slice`
- `模板编译 slice` + `运行详情 slice`

## Completion Signal

切片化完成后的判断标准：

- 每个 lane 已经可以认领具体 `Slice ID`
- 每个 `Slice ID` 都有文件边界
- 每个 `Slice ID` 都有最小验收标准
- 后续给 Codex 开新对话时，不必再重新解释整仓库背景
