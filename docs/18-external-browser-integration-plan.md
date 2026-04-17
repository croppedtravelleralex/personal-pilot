# 外部指纹浏览器整合方案
Updated: 2026-04-16 (Asia/Shanghai)

## 0. 口径边界

- 主线交付：`100% / 0% / green`
- 整体终局：`35% / 65% / yellow`
- 本文属于 `整体终局` 轨道，主要服务于指纹真实性、运行时深度、AdsPower 追评和外部能力整合
- 本文不是当前主线 `7%` closeout 的完成证明，方案落地后才会逐步反映到整体终局进度

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
- 当前 runtime 只投影 `12` 个 env-backed 指纹字段
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

这样既能吸收外部长处，也不会打断当前 `100% / 0%` 的主线收口。
