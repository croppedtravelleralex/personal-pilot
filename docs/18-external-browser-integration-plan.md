# 外部指纹浏览器整合方案
Updated: 2026-05-27 (Asia/Shanghai)

## 0. 口径边界

- 当前主线交付：`100% / 0% / green`
- 当前整体终局：`40% / 60% / yellow`
- 本文属于 `整体终局` 轨道，主要服务于指纹真实性、运行时深度、AdsPower 追评和外部能力整合
- 本文不是当前 Mainline closeout 的完成证明；旧 `95% / 7%`、`30% / 70%` 只作为历史口径保留，方案落地后才会逐步反映到整体终局进度

详细阶段任务、评分口径、以及与 AdsPower 的统一汇总入口，见 `docs/19-phase-plan-and-scorecard.md`。

## 1. 研究范围与本地快照

本轮目标不是“直接合并外部浏览器代码”，而是把外部项目里真正高价值、可长期维护、且不破坏当前 `Win11 only + Tauri 2 + Vite + React + TypeScript + 单实例 + 轻 Rust` 基线的能力抽出来，落成 `persona-pilot` 可执行的整合方案。

本轮已完成：

- 建立本地研究目录：`research/external/`
- 7 个外部项目已落地到本地
- 完成 8 个子 agent 轮次研究
- 所有子 agent 已关闭

本地研究快照：

| 项目 | 本地目录 | 远程仓库 | 快照 |
| --- | --- | --- | --- |
| Donut Browser | `research/external/donutbrowser` | `https://github.com/zhom/donutbrowser.git` | `adb1335` |
| BotBrowser | `research/external/botbrowser` | `https://github.com/botswin/BotBrowser.git` | `aa35902` |
| Mullvad Browser | `research/external/mullvad-browser` | `https://github.com/mullvad/mullvad-browser.git` | `main=13cd0f4`, 源码取 `tag 15.0.2` |
| VirtualBrowser | `research/external/virtualbrowser` | `https://github.com/Virtual-Browser/VirtualBrowser.git` | `db322e1` |
| TheGP/untidetect-tools | `research/external/untidetect-tools` | `https://github.com/TheGP/untidetect-tools.git` | `09ce3b9` |
| Camoufox | `research/external/camoufox` | `https://github.com/daijro/camoufox.git` | `e4528a2` |
| FakeBrowser | `research/external/fakebrowser` | `https://github.com/samshine/FakeBrowser.git` | `c928bfe` |

说明：

- `Mullvad Browser` 的 `main` 分支只有说明页，真实源码要看特定 `tag/branch`，本轮以 `15.0.2` 为源码依据。
- 用户点名的 `FakeBrowser` 在 GitHub 上没有稳定、统一、可确认的单一官方公开仓库，本轮落地的是公开可获取、最接近 TLS/transport mimic 方向的 `samshine/FakeBrowser`，仅作为传输层研究代理，不代表“官方唯一源”。

## 2. 总结论

统一结论如下：

- `persona-pilot` 当前主线不该切向“浏览器 fork 主线化”，而应继续沿着现有桌面壳、控制面、数据面收口。
- 外部项目里最高 ROI 的资产不是“再造一个完整浏览器壳”，而是：
  - 验证与检测基线
  - 指纹字段与观测字典
  - profile / session bundle 合同
  - proxy / IP 一致性与泄漏验证
  - 少量 operator surface 产品设计
- 若未来要做自有内核或深改浏览器，应该开独立实验轨或独立仓库，不应直接并入当前主仓主线。

对 `persona-pilot` 的最重要判断：

- 当前真实缺口不在“再换一个更强内核”。
- 当前真实缺口在：
  - Proxy / IP 写入与一致性闭环
  - Synchronizer 原生批量写与广播闭环
  - Recorder / Templates 更深的原生闭环
  - Fingerprint observation / validation 深度不足

当前与整体终局直接相关的事实锚点：

- first-family 已声明 `80` 个核心控制字段
- 当前 runtime 已投影 `26` 个 env-backed 指纹字段（`25` 个 control-supported + derived `platform`）
- 当前行为运行时只有 `13` 个 shipped primitives
- cookie / localStorage / sessionStorage continuity 已支持关软件重启后恢复
- `450+` 指纹信号与 `450+` 事件类型仍是待落地的整体终局目标

## 3. 外部项目长处与可用性判断

| 项目 | 最强资产 | 对 `persona-pilot` 的建议 |
| --- | --- | --- |
| Donut Browser | `profile-first runtime`、local proxy、profile persistence、sync、REST/MCP 自动化 facade | 借 runtime 合同、profile 落盘、local API/MCP 思路；不借其重架构和商业内核耦合 |
| BotBrowser | per-context fingerprint、browser-level CDP 契约、验证矩阵、性能基准 | 强烈建议借方法论和验证资产；不建议假设其闭源内核能力可公开复刻 |
| Mullvad Browser | 薄定制、厚基线、默认隐私基线、release discipline | 只借原则层：归一化、可信默认、release 锁定、供应链纪律 |
| VirtualBrowser | 多 profile 控制面、批量操作、proxy 预处理链、双自动化入口 | 借 profile/group/import-export、`launch -> CDP attach` 思路；不借其 WebUI/Node 架构 |
| untidetect-tools | 生态能力地图、检测清单、代理与泄漏检查清单 | 直接转化为验证板和供应链地图 |
| Camoufox | Firefox 原生 patch、typed property registry、per-context manager、worker/process 自洽 | 借字段体系、coherence guardrail、隔离世界桥接思路；谨慎评估是否值得走 Firefox fork |
| FakeBrowser | 旧式 TLS / HTTP / SOCKS mimic 研究样例 | 只保留传输层观测点和实验器思路，不进主内核 |

## 4. 直接迁移、借鉴、排除

### 4.1 可直接进入主线

- 建立 `validation board`
  - 来源：`BotBrowser`、`untidetect-tools`
  - 内容：`CreepJS / BrowserLeaks / Pixelscan / WebRTC leak / DNS leak / bot detector / request fingerprint / canvas / audio / worker`
- 扩充现有 `fingerprint schema` 的字段覆盖
  - 来源：`Camoufox`、`VirtualBrowser`
  - 方式：先扩字段字典与 observation probes，再扩 runtime materialization，不倒置顺序
- 完善 `profile/session bundle` 合同
  - 来源：`Donut Browser`、`VirtualBrowser`
  - 内容：profile groups、import/export、cookie/extension metadata、stable profile directory
- 完善 `proxy/IP consistency` 视角
  - 来源：`BotBrowser`、`untidetect-tools`、`FakeBrowser`
  - 内容：代理写入、geo/locale/timezone 联动、transport leak probes、preflight checks
- 引入 `release discipline` 和 `privacy baseline` 审查模板
  - 来源：`Mullvad Browser`

### 4.2 只能借鉴，不应主线直接并入

- `BotBrowser` 的 Chromium patch / per-context browser-level CDP 内核能力
- `Camoufox` 的 Firefox 深 patch 与 Playwright 绑定链
- `Donut Browser` 的 Wayfern 商业内核耦合、同步全家桶、VPN/daemon 系统
- `VirtualBrowser` 的 Node/Vue 双前端控制面
- `FakeBrowser` 的旧 C++ TLS 栈与手工 `ClientHello`

### 4.3 明确排除出当前主线

- 在当前主仓直接引入浏览器 fork
- 在当前主仓引入 Python runtime
- 用 Angular / Neutralino / Vue WebUI 替换当前 Tauri 壳
- 为了“反指纹能力”而牺牲现有 `Win11 + Tauri 2 + 单实例 + 轻 Rust` 基线
- 直接复用带明显许可风险或供应商锁定的整仓实现

## 5. 统一整合后的目标架构

### 5.1 总原则

- 薄定制、厚基线
- 真实性优先于随机性
- 归一化优先于“花式伪装”
- 先 observation / validation，后 materialization
- 先 session bundle，再 browser fork

### 5.2 目标分层

`persona-pilot` 建议收敛为 7 层：

1. Desktop shell
2. Persona control plane
3. Runtime adapter plane
4. Proxy / IP plane
5. Fingerprint control + observation plane
6. Automation / recorder / template plane
7. Validation + release governance plane

建议的责任边界：

- `pages/components`
  - 只做渲染、操作入口、状态展示
- `features/hooks/store`
  - 管理 persona、session、proxy、validation、automation 的业务状态
- `src/services/desktop.ts`
  - 继续作为唯一原生边界
- `src-tauri`
  - 只承接真正需要 native 的文件、进程、网络调用、runtime sidecar 协调

### 5.3 建议新增的一等对象

- `PersonaSpec`
  - provider-neutral 的人格规格
  - 包含 fingerprint policy、proxy policy、automation policy、validation profile、storage bundle
- `SessionBundle`
  - 当前 persona 的可移植会话资产
  - 至少包含 profile dir、cookie/localStorage/sessionStorage snapshot、launch args、proxy binding、extension metadata、runtime notes
- `RuntimeAdapter`
  - 统一描述当前/未来的浏览器运行时
  - 当前可先覆盖 `FakeRunner`、`Lightpanda`
  - 未来可挂 `headed runtime` 适配层，但不直接进入主仓 fork
- `ObservationReport`
  - 统一承载实际观测值、期望值、偏差、风险等级、证据链接
- `ValidationProfile`
  - 一组可复跑的检测目标、阈值和 acceptance 规则

### 5.4 指纹模型建议

建议把“指纹”拆成 3 层：

- `control taxonomy`
  - 声明字段、策略、约束、来源
- `materialization`
  - 具体哪个 runtime 能消费哪些字段
- `observation`
  - 页面内、worker、network、WebRTC、canvas、audio、transport 实际看到的是什么

对当前仓库的直接对应：

- `src/network_identity/first_family.rs`
  - 继续做 control taxonomy 起点
- `src/network_identity/fingerprint_consumption.rs`
  - 扩成 observation / applied / ignored / mismatch 的完整报告
- `src/runner/lightpanda.rs`
  - 保留 runtime explain 契约，未来 headed runtime 也必须对齐这一套 explain

### 5.5 Proxy / IP 设计建议

从外部研究看，代理不应只是一条 URL，而应是完整 policy：

- provider
- protocol
- auth mode
- sticky session semantics
- geo expectation
- locale / timezone expectation
- DNS strategy
- WebRTC strategy
- transport probe result
- fallback / retry / rollback state

这部分应直接落到当前主线，而不是等浏览器 fork 先完成。

### 5.6 Automation 设计建议

外部研究说明，自动化能力最适合收敛成双通道：

- `persistent profile direct launch`
  - 适合稳定复现与人工接管
- `launch -> attach`
  - 适合任务调度、CDP、local API、MCP

对当前仓库的建议：

- 保持 `recorder / templates / tasks / automation` 为主线
- 补上 local API / MCP 合同设计
- 不在主线引入 Python wrapper
- 不让自动化脚本直接暴露到 page scope

## 6. 与当前 `persona-pilot` 的落地映射

### 6.1 当前真实基线

当前仓库已经具备：

- Tauri 2 + Vite + React + TypeScript 桌面壳
- `src/services/desktop.ts` 单一 native 边界
- `Lightpanda` runner
- 第一族指纹 schema 起点
- 行为计划与 runtime explain 契约
- cookie / storage 持久化基础
- Dashboard / Profiles / Proxies / Automation / Synchronizer / Logs / Settings 页面

当前不应被打断的收口项：

- provider-grade proxy API write
- synchronizer native batch / broadcast writes
- recorder / templates deeper native closure

### 6.2 直接新增的主线资产

建议新增但不改壳：

- `docs/validation-targets.md`
  - 统一列出检测站点、泄漏检查、acceptance 规则
- `src/features/validation/`
  - 做本地 validation board
- `src/types/persona.ts`
  - 定义 `PersonaSpec` / `SessionBundle` / `ValidationProfile`
- `src/network_identity/`
  - 扩字段字典与 observation schema
- `src/features/proxies/`
  - 引入 preflight / sticky / geo consistency / transport probes
- `src/features/profiles/`
  - 引入 groups、import/export、session bundle

### 6.3 不建议在当前仓库做的事

- 把 `Camoufox` 或 `BotBrowser` 式浏览器 fork 拉进主仓
- 为了 headed browser 直接改成多进程、多窗口、多后端架构
- 为了“功能看起来全”提前引入 sync/VPN/daemon/tray 全家桶

## 7. 分阶段落地路线

### P0：主线补强，不换壳

- 建立 Win11 本地 `validation board`
- 扩 `fingerprint observation`，先补 probe 再补 materialization
- 补 `proxy/IP consistency` 与 transport leak probes
- 补 `profile groups`、`import/export`、`session bundle`
- 给现有 runner 契约补 `ObservationReport`

P0 完成标准：

- 有可复跑的验证列表
- 每次 closeout 都能给出观测证据，不再只给“声明字段”
- proxy / locale / timezone / WebRTC / DNS 有一致性检查

### P1：统一 runtime adapter

- 抽象 `RuntimeAdapter` 契约
- 让 `Lightpanda` 和未来 `headed runtime` 共用 explain / observation / validation 合同
- 补 local API / MCP facade
- 补 transport-layer 观测而非 transport mimic

P1 完成标准：

- 同一 persona 可在不同 runtime adapter 上复用合同
- 验证板能区分 control、materialization、observation 三层

### P1.5：Camoufox 单任务生产级 Runner（90 分完整方案）

本阶段目标不是做 99+ 平台化浏览器体系，而是把 Camoufox 接成一条完整、可诊断、可清理、可回退的单任务执行链路。

本节在 2026-05-27 按当前项目结构复核后深化。结构证据：

- `TaskRunner` 已是统一执行器抽象，`RunnerKind::from_env()` 已用于选择 Fake / Lightpanda；参见 `src/runner/mod.rs`、`src-tauri/src/state.rs`、`src/main.rs`。
- `LightpandaRunner` 已有可复用的进程生命周期样板：pid 注册、取消、超时、stdout/stderr 预览、shutdown/kill；参见 `src/runner/lightpanda.rs`。
- `RunnerExecutionResult`、`RunnerSummaryArtifact` 和 `artifacts` 表已存在，run detail 已能读取 artifact 并在 Automation 详情展示；参见 `src/runner/types.rs`、`src/runner/engine.rs`、`src/desktop/mod.rs`、`src/components/automation/RunDetailPanel.tsx`。
- 前端已有分页、虚拟滚动、300ms debounce 和 stale-result requestId 保护；参见 `src/features/logs/*`、`src/features/tasks/*`、`src/components/VirtualList.tsx`、`src/shared/components/Table.tsx`。
- release 性能预算已有真实 smoke 入口和当前 warning 基线：`2437ms` cold start、`465MB` RSS、`9` processes；参见 `docs/release-performance-mitigation-plan.md`、`scripts/release_performance_smoke.ps1`。

定位：

- Camoufox 是 `Browser Execution Engine`，不是主应用打开引擎。
- 主应用仍是 `Tauri 2 + WebView2`，Camoufox 只运行任务目标网页。
- Camoufox 必须作为可选 runner adapter 存在；未配置或不可用时，不影响 Lightpanda、FakeRunner 和主应用启动。
- 第一版不默认引入 Python runtime、不默认启动 remote server、不把 Firefox fork 拉进主仓。

#### 项目结构落点

优先走现有 Rust runner 路线，而不是新增 Go/Node/Python 常驻服务：

```text
src/runner/mod.rs
  增加 RunnerKind::Camoufox

src/runner/camoufox.rs
  新增 CamoufoxRunner，复用 LightpandaRunner 的生命周期模式

src/runner/types.rs
  补 BrowserEngineCapability / CamoufoxSettings / CamoufoxRun explain 类型

src/runner/engine.rs
  复用现有 claim、run status、summary_artifacts、artifacts 表写入

src-tauri/src/state.rs
  RunnerKind::Camoufox -> Arc<CamoufoxRunner>

src-tauri/src/commands.rs
  只加设置/能力检测命令，不把 Camoufox 业务逻辑写进命令函数

src/services/desktop.ts
  继续作为唯一 invoke 出口，新增 typed wrapper

src/features/settings/*
  增加 Camoufox 设置表单和 capability 只读状态

src/components/automation/RunDetailPanel.tsx
  复用现有 artifact/detail 展示，不新增大列表或全量日志面板
```

不要新增：

- `backend/` 常驻 Camoufox 服务。
- Node sidecar。
- Python embedded runtime。
- 新 Tauri plugin。
- 第二套 Camoufox 专用 UI。

#### 性能最优原则

90 分版本的性能目标不是让 Camoufox 常驻，而是让它“不拖垮主应用基线”。具体约束：

1. **按任务启动，默认不常驻**
   - 默认 `managed-per-run` 模式：任务开始才启动 Camoufox，任务结束立即关闭。
   - 不在应用启动时检测或预热 Camoufox，避免恶化 cold start。
   - Settings 页 capability 检测必须由用户点击或进入设置后懒加载触发。

2. **并发默认 1**
   - `PERSONA_PILOT_CAMOUFOX_MAX_CONCURRENCY` 默认 `1`。
   - 不与 `PERSONA_PILOT_RUNNER_CONCURRENCY` 简单相乘，避免每个 worker 都能拉起一个重浏览器。
   - 后续如要并发，必须先有 release RSS/process count 报告证明预算可接受。

3. **artifact 按需采集**
   - 成功任务默认保存 `summary.json`、`stdout.log`、`stderr.log` 和 `screenshot.png`。
   - `page.html` 默认只保留截断预览或失败时完整保留；避免大 HTML 长期堆积。
   - stdout/stderr 沿用 Lightpanda 预览策略，UI 只展示预览和 artifact ref，不把大日志塞进全局状态。

4. **设置与能力检测做缓存**
   - capability 结果包含 `checkedAt`、`version`、`reason`。
   - TTL 默认 5 分钟；路径或设置变更后失效。
   - 检测命令必须有短超时，建议 `5-8s`。

5. **profile 映射只做轻量字段**
   - 第一版只消费 locale、timezone、UA、viewport、screen、proxy。
   - 不启用 Canvas/Audio/font 深度策略，不引入字体包或噪声引擎。
   - 所有未消费字段进入 `ignored_fields`，不静默假装已应用。

6. **UI 维持现有性能模式**
   - 设置页不轮询 capability。
   - run detail 使用现有 artifact list；artifact 超过 200 时必须分页或折叠，不做全量展开。
   - logs 继续走 `listLogPage` 分页和 300ms debounce。

7. **release 指标必须单独记账**
   - 不把 Camoufox 任务运行时 RSS 计入主应用 idle RSS 目标。
   - 必须新增 task-run scoped measurement：启动耗时、导航耗时、峰值 RSS、子进程数、artifact size。
   - release smoke 仍要确认主应用 idle 不因 Camoufox 设置/检测代码超预算。

#### Runner 生命周期设计

CamoufoxRunner 应复用 LightpandaRunner 的关键模式：

```text
execute(task)
  -> validate action/url/settings
  -> resolve timeout = task.timeout_seconds.clamp(1, 120)
  -> create run temp dir
  -> write request.json
  -> spawn child with stdout/stderr piped
  -> register pid by task_id
  -> wait readiness/navigation under timeout
  -> capture result artifacts
  -> unregister pid
  -> graceful shutdown
  -> force kill on timeout/cancel/cleanup failure
  -> return RunnerExecutionResult with summary_artifacts
```

取消语义：

- `cancel_running(task_id)` 必须先查 pid registry。
- Windows 下优先结束进程树，不能只 kill parent。
- cancel 成功返回 `RunnerOutcomeStatus::Cancelled`，并沿用现有 `runner_cancelled` 语义，不新增一套 UI 状态。

失败分层：

```text
config_error        -> settings/path/module 缺失
launch_error        -> 进程启动失败或 endpoint 不可用
navigation_error    -> URL/网络/页面加载失败
proxy_error         -> proxy auth/connect 失败
profile_error       -> profile 映射不合法
timeout             -> task timeout
cancelled           -> operator cancel
cleanup_error       -> 结束后清理失败
```

#### 推荐技术路径

第一实现不要直接依赖 remote server。按优先级：

1. **外部命令包装模式**
   - Rust runner 调用用户配置的 `pythonPath` + `camoufox` 脚本或可执行文件。
   - 通过临时 `request.json` / `summary.json` 交换结构化结果。
   - 优点：对当前架构侵入最低，主应用不嵌 Python runtime。
   - 缺点：需要一个很薄的 runner helper 脚本或明确的 Camoufox CLI 契约。

2. **一次性 helper 脚本模式**
   - `scripts/camoufox_runner.py` 可作为外部环境脚本，但不打包 Python。
   - Rust 只负责传入 request path、run dir、timeout、env。
   - helper 输出结构化 summary 和 artifact。

3. **remote server 模式**
   - 只作为 P2+，不进入 90 分第一闭环。
   - 原因：会引入端口、server 生命周期、session 复用风险和额外进程治理。

#### 任务与 action 范围

第一版只支持和 Lightpanda v1 对齐的读型 action：

```text
open_page
get_html
get_title
get_final_url
extract_text
validation_probe
```

不支持：

- 任意 Playwright 脚本上传执行。
- 多实例开通/交互式挑战适配。
- 多标签同步控制。
- 长时人工接管。

这样可以复用已有 browser-facing API 和 task/control-plane 语义，不让 Camoufox 把产品层重新拖成引擎细节。

90 分保留范围：

- Camoufox 设置与可用性检测。
- `engine = lightpanda | camoufox` 任务选择。
- 单任务启动、打开 URL、采集结果、关闭进程。
- 超时、取消、强杀和孤儿进程清理。
- 基础 proxy 接入。
- 基础 fingerprint/profile 映射。
- artifact：`summary.json`、`stdout.log`、`stderr.log`、`page.html`、`screenshot.png`。
- 任务详情回显 engine、profile version、artifact 和归一化错误。
- release build 下 smoke 验证。

90 分明确不做：

- remote server 默认主链。
- browser pool / 多实例调度池。
- 自动下载或内置安装 Camoufox。
- 长驻 Python / Node 后端服务。
- 深度 Canvas / Audio / 字体拟真。
- 高级 trust score。
- 复杂站点策略 DSL。
- 大规模并发调度。

建议新增类型：

```ts
export type BrowserEngineKind = "lightpanda" | "camoufox";

export interface BrowserEngineCapability {
  kind: BrowserEngineKind;
  available: boolean;
  version?: string;
  supportsHeadful: boolean;
  supportsHeadless: boolean;
  supportsProxy: boolean;
  supportsFingerprint: boolean;
  reason?: string;
}

export interface CamoufoxSettings {
  enabled: boolean;
  pythonPath?: string;
  executablePath?: string;
  profileRoot: string;
  defaultHeadless: boolean;
  startupTimeoutMs: number;
  runTimeoutMs: number;
}
```

建议运行请求：

```ts
export interface CamoufoxRunRequest {
  runId: string;
  url: string;
  headless: boolean;
  timeoutMs: number;
  proxy?: {
    server: string;
    username?: string;
    password?: string;
  };
  fingerprint?: {
    locale?: string;
    timezone?: string;
    userAgent?: string;
    viewportWidth?: number;
    viewportHeight?: number;
    screenWidth?: number;
    screenHeight?: number;
  };
}
```

建议运行结果：

```ts
export interface CamoufoxRunResult {
  status: "success" | "failed" | "timeout" | "cancelled";
  startedAt: string;
  finishedAt: string;
  exitCode?: number;
  engineVersion?: string;
  artifacts: {
    type: "summary" | "stdout" | "stderr" | "html" | "screenshot";
    path: string;
  }[];
  error?: {
    code: string;
    message: string;
  };
}
```

必须落地的模块边界：

- `src/types/desktop.ts`：增加 Camoufox 设置、能力、运行请求和运行结果类型。
- `src/services/desktop.ts`：增加 `readCamoufoxSettings`、`applyCamoufoxSettings`、`checkCamoufoxCapability`；所有 Tauri invoke 仍只能从这里出口。
- `src/features/settings` 或 runtime settings：增加 Camoufox 配置 UI。
- `src/features/runs`：任务详情显示 engine、artifact、profile version 和错误原因。
- `src-tauri/src`：只做命令包装、路径检查、进程协调，不承载业务策略。
- 现有 runner/core 层：新增 `CamoufoxRunner`，与 Lightpanda/FakeRunner 对齐 result/error/artifact 合同。

运行目录固定为：

```text
data/
  engines/
    camoufox/
      settings.json
      profiles/
      runs/
        <run-id>/
          request.json
          summary.json
          stdout.log
          stderr.log
          page.html
          screenshot.png
```

错误码必须先稳定：

```text
camoufox_disabled
camoufox_path_missing
camoufox_python_missing
camoufox_module_missing
camoufox_version_unsupported
camoufox_launch_timeout
camoufox_launch_failed
camoufox_navigation_timeout
camoufox_proxy_failed
camoufox_profile_invalid
camoufox_cancelled
camoufox_process_cleanup_failed
```

执行生命周期：

```text
1. 创建 run dir
2. 写入 request.json
3. 检查 Camoufox settings
4. 检查 capability cache 是否有效
5. 启动进程
6. 记录 pid
7. 等待页面加载或超时
8. 保存 screenshot/html/logs
9. 写 summary.json
10. 正常关闭
11. 超时/取消时强杀
12. 标记最终状态
```

90 分验收标准：

```text
1. Camoufox 未配置时，主应用正常启动
2. 路径错误时，设置页显示 camoufox_path_missing
3. Python/Camoufox 缺失时，显示明确原因
4. 正确配置后，capability 显示 available=true 和 version
5. 一个任务可以选择 engine=camoufox
6. 能打开 https://example.com
7. 任务成功后有 summary.json
8. 至少保存 screenshot.png 和 stdout/stderr
9. 导航超时后任务状态为 timeout
10. 用户取消后任务状态为 cancelled
11. 超时/取消后没有残留 Camoufox/Python 进程
12. Lightpanda 原有任务不受影响
13. 所有 Tauri invoke 仍集中在 src/services/desktop.ts
14. release build 下 smoke 通过
15. enforce-win11-tauri.ps1 通过
```

实现顺序：

1. **结构接入，不启动浏览器**
   - 增加 `RunnerKind::Camoufox`、设置类型、capability 类型、错误码。
   - 验证：typecheck / Rust test 编译；主应用无 Camoufox 配置仍启动。

2. **能力检测**
   - 增加 `check_camoufox_capability`，手动触发、短超时、缓存。
   - 验证：空路径、错路径、缺模块、正确路径四类结果稳定。

3. **最小 runner**
   - 单任务打开 `https://example.com`，产出 summary/stdout/stderr/screenshot。
   - 验证：成功、超时、取消、进程清理。

4. **artifact/detail 闭环**
   - 把 artifact ref 进入现有 artifacts 表和 RunDetailPanel。
   - 验证：run detail 可看到 artifact，不把大内容进全局 store。

5. **基础 profile/proxy 映射**
   - 映射轻量字段，输出 applied/ignored explain。
   - 验证：profile version、proxy id、applied/ignored 在 result_json 可见。

6. **性能验收**
   - 主应用 release smoke 不因 Camoufox 设置代码变差。
   - Camoufox task-run report 单独记录启动耗时、峰值 RSS、进程数、artifact size。

#### 分支与多 agent 执行建议

如果 subagent 通道可用，建议用 5 个互不重叠切片并行：

| Agent | 责任 | 写入范围 | 验收 |
| --- | --- | --- | --- |
| A Runner contract | RunnerKind、类型、错误码、CamoufoxRunner skeleton | `src/runner/mod.rs`、`src/runner/types.rs`、`src/runner/camoufox.rs`、`src-tauri/src/state.rs` | Rust 编译；Fake/Lightpanda 不变 |
| B Settings/capability | 设置读写、能力检测命令、desktop wrapper | `src-tauri/src/commands.rs`、`src/services/desktop.ts`、`src/types/desktop.ts`、`src/features/settings/*` | 设置页可显示 capability；无路径不报崩 |
| C Artifact/detail | artifact 写入合同和 RunDetail 展示调整 | `src/runner/engine.rs`、`src/desktop/mod.rs`、`src/components/automation/RunDetailPanel.tsx` | run detail artifact 可见；大内容不进 store |
| D Profile/proxy mapping | fingerprint/proxy 轻量映射和 explain | `src/network_identity/*`、`src/runner/camoufox.rs` | applied/ignored 字段可测 |
| E Verification/perf | tests、smoke 脚本、文档更新 | `scripts/*`、`docs/*`、相关 tests | release smoke + Camoufox smoke 有报告 |

本轮 2026-05-27 曾尝试派出 4 个 explorer subagent，但本地 distributor 均返回 `503 Service Unavailable`，因此当前深化由主线程只读扫描完成。后续真正实现时，如果 subagent 恢复，应按上表拆分；如果仍不可用，则按实现顺序串行推进。

#### 2026-05-27 切片 D 实施记录

当前分支：`codex/camoufox-runner-90`。

本次从方案进入实施记录阶段，但不得视为 Camoufox runtime 已完成。切片 D 的目标是先约束 `profile/proxy mapping` 的最小可验证合同：Camoufox runner 后续只能输出 profile version、proxy id、applied/ignored fields、failure reason 和 artifact refs；不得把完整 profile、proxy 密钥、cookie/storage payload 或大体积页面内容写入全局 store。

Worker 切片边界：

- **D Profile/proxy mapping**：后续实现写入范围应集中在 `src/network_identity/*` 与 `src/runner/camoufox.rs`，只做轻量映射和 explain。
- **D 当前文档切片**：本轮只补实施记录与最小验证口径，不改 runner 代码，不新增默认引擎，不触碰 Lightpanda/FakeRunner 行为。
- **D 与 E 的接口**：D 只产出可被 E 验证的字段合同；release smoke、Camoufox task-run smoke、进程清理检查仍归 E Verification/perf。

最小验证草案：

1. 未配置 Camoufox 时，`engine=lightpanda` / FakeRunner 现有任务不受影响。
2. 选择 Camoufox 且 profile/proxy 不完整时，结果必须包含 `ignored` 和明确 `failureReason`，不能静默成功。
3. 选择 Camoufox 且 profile/proxy 可映射时，`result_json` 至少包含 `profileVersion`、`proxyId`、`applied`、`ignored`、`runtimeAdapter=camoufox`。
4. 取消或超时后，不得残留 Camoufox/Python 子进程。
5. artifact 只保存 summary/stdout/stderr/screenshot 引用；大内容不得进入全局 store。

性能约束：

- 保持主应用 release baseline 独立记账：cold start `<= 2.0s`、idle RSS `<= 220MB`、process count `<= 4`。
- Camoufox 不得在 app 启动时预热、自动检测或常驻；能力检测只能由用户触发或短 TTL 缓存。
- Camoufox 任务运行指标必须单独记录：task cold start、峰值 RSS、子进程数、artifact size、cleanup result。
- 当前已知 release smoke 仍有 warning 基线，不能因新增设置、mapping 或检测代码进一步恶化主应用 idle 指标。

完成前不得宣称：

- Camoufox 已成为默认引擎。
- 已具备完整 AdsPower 级 fingerprint realism。
- 已完成 headed runtime 平台化能力。
- 已完成 remote server / browser pool。

#### 2026-05-27 切片 E 验证与文档记录

本轮 E 只负责验证口径、脚本入口和维护文档同步，不修改 `src` 代码，不接管其他 worker 的 runner / settings / mapping 实现。

当前代码证据显示 Camoufox 已进入 skeleton / contract-ready 阶段，但不是 runtime-ready：

- `src/runner/mod.rs` 已有 `RunnerKind::Camoufox` 和 `PERSONA_PILOT_RUNNER=camoufox` 映射。
- `src-tauri/src/state.rs` 可选择 `CamoufoxRunner`，默认 runner 仍不因此变为 Camoufox。
- `src-tauri/src/commands.rs` 已有 `read_camoufox_settings`、`apply_camoufox_settings`、`check_camoufox_capability`，当前 capability 只检查路径存在性，不启动浏览器。
- `src/services/desktop.ts` 已导出 `readCamoufoxSettings`、`applyCamoufoxSettings`、`checkCamoufoxCapability`，继续满足所有 Tauri invoke 只从 `src/services/desktop.ts` 出口。
- `src/runner/camoufox.rs` 当前仍返回 `runner_disabled` / `runner_config_missing` / `runner_not_implemented`，并显式标记 `real_browser_execution=false`、`browser_launch_attempted=false`。

新增验证入口：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/camoufox_smoke.ps1 -AllowBlocked
```

该命令默认执行非破坏性合同检查并生成 `data/reports/camoufox-smoke/camoufox-smoke-*.json`。当前预期状态是 `contract_ready_runtime_smoke_required`，表示设置/runner skeleton 合同可被检查，但真实 Camoufox 启动、artifact、取消/超时清理和 task-run 性能尚未完成。

如只想本地快速核对而不写 report，可用：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/camoufox_smoke.ps1 -NoReport -AllowBlocked
```

完整 E 验收命令应在后续 runtime 实现后执行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/camoufox_smoke.ps1 -RunRustTests -RunReleasePerformanceSmoke
powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot D:\SelfMadeTool\personal-pilot
```

完成前仍必须补齐的 runtime proof：

1. `engine=camoufox` 真实打开 `https://example.com`。
2. 任务产出 `summary.json`、`stdout`、`stderr`、`screenshot` artifact ref。
3. timeout / cancel 后无残留 Camoufox 或 Python 子进程。
4. Lightpanda / FakeRunner 现有任务不回归。
5. release idle smoke 和 Camoufox task-run 性能报告分开记录。

### P2：独立实验轨

- 若确定要研究 Firefox / Chromium 深 patch
- 单开实验仓或实验分支
- 单独评估：
  - 许可
  - rebasing 成本
  - release pipeline
  - 供应链与签名
  - 对当前主仓的侵入度

P2 的前置条件：

- P0/P1 已稳定
- 当前主仓的 proxy/synchronizer/recorder 收口已完成
- 用户明确批准进入自有内核实验

## 8. 明确决策建议

本轮建议直接定下以下口径：

- 当前主仓不引入浏览器 fork
- 当前主仓不引入 Python runtime
- 当前主仓不替换 Tauri 壳
- 当前主线优先级是 `validation + schema + proxy consistency + session bundle`
- 自有内核研究单开实验轨
- `Mullvad` 学原则，`BotBrowser/Camoufox` 学方法，`Donut/VirtualBrowser` 学产品面，`FakeBrowser` 只学 transport 观测点

## 9. 一句话路线

不是把 7 个外部项目拼成一个更重、更乱的“超级浏览器”，而是把它们最有价值的能力抽成 `persona-pilot` 的 4 条主线资产：

- `validation board`
- `fingerprint control + observation schema`
- `session bundle + profile contract`
- `proxy/IP consistency + automation facade`

这样既能吸收外部长处，也不会打断当前 `95% / 7%` 的主线收口。
