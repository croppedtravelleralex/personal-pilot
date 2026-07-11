# 双轨 99+ 规范（PGS / XBS）

> Personal Pilot Generic Stealth（PGS）+ Platform eXtra Bundle（XBS）

Updated: 2026-06-29

## 1. 架构分层

| 层 | 职责 | 目录 |
|---|---|---|
| PGS 通用底座 | 代理桥、注入门控、拟人化 workbench、API 审计 | `backend/` |
| 通用范式 Plans | 登录/养号/Feed 采集/控制台只读 | `plans/` |
| XBS 平台差异包 | URL、选择器、Cookie 域、节奏 | `platform-packs/{platform}/` |

**原则**：凡 XHS 可复用到其他平台的逻辑，必须沉入 PGS；XBS 只保留平台差异。

**交接入口**：`docs/45-stealth-platform-handoff.md`（命令、实例、live 验收纪律）。

**全局 99+ 蓝图**：`docs/46-global-99plus-architecture.md`（L0–L6 分层栈、Input Plane Router、达标路径）。

## 2. Phase 0 硬门槛（已实现）

| ID | 门槛 | 实现 |
|---|---|---|
| H-P1 | sing-box 端口 hold-until-bind | `proxy.reservePortNumber()` + sing-box 15s 就绪等待 |
| H-P2 | 禁止静默 direct 回退 | `proxy.ValidateDirectFallbackSwitch()` + 标签 `allow-direct-fallback` |
| H-P3 | Injection Ready Gate | `Profile.injectionReady` + `runningProfileForWorkbench` |
| H-P4 | 全 API Audit | `apiAuditMiddleware` + `GET /api/audit/logs` |
| H-P5 | script 旁路禁令 | `validateWorkbenchScriptPolicy` + 标签 `allow-script-bypass` |

## 3. 通用 Plans（Phase 1–2）

| Plan | 文件 | 说明 |
|---|---|---|
| account-login-v1 | `plans/account-login-v1.yaml` | 手机号/OTP 登录范式（type/click humanize） |
| account-nurture-conservative-v1 | `plans/account-nurture-conservative-v1.yaml` | 保守养号 |
| feed-scrape-v1 | `plans/feed-scrape-v1.yaml` | Feed 滚动采集 |
| platform-console-readonly-v1 | `plans/platform-console-readonly-v1.yaml` | 创作者中心只读巡检 |

## 4. XHS 差异包（Phase 3）

目录：`platform-packs/xhs/`

- `pack.yaml` — 平台元数据、依赖 plans
- `urls.yaml` — 主页/登录/创作者中心 URL
- `selectors.yaml` — 平台 DOM 选择器
- `cadence.yaml` — 养号节奏 override

## 5. 严格验收

```powershell
# PGS 硬门槛（离线）
.\scripts\platform_99_gate.ps1 -Track PGS

# XBS 差异包（离线结构 + 可选 live）
.\scripts\platform_99_gate.ps1 -Track XBS -Platform xhs

# XHS live（需运行实例 + 可用代理；UDEAL 常需 ?pp_via_ssh=panda）
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda -StartTimeoutSec 300

# 能力雷达（离线 API/契约；≠ 目标站 live）
.\scripts\capability_scenario_suite.ps1 -Tier all -GenerateRadar
```

PGS 通过标准：H-P1~H-P5 全部 PASS + `go test` 绿。

XBS 通过标准：pack 结构完整 + **live 场景 pageUrl 非 chrome-error**（见交接文档）。

## 6. 实例标签

| 标签 | 用途 |
|---|---|
| `auto-99` | 启用 Stealth Autopilot |
| `allow-script-bypass` | 允许 workbench `script`（调试专用） |
| `allow-direct-fallback` | 允许桥接代理切回 direct:// |

## 7. 验收签字表

| 轨道 | 场景 | 门槛 | 状态 |
|---|---|---|---|
| PGS | 端口预留 | H-P1 | 代码已落地；跑 gate 确认 |
| PGS | 禁 direct 回退 | H-P2 | 代码已落地 |
| PGS | 注入门控 | H-P3 | 代码已落地 |
| PGS | API 审计 | H-P4 | 代码已落地 |
| PGS | script 禁令 | H-P5 | 代码已落地 |
| PGS | capability 雷达 | 离线 34 场景 | **34/34 passed**（2026-06-29） |
| XBS | xhs pack 结构 | S-xhs-1 | 已落地；跑 gate 确认 |
| XBS | explore 页面可达 | live URL | **已通过**（2026-06-30；`xhs_live_acceptance.ps1` L-06 HTTPS explore） |
| XBS | CreepJS ≥85 | stealth-probe | **已通过**（2026-06-30；creepTrust=100） |
| XBS | 登录 humanize | S-xhs-2 | 待 live；脚本须去 script |

## 8. 已知缺口（勿误报通过）

- workbench navigate HTTP 200 但 `pageUrl=chrome-error://` **不算通过**。
- capability 雷达 / PGS 离线 gate **不等于** XHS 页面加载成功。
- 鼠标可视化需 humanize 或 mouse show API；裸 navigate 无轨迹。
- 验收脚本默认 **finally stop 浏览器**；CreepJS 页无后续动作属 L-12 探针结束，非卡死（`-KeepInstance` 除外）。
