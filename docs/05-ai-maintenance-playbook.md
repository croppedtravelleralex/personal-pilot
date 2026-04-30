# personal-pilot AI 维护接手手册

本文件定义后续 AI 接手 personal-pilot 时的阅读顺序、更新纪律和判断规则。

## AI 接手阅读顺序

1. 先读 [README.md](./README.md)
2. 再读 [01-project-charter.md](./01-project-charter.md)
3. 再读 [02-current-state.md](./02-current-state.md)
4. 再读 [03-roadmap.md](./03-roadmap.md)
5. 需要时读 [04-improvement-backlog.md](./04-improvement-backlog.md)
6. 涉及行为录制、回放、养号、自然语言任务时读 [06-behavior-recording-technical-guidance.md](./06-behavior-recording-technical-guidance.md)
7. 涉及实例工作台、可视化管理、API 操作、任务队列、窗口激活/排列时读 [07-instance-workbench-technical-guidance.md](./07-instance-workbench-technical-guidance.md)
8. 需要历史上下文时读 `logs/YYYY/YYYY-MM.md`
9. 最后再进入相关代码、配置和命令验证

## 更新纪律

- 新事实先更新 [02-current-state.md](./02-current-state.md)
- 新路线或优先级变化更新 [03-roadmap.md](./03-roadmap.md)
- 新改进建议、风险或技术债更新 [04-improvement-backlog.md](./04-improvement-backlog.md)
- 行为录制专项变化同步更新 [06-behavior-recording-technical-guidance.md](./06-behavior-recording-technical-guidance.md)
- 实例工作台专项变化同步更新 [07-instance-workbench-technical-guidance.md](./07-instance-workbench-technical-guidance.md)
- 每轮实际工作都要追加当月日志

## 信息来源优先级

1. 代码、配置、数据库、测试和命令结果
2. 当前状态主档
3. 路线图
4. 月度日志
5. 根目录 README

## 行为录制专项接手规则

- 行为录制 P0/P1/P2 和 P3 基础性能/产品工具切片已在 2026-04-29 落地；后续不要重复做大范围基础重构，优先围绕真实业务页回归、target discovery 兜底提示和发布前性能基线收口。
- 2026-04-29 P2/P3 收口核验结论：错误码、cleanup POST、前端 API 收口、事件释放、active recording 状态统一、list/detail 拆分、搜索 debounce、详情分页、敏感输入脱敏、裁剪、导入导出、复制模板、回放进度、失败重试和真实浏览器基础 E2E 均可标 Done。
- 2026-04-29 剩余问题收口结论：same-page iframe 聚合与可连接 OOPIF/CDP iframe target 同步已完成；新 tab/多 page target 已改为浏览器级 `Target.targetCreated/targetInfoChanged` 事件驱动同步，并保留 100ms 轮询兜底；manifest/index 已完成，detail page 已走索引；EventMonitor 表格虚拟滚动、BrowserLogs 分页增量加载和 Table 虚拟滚动 warning 已消除。后续 AI 不应再把这些写成未完成主线，但必须保留 target discovery 兜底提示和真实业务页回归边界。
- 真实浏览器验收只有在实际启动浏览器并跑完录制/回放后才能写通过；本轮可引用 `PERSONAL_PILOT_RECORDING_E2E=1 go test -run TestRealFingerprintBrowserRecordPlayback`，不要用 `TestAutoRecordNurturing` 的 SKIP 当通过证据。
- 每个阶段必须有对应验证：行为包测试、后端全量测试、前端构建，涉及真实交互时补真实浏览器录制回放验收；本轮最新 `go test -count=1 ./backend/...` 已通过，后续若代码再变更必须重新跑，不得沿用旧结果。
- 涉及敏感输入、验证码、密码、账号数据的录制策略，默认保守处理；未获得明确确认前不要明文保存。
- 修改录制/回放事件语义时，同步更新后端类型、前端类型、详情展示和测试。
- 当前仓库已补齐根目录 Tauri 2 + Vite/React/TypeScript 基线入口，Win11 Tauri 模板检查最新已通过；存量 Wails/Go 模块仍作为迁移期历史代码存在，后续不要把旧失败口径当作当前事实。

## 实例工作台专项接手规则

- 主线是非容器化：浏览器保持外部真实顶层窗口，软件内做可视化、API 操作和任务编排。
- 不要恢复已移除的旧实验页或 headless screencast 主体验；L0 主线只管理外部真实 Chromium。
- 工作台任务必须优先复用现有实例、profile、debugPort 和 CDP 能力，不改指纹启动参数。
- 外部窗口激活/排列只作为 Windows-only 辅助能力；按 PID 查找真实顶层窗口，不做 re-parent，不改变启动参数。
- 超过 200 条实例列表必须继续走虚拟滚动，搜索默认 250-400ms debounce。
- 任务队列最近 200 条由后端持久化；新增任务字段时同步 Go 类型、前端类型和 Wails 绑定。
- 窗口激活/排列失败不得阻断导航、刷新、截图等 CDP API 操作。

## 如何记录不确定项

- 已确认：有代码、配置、测试或命令结果支撑
- 待确认：只有局部证据，仍需继续核验
- 已过时：历史上成立，但已被现状覆盖

## 月度日志写法

每次追加：

- 日期
- 目标
- 观察到的事实
- 已完成
- 未完成
- 下一步
- 关联文件/命令
## 2026-04-29 实例工作台验收接手规则

- 做实例工作台回归时，优先运行 `scripts/verify-workbench-two-instances.ps1`，它会用本地 LaunchServer 和 workbench HTTP API 覆盖 2 实例真实启动、导航、刷新、截图、排列、停止隔离和清理。
- 脚本默认只选未运行 profile，避免误停用户已有浏览器；需要验收已运行实例时，显式传 `-ProfileIds <id1>,<id2> -StopPreExisting`。
- 若脚本失败，先看失败阶段：health/profile 代表 App 或实例数据问题；launch/debugReady 代表浏览器核心/profile 启动问题；workbench/screenshot/arrange 代表 CDP 或 Win32 窗口辅助层问题；cleanup stop 代表受控停止链路问题。

## 2026-04-30 persona-pilot selective asset merge 接手规则

- 后续不要整仓合并 `D:\SelfMadeTool\persona-pilot`。该 sibling repo 只能作为样板库使用。
- 已吸收的通用资产记录在 [10-persona-pilot-valuable-assets.md](./10-persona-pilot-valuable-assets.md)。
- 新工作台页面优先复用 `src/components/workbench/*`，能力边界提示优先用 `TruthBoundaryBanner`，长文本预览优先用 `InlineContentPreview`。
- 新业务域优先在 `src/features/<domain>/` 下采用 `model/adapters/store/selectors/hooks` 结构。
- 搜索/筛选/排序的异步请求必须使用 request id 或 abort 策略；可直接用 `createRequestGate()`。
- React 19、pnpm、TypeScript 6、Vite 8、Rust 后端原型和内嵌浏览器 M1 路线不属于当前主线，除非另开设计决策。
