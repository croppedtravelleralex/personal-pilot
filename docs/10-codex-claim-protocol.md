# 10 Codex Claim Protocol

Updated: 2026-04-15 (Asia/Shanghai)

## Goal

定义“给 Codex 认领一个 slice”时的标准对话方式。

目标不是写漂亮提示词，而是降低重复解释成本、避免多个 Codex 对话互相踩文件。

## Core Rules

1. 一个对话只认领一个 `Slice ID`，最多两个相邻 slice。
2. 每次认领都必须写明：
   - `Slice ID`
   - 目标
   - 允许修改的文件边界
   - 不允许修改的边界
   - 验收方式
3. 默认加上：
   - 只做本地 `Windows 11`
   - 不提交
   - 先直接执行
   - 过程中中文短汇报
4. 共享契约类 slice 单独认领，不和 UI slice 混合。

## Recommended Conversation Structure

每次开新对话，建议用户按这个顺序说：

1. 认领哪个 `Slice ID`
2. 本轮要交付什么
3. 允许改哪些路径
4. 不要碰哪些路径
5. 验收命令或验收口径
6. 是否允许顺手补文档

## Short Claim Template

适合你快速开工时直接发：

```text
你认领 `P-03A`。
目标：把 ProfilesTable 骨架做出来，保持本地 Win11 规则，不要提交。
允许修改：
- src/components/profiles/*
- src/features/profiles/*
- src/pages/ProfilesPage.tsx
不要改：
- src/services/desktop.ts
- src-tauri/src/*
- 其他业务模块
验收：
- pnpm typecheck
- 页面骨架能挂到 Profiles 入口
先直接执行，过程中中文短汇报。
```

## Standard Claim Template

适合中等切片：

```text
你认领 `A-02A`。

本轮目标：
- 在 Automation 页面补 TemplatesBoard 骨架
- 不改 recorder 底层状态机
- 不引入新依赖

允许修改：
- src/components/automation/*
- src/features/automation/*
- src/features/templates/*
- src/pages/AutomationPage.tsx

禁止修改：
- src/services/desktop.ts
- src-tauri/src/*
- src/features/recorder/*
- 其他页面

验收标准：
- pnpm typecheck 通过
- Automation 页面出现 TemplatesBoard 骨架
- 不影响现有 runs board

附加要求：
- 只做本地 Windows 11
- 不提交
- 如果发现缺契约，只记录缺口，不顺手扩命令层
- 完成后汇报：改了什么、没做什么、下一刀建议是什么
```

## Contract Slice Template

适合共享底座切片：

```text
你认领 `C-01A`。

本轮目标：
- 为 Profiles 模块补 desktop read model types
- 补 services/desktop.ts 的 typed wrapper 占位
- 不做页面 UI

允许修改：
- src/types/desktop.ts
- src/services/desktop.ts
- src-tauri/src/*

禁止修改：
- src/pages/*
- src/components/*
- src/features/profiles/*

验收标准：
- pnpm typecheck 通过
- 类型命名清晰
- 没有页面直接 invoke

先直接执行，过程中中文短汇报，不要提交。
```

## Codex Expected Acknowledgement

理想情况下，Codex 的认领回复应类似这样：

```text
我认领 `P-03A`。
本轮只会动：
- src/components/profiles/*
- src/features/profiles/*
- src/pages/ProfilesPage.tsx

不会碰：
- src/services/desktop.ts
- src-tauri/src/*

我先补 ProfilesTable 骨架和最小状态接线，然后跑 typecheck，再给你汇报结果。
```

## Progress Update Template

Codex 过程中建议按这个口径汇报：

```text
我在推进 `P-03A`，当前已经把表格结构和列模型拆开。
下一步会接 selection state，但不会扩到 desktop contracts。
```

## Handoff Template

一个 slice 做完后，建议用这个格式交接：

```text
`P-03A` 已完成。

本轮改动：
- ProfilesTable 骨架
- 基础列定义
- 空状态与占位数据接线

未做：
- 批量动作
- 真实 desktop contracts 接线

建议下一刀：
- `P-03B` 列配置与行选择状态
或
- `C-01A` Profiles read model types
```

## Release Claim Template

如果一个对话不再继续某 slice，建议这样释放：

```text
我释放 `R-03A`。
当前只完成了 RecorderTimeline 的静态骨架，尚未接状态。
后续认领者请继续修改：
- src/components/automation/*
- src/features/recorder/*
不要重复重做现有时间线布局。
```

## Conflict Template

如果发现别人也在改相邻区域，建议这样回报：

```text
我在 `P-05A` 里发现 `src/features/profiles/*` 已有并行改动。
当前不适合继续扩 selection state。
建议把本对话收敛到 Drawer 外壳，不再触碰共享 store。
```

## Recommended User Verbs

你后面给 Codex 发任务时，建议优先用这些动词：

- `认领`
- `只做`
- `允许修改`
- `不要改`
- `验收`
- `完成后汇报`
- `不提交`

这几个词比“帮我看看”“顺手做一下”更稳定，能明显降低跑偏概率。

## Best Practice

最推荐的实际用法是：

1. 先从 [09-local-claimable-slice-board.md](D:/SelfMadeTool/persona-pilot/docs/09-local-claimable-slice-board.md) 选一个 `Slice ID`
2. 按本文件模板开一个新 Codex 对话
3. 一个对话只做一个 slice
4. 做完后在下一个对话认领下一个 slice

这样最适合并行推进，也最不容易互相踩文件。
