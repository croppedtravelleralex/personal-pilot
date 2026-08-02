# 06 Windows Local Solution

Updated: 2026-04-15 (Asia/Shanghai)

## 目标

把当前 `PersonaPilot` 仓库演进成一个只在本地 `Windows 11` 运行的桌面工具。
推荐终态：

- `Tauri 2`
- `Vite`
- `React`
- `TypeScript`
- 单窗口
- 单实例

## 设计原则

- 本地优先
- 保留当前 Rust 核心的可复用能力
- 前端只做 UI 和交互编排
- 所有桌面原生调用都走统一服务层
- 不引入额外 Node 后端服务
- 不做 Electron

## 推荐结构

```text
persona-pilot/
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
      main.rs
      commands/
      core/
    capabilities/
    tauri.conf.json
  data/
  scripts/
```

## Rust 与桌面壳的分工

### Rust 侧保留
- SQLite 初始化与迁移
- task / run / log / artifact 核心能力
- persona / platform / continuity 规则
- 浏览器任务执行
- 本地文件系统、进程调用、系统集成

### 前端侧负责
- 任务面板
- persona / profile / continuity 页面
- 日志与报告视图
- 本地设置页
- 交互编排和状态展示

## 强制调用链

```text
pages/components -> features/hooks/store -> services/desktop.ts -> tauri -> Rust core
```

禁止：

- 页面直接 `invoke`
- 页面直接碰文件系统或进程控制
- `services` 反向依赖 `pages/components/features`

## `desktop.ts` 责任

`src/services/desktop.ts` 必须成为唯一桌面调用出口。
至少要负责：

- `getAppState()`
- `runTask()`
- `listTasks()`
- `listLogs()`
- `readSettings()`
- `writeSettings()`
- `openDataDirectory()`
- `startLocalRuntime()`
- `stopLocalRuntime()`

所有返回值都需要明确 TypeScript 类型和统一错误结构。

## Windows 本地数据策略

开发态可以继续使用仓库内：

- `data/persona_pilot.db`
- `data/reports/`
- `data/logs/`

打包态建议迁移到：

- `%LOCALAPPDATA%\\PersonaPilot\\data\\`
- `%LOCALAPPDATA%\\PersonaPilot\\logs\\`
- `%LOCALAPPDATA%\\PersonaPilot\\reports\\`

## 页面建议

### 首页

- 本地运行状态
- 最近任务
- 最近错误
- 快速入口

### Tasks

- 任务列表
- 状态筛选
- 结果详情
- 分页 / 增量加载

### Personas

- persona 列表
- continuity 状态
- 最近事件
- 快照摘要

### Logs

- 分页日志
- 关键字搜索
- 错误等级筛选
- 虚拟滚动

### Settings

- 数据目录
- 浏览器路径
- 运行参数
- 本地调试开关

## 迁移建议

### Slice 1

- 固化本地文档入口
- 增加 PowerShell 验证入口

### Slice 2

- 梳理当前 Rust 核心的模块边界
- 识别未来 Tauri commands 列表

### Slice 3

- 建立 Tauri 基础壳
- 加入 `desktop.ts`

### Slice 4

- 先接入只读页面：
  - status
  - tasks
  - logs

### Slice 5

- 再接入写操作：
  - run task
  - edit settings
  - local runtime control

## 当前推荐下一步

1. 固化本地 PowerShell 验证入口
2. 为 Tauri 落地拆分命令面
3. 新建前端骨架与目录
4. 先做只读状态页，再做任务写操作
