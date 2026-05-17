# 11 Codex Claim Templates Batch 01

Updated: 2026-04-15 (Asia/Shanghai)

## Goal

这一批模板用于同时开 `4` 个 Codex 对话并行推进。

设计原则：

- 每个任务量为“中等偏多”
- 每个对话只认领一个主模块
- 既不是过小碎片，也不跨越过多业务域
- 每个模板都明确允许修改范围、禁止范围、验收方式和汇报方式

## Template 1: Profiles Workbench

```text
你认领 `Profiles Workbench` 模块，本轮主任务覆盖：
- `P-01A` ProfilesToolbar 骨架
- `P-02A` FilterRail 骨架
- `P-03A` ProfilesTable 骨架
- `P-03B` 列配置与行选择状态
- `P-04A` BatchBar 动作条

本轮目标：
- 把 Profiles 页面从占位页推进成真正的工作台骨架
- 页面要有左侧筛选、顶部工具条、中间高密度表格、批量选择条
- 暂时不做真实 desktop contracts 接线，可以用本地静态 mock / placeholder state
- 结构要为后续 Drawer 和 Wizard 留接口，但本轮不要扩到 Drawer / Wizard

允许修改：
- src/pages/ProfilesPage.tsx
- src/components/profiles/*
- src/features/profiles/*
- src/app/styles.css
- 如有必要，可补少量通用展示组件，但不要重写整个 AppShell

禁止修改：
- src/services/desktop.ts
- src/types/desktop.ts
- src-tauri/src/*
- src/features/proxies/*
- src/features/automation/*
- src/features/recorder/*
- src/features/templates/*
- src/features/synchronizer/*

实现要求：
- 保持 Win11 本地桌面规则
- 不新增重量级依赖
- 大表格结构按后续虚拟滚动兼容方式组织
- 搜索和筛选状态放在 features/profiles，不要塞回页面 JSX
- 页面不能直接 invoke

验收标准：
- pnpm typecheck 通过
- Profiles 页面出现：
  - Toolbar
  - FilterRail
  - ProfilesTable
  - BatchBar
- 页面结构明显是 AdsPower 风格工作台，而不是普通空卡片页
- 不影响 Dashboard / Automation / Logs / Settings 现有可用性

汇报要求：
- 过程中中文短汇报
- 完成后按以下格式汇报：
  - 本轮完成了什么
  - 哪些数据仍是 mock / placeholder
  - 下一刀建议接 `P-05A` 还是 `C-01A`

不要提交，先直接执行。
```

## Template 2: Proxy Center

```text
你认领 `Proxy Center` 模块，本轮主任务覆盖：
- `X-01A` ProxyTable 骨架
- `X-02A` BatchCheck 工具条
- `X-03A` UsagePanel 骨架
- `X-05A` ProxyFilterBar

本轮目标：
- 把 Proxies 页面从占位页推进成真正的代理中心骨架
- 页面至少具备：
  - 顶部工具条
  - 过滤区
  - 代理列表
  - 右侧或下方 usage 信息区
- 先不做真实批量检测命令，只把状态模型、入口和反馈框架搭好
- UI 要适合后续接入出口 IP、地区、标签、来源、被哪些 profile 使用

允许修改：
- src/pages/ProxiesPage.tsx
- src/components/proxies/*
- src/features/proxies/*
- src/app/styles.css

禁止修改：
- src/services/desktop.ts
- src/types/desktop.ts
- src-tauri/src/*
- src/features/profiles/*
- src/features/automation/*
- src/features/recorder/*
- src/features/templates/*
- src/features/synchronizer/*

实现要求：
- 保持本地 Win11 桌面方案
- 不引入新依赖
- 代理列表结构要兼容后续状态色、批量检测结果和 usage mapping
- 过滤条件和表格状态放在 features/proxies
- 如果需要占位数据，写成清晰的本地 mock model，不要散落在 JSX

验收标准：
- pnpm typecheck 通过
- Proxies 页面出现：
  - ProxyFilterBar
  - BatchCheck Toolbar
  - ProxyTable
  - UsagePanel
- 页面结构清晰，可直接作为后续真实接线的壳层
- 不影响现有其他页面

汇报要求：
- 过程中中文短汇报
- 完成后说明：
  - 已完成的结构块
  - 哪些动作仍未接 desktop contracts
  - 下一刀建议接 `X-02B/C-01B` 哪个方向

不要提交，先直接执行。
```

## Template 3: Automation + Templates Board

```text
你认领 `Automation Center` 模块，本轮主任务覆盖：
- `A-01A` RunsBoard 标题与布局升级
- `A-02A` TemplatesBoard 骨架
- `A-03A` RunLauncher 骨架
- `A-04A` RunDetailPanel 骨架

本轮目标：
- 把当前 Automation 页面从“只是一块 runs board”升级成真正的自动化中心骨架
- 保留现有 runs board 可用
- 新增模板列表区、运行发起面板、运行详情面板
- 先不接 recorder 底层状态机，也不扩真实 template compiler
- 整体页面需要明显表现出：
  - runs
  - templates
  - launcher
  - detail
  这四块并存

允许修改：
- src/pages/AutomationPage.tsx
- src/pages/TasksPage.tsx
- src/components/automation/*
- src/features/automation/*
- src/features/templates/*
- src/app/styles.css

禁止修改：
- src/services/desktop.ts
- src/types/desktop.ts
- src-tauri/src/*
- src/features/recorder/*
- src/features/profiles/*
- src/features/proxies/*
- src/features/synchronizer/*

实现要求：
- 保留现有 TasksPage runs board 能力，不要回退
- Automation 页面要形成更强的信息架构，而不是继续只有单列表
- 状态组织要放在 features/automation 和 features/templates
- 不新增外部依赖
- 不要顺手去扩 desktop contracts，如果缺契约只留清晰占位

验收标准：
- pnpm typecheck 通过
- Automation 页面同时具备：
  - RunsBoard
  - TemplatesBoard
  - RunLauncher
  - RunDetailPanel
- 现有 runs board 不损坏
- 页面结构明显可容纳后续 recorder / template compiler 接入

汇报要求：
- 过程中中文短汇报
- 完成后说明：
  - 新增了哪几块 UI
  - 保留了哪些旧能力
  - 还缺哪些 contracts 才能进入真实运行

不要提交，先直接执行。
```

## Template 4: Desktop Contracts For Profiles / Proxies / Templates / Sync

```text
你认领 `Desktop Contracts` 模块，本轮主任务覆盖：
- `C-01A` Profiles read model types
- `C-01B` Proxies read model types
- `C-01C` Templates / recorder read model types
- `C-01D` Synchronizer read model types
- 视情况补少量对应 typed wrapper 占位

本轮目标：
- 先把 4 组核心 read model types 补齐
- 在 src/services/desktop.ts 中补 typed wrapper 占位或接口占位
- 不做页面 UI
- 不做复杂 Tauri 实现，如果 Rust 端真实命令暂时不存在，就先把类型和 wrapper 设计收敛好
- 输出的目标是让后续 4 个 UI 模块都能有统一的只读契约入口

允许修改：
- src/types/desktop.ts
- src/services/desktop.ts
- src-tauri/src/commands.rs
- src-tauri/src/state.rs
- src-tauri/src/lib.rs
- 如有必要，可补少量 src/desktop/* 只读模型映射

禁止修改：
- src/pages/*
- src/components/*
- src/features/profiles/*
- src/features/proxies/*
- src/features/automation/*
- src/features/recorder/*
- src/features/templates/*
- src/features/synchronizer/*
- src/app/styles.css

实现要求：
- 所有 native 调用继续只经 src/services/desktop.ts
- 类型命名清晰，不允许用 any 糊过去
- 错误结构继续统一走现有 DesktopServiceError 风格
- 若后端命令未就绪，可以先补明确的 TODO 风格 wrapper 占位，但不要伪造假实现
- 不要改现有已工作的 status / tasks / logs / settings 契约行为

验收标准：
- pnpm typecheck 通过
- 新增的 read model types 能覆盖：
  - profile row / detail
  - proxy row / health / usage
  - template metadata / recorder snapshot
  - sync window / layout state
- services/desktop.ts 中已有对应 typed 接口入口或清晰占位
- 没有页面直接 invoke，没有破坏现有 contracts

汇报要求：
- 过程中中文短汇报
- 完成后说明：
  - 新增了哪些 type
  - 哪些 wrapper 已补
  - 哪些 Tauri command 仍缺真实实现
  - 推荐下一个最适合接的 UI slice 是什么

不要提交，先直接执行。
```

## Recommended Launch Order

建议 4 个对话按这个顺序同时开：

1. `Profiles Workbench`
2. `Proxy Center`
3. `Automation + Templates Board`
4. `Desktop Contracts`

这样 UI 和契约层能并行推进，但不会全部卡在同一个文件边界上。
