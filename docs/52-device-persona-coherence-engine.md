# 52 设备人格与相干性引擎工程指导与验收

Updated: 2026-07-08 (Asia/Shanghai)

## 0. 定位

- **目标**：把「指纹值各自默认 + 本机采样」升级为**真实机型人格（Device Persona）驱动 + 相干性硬门禁 + geo 活体一致 + 受控老化**，消除「现实中不存在的设备组合」和「跨 profile 同一基础设备」这两类横向矛盾。
- **四文档关系**：
  - `docs/49`：修**已有 hook**真实性（toString/UA/噪声/DNS/鼠标）。
  - `docs/50`：**战略维度**（出站身份统一、同一用户、连接隐匿、信任优先、成本不对称）。
  - `docs/51`：**指纹面广度**（补齐未 hook 的 surface）。
  - `docs/52`（本文）：**相干性与人格**——保证所有指纹面**组合起来像同一台真实设备**，且跨 profile 是**不同真实设备**，并随时间**合理演化**。
- **核心洞察**：检测器抓的不是「某个值假」，而是「值之间不自洽」。一个内部完全自洽的普通设备，比一个每项都「高分」但互相矛盾的设备更难被拦——这正是 `docs/50` 不对称支配的指纹侧落点。
- **诚实边界**：能由 fingerprint-chromium seed 内核提供的优先内核；人格库只约束**取值组合**，不重复注入内核已覆盖项（防 `docs/50` F6 双层）。无证据不写 observed。

## 1. 证据基线：相干性现状与缺口

本轮核实（文件:行）：

| 编号 | 现状 | 证据 | 缺口 |
| --- | --- | --- | --- |
| C-E1 | `AssessFingerprintConsistency` 10 维打分**仅用于报告** | `backend/internal/browser/fingerprint_consistency.go:66`、`identity_report.go:256` | 不是**发射前硬门禁**，不自洽 profile 照常启动 |
| C-E2 | `checkProxyVsExitRegion` 永远 pass 占位 | `fingerprint_consistency.go:141-144` | 代理出口地域 vs 目标地域未真正校验 |
| C-E3 | `EvaluateConsistencyMatrix` 仅 4 项且未接发射流程 | `consistency_matrix.go:33`（生产无调用，仅测试） | 第二套相干性合同闲置 |
| C-E4 | 指纹值来自 `MaterializeRuntimeArgs` 独立默认 + 本机采样 | `runtime_materialize.go:42-68`、`scripts/full_observed_fingerprint_probe.mjs`（采本机） | **无真实机型人格库**；跨 profile 同基础设备；可能生成现实不存在组合 |
| C-E5 | `docs/34` Reference Library 仅设计 | `docs/34-environment-signature-sampling.md` | 未产品化为 persona 分配 |
| C-E6 | Geolocation 仅**检测**不覆盖 | `backend/internal/browser/geo_mismatch.go:34`（仅 `GeoMismatchInfo`）；全库无 `setGeolocationOverride` | `navigator.geolocation` 坐标与代理城市不一致 |
| C-E7 | 指纹永久冻结 | `docs/00-master-todo.md:208` 「指纹渐变式变异」未做 | 长期时间轴上「零演化」本身是弱信号 |

已具备（保留）：`fingerprint_consistency.go` 的 10 维检查逻辑、`ApplyGeoLocale`（locale/timezone 对齐）、`HumanizeSeed` 确定性派生、observed 采集链路。

## 2. 统一原则（所有任务通用）

1. **组合真实优先于单值高分**：宁可用一个平凡但自洽的机型，也不拼凑矛盾的「高端」组合。
2. **人格单一真相源**：一个 profile 绑定一个 Persona，所有指纹面（UA/GPU/屏幕/核数/内存/字体/audio/webgl/touch/平台）从**同一 persona 派生**，与 `docs/49` A3 core 版本、`docs/51` 各 surface 同源。
3. **相干性是硬门禁**：不自洽（hard 失败）**阻止发射**或强制降级，而非仅报告。
4. **跨 profile 多样性**：人格库覆盖多机型，分配时避免池内碰撞（联动 `docs/49` B2 指纹 DNA）。
5. **演化有据可依**：老化遵循真实规律（Chrome 版本随时间升、字体只增不减、硬件不变），不随机漂移。

## 3. 任务分解

### P0

#### Task DP1 — 真实设备人格库（Device Persona Library）
- **现状/根因**（C-E4/C-E5）：无机型模板，指纹面各自为政。
- **技术指导**：
  1. 新增 `backend/internal/browser/persona/library.go`：定义 `DevicePersona{ osName/osVersion, uaTemplate, gpuVendor/renderer, screen(w/h/avail/dpr), hardwareConcurrency, deviceMemory, fontSet, audioProfile, webglParams, platform, touch }`，每个字段取值来自**真实共现**（如 "Windows 11 + Intel UHD 集显 + 1920×1080 + 8 核 + 8GB + Win 字体集"）。
  2. 内置 N（≥12）个覆盖主流 Win/Mac 的真实机型，标注来源采样（可用 `full_observed_fingerprint_probe.mjs` 在多台真机/多环境采集，或权威公开机型库，禁止编造）。
  3. profile 创建时按 `HumanizeSeed` 确定性分配一个 persona；持久化到 profile。
- **改动文件**：新增 `persona/library.go`、`persona/assign.go`；`runtime_materialize.go` 从 persona 派生而非独立默认；`profile.go` 增 `PersonaID`。
- **AC**：
  - AC1：单测：同 seed 稳定分配同 persona；不同 seed 覆盖多机型。
  - AC2：persona 派生出的 UA/GPU/屏幕/核数/字体两两自洽（喂给 `AssessFingerprintConsistency` 得 `coherent`、score≥80）。
  - AC3：库内每个 persona 标注采样来源，无编造字段（评审 + 单测校验必填元数据）。
  - AC4：`go test ./backend/internal/browser/... -count=1` passed。

#### Task DP2 — 相干性硬门禁（发射前拦截）
- **现状/根因**（C-E1/C-E3）：相干性仅报告，不拦截。
- **技术指导**：
  1. 在 `app_instance.go` 启动链路（`MaterializeRuntimeArgs` 之后、`exec.Command` 之前）调用 `AssessFingerprintConsistency`；`HardFailures>0` → **阻止发射**并返回明确错误；`suspicious` → warning + 审计。
  2. 合并 `consistency_matrix.go` 与 `fingerprint_consistency.go` 为单一入口（消除 C-E3 双合同），避免口径分裂。
  3. `IdentityStrengthReport` 展示 coherence 门禁结果。
- **AC**：
  - AC1：单测：注入一个 GPU/OS 矛盾的 profile → 启动被拒且错误含具体维度。
  - AC2：自洽 profile 正常启动，无回归（XHS live 12/12）。
  - AC3：两套相干性合同合一，生产只调一个入口（grep 断言旧入口无生产调用）。

#### Task DP3 — Geolocation 活体一致（联动 ApplyGeoLocale）
- **现状/根因**（C-E6）：只检测不覆盖。
- **技术指导**：
  1. 启动注入阶段用 CDP `Emulation.setGeolocationOverride`（lat/lon/accuracy）设为**代理城市**坐标（由出口 IP 反查城市 → 坐标表）。
  2. 与 `ApplyGeoLocale` 的 timezone/locale 同源（同一 country/city）。
  3. `permissions.query({name:'geolocation'})`（`docs/51` S2）与之自洽。
- **改动文件**：`app_environment_injection.go`/`environment_injector.go`（geo override）、`runtime_geo_locale.go`（城市坐标表）。
- **AC**：
  - AC1：`navigator.geolocation.getCurrentPosition` 返回坐标落在代理城市（对照断言）。
  - AC2：坐标/timezone/locale 三者同城一致；`go test` passed。

### P1

#### Task DP4 — 受控指纹老化（Fingerprint Aging）
- **现状/根因**（C-E7）：永久冻结。
- **技术指导**：
  1. persona 记录 `bornAt`；按真实规律演化：Chrome major 随发布节奏推进（联动 `docs/49` A3 core 升级）、字体集只增不减、硬件不变、偶发小版本更新。
  2. 演化是**确定性 + 缓慢**（周/月级），非随机；演化即视为「同一用户的设备更新」，不断裂 trust（与 `docs/50` B3 seed 生命周期锁一致）。
- **AC**：单测：同 persona 在 t 与 t+90d 的 UA major 单调不减、字体集包含关系、硬件恒定；演化不触发 trust 重置。

#### Task DP5 — 跨 profile 多样性 gate（联动 docs/49 B2）
- **技术指导**：persona 分配后计算池内相似度，`>0.92` 触发重分配；`fingerprint_dna_gate.ps1` 纳入 persona 维度。
- **AC**：100 个 profile 分配后无 persona 组合碰撞 `>0.92`；分布覆盖多机型（直方图证据）。

## 4. 覆盖率与验收模型

1. 扩展 `runtime_projection.go`：新增 `personaId`、`coherenceGate` 归属，纳入 observed/identity 报告。
2. 新增 `scripts/coherence_gate.ps1`：对每个 profile 跑 `AssessFingerprintConsistency`，输出 coherent/suspicious/inconsistent 分布，`inconsistent>0` 则 fail。
3. 与 `docs/50` C3 `stealth_surface_audit`、`docs/51` `fingerprint_surface_coverage_gate` 合并到统一 stealth 报告。

## 5. 总验收门禁（本文任务）

```powershell
go test ./backend/internal/browser/... -count=1
.\scripts\coherence_gate.ps1                       # DP2：相干性硬门禁分布
.\scripts\fingerprint_dna_gate.ps1                 # DP5：persona 多样性（docs/49 B2 共用）
node scripts/full_observed_fingerprint_probe.mjs   # persona 派生 vs observed 一致
.\scripts\platform_99_gate.ps1 -Track all
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda    # 回归不得下降
```

**通过判据**：persona 派生指纹经 `AssessFingerprintConsistency` 全部 `coherent`；矛盾 profile 被硬门禁拒绝发射；geolocation 与代理城市一致；老化单调合理；100-profile 无 persona 碰撞；XHS live 12/12 不回归。任一未达标 `blocked` + `failureReason`，禁止改写 accepted。

## 6. 与 49/50/51 的合并执行顺序

| 波次 | 本文任务 | 依赖 |
| --- | --- | --- |
| Wave 1 | DP1 人格库、DP2 硬门禁 | 依赖 `docs/49` A3（core 版本单一真相源） |
| Wave 2 | DP3 geo 活体、DP5 多样性 | 依赖 `docs/49` B2 DNA、`docs/51` S2 permissions |
| Wave 3 | DP4 老化 | 依赖 `docs/49` A3 内核版本轨、`docs/50` B3 seed 锁 |

## 7. 风险与回滚

| 风险 | 缓解 |
| --- | --- |
| 人格库字段编造导致「假机型」 | 强制每 persona 标注真实采样来源；单测校验元数据必填；宁缺毋造 |
| 硬门禁误杀合法 profile | 先 warning 观察一轮再切 hard；hard 仅限确定性矛盾（GPU/OS、locale/accept-language、viewport>screen） |
| geo override 与真实定位冲突触发权限弹窗 | 与 permissions.query 一致返回；无把握则不声明高精度 |
| 老化引入不稳定 | 演化确定性 + 周/月级；单测锁单调性 |
| 回滚 | 每 Task 独立提交；`~/.cursor/anti-lazy/scripts/rollback.py` |

## 8. 变更同步

每 Task 完成后更新 `docs/02-current-state.md`（persona/coherence 证据）与 `docs/04-improvement-backlog.md`（退出条件）；`docs/00-master-todo.md`「指纹渐变式变异」勾除并回指 DP4；`docs/34` 标注「Reference Library 已由 docs/52 DP1 产品化」。
