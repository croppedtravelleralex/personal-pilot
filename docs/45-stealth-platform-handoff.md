# Stealth / 平台自动化交接（PGS + XBS）

Updated: 2026-06-30 (Asia/Shanghai)

## 下一位接手先读

1. `docs/02-current-state.md` — live truth（含代理与 live 验收边界）
2. `docs/46-global-99plus-architecture.md` — **全局 99+ 最优解（L0–L6 + Input Plane）**
3. `docs/44-dual-track-99plus-spec.md` — PGS 硬门槛 + XBS 差异包规范
4. 本文 — 命令、实例、**什么算通过 / 什么不算**
5. `docs/29-proxy-supply-chain.md` §5.3 — Chrome 代理参数与 DNS 硬ening
6. `docs/43-capability-scenario-test-suite.md` — 能力雷达设计（离线 34/34 已通过，≠ 平台 live）

## 双轨含义

| 轨道 | 范围 | 验收脚本 |
|------|------|----------|
| **PGS** | 通用 stealth：sing-box 桥、注入门控、API 审计、script 禁令 | `scripts/platform_99_gate.ps1 -Track PGS` |
| **XBS** | 平台差异包（如 `platform-packs/xhs/`）+ 该平台 live 场景 | `scripts/platform_99_gate.ps1 -Track XBS -Platform xhs` |

**原则**：可复用逻辑必须在 PGS；XBS 只保留 URL、选择器、节奏等平台差异。

## 当前状态（诚实口径）

### 已落地（代码 + 离线 gate + XHS live）

- Phase 0 硬门槛 H-P1~H-P5（见 `docs/44-dual-track-99plus-spec.md`）
- 通用 plans：`plans/account-login-v1.yaml` 等（含 `inputMode: auto`）
- XHS pack：`platform-packs/xhs/`
- G0 Input Plane：`backend/internal/behavior/inputplane/` + workbench `inputPlane` 审计字段
- G1 CDP-minimal / CreepJS probe / Camoufox flag 过滤（见 `docs/46-global-99plus-architecture.md`）
- Capability 雷达：`scripts/capability_scenario_suite.ps1 -Tier all -GenerateRadar` → **34/34 passed**（API/契约层）
- `scripts/platform_99_gate.ps1 -Track all` → **29/29 PASS**（PGS + XBS 结构门禁）
- **XHS live**：`scripts/xhs_live_acceptance.ps1` → **12/12 PASS**（2026-06-30；UDEAL + `pp_via_ssh=panda`；CreepJS trust=100）

### 已知限制 / 不得误读

| 项 | 说明 |
|----|------|
| CreepJS 页面 UI | L-12 导航到 CreepJS 后 **只等 3s** 即 CDP 采样；页面可能仍显示 `FP ID: Computing...`，API 侧可走启发式 trust |
| 验收后浏览器 | 默认 **finally 会 stop 实例**；调试 CreepJS 页用 `-KeepInstance` |
| 多 core 分裂 | 验收 **默认复用** `19876` LaunchServer；勿重复 spawn harness（`-ForceNewCore` 仅隔离调试） |
| proxy hash mismatch | UI 改代理后须 **stop → 更新 profile → start**；运行中改绑会导致 workbench 409 |
| `--host-resolver-rules` | 顶部黄条为预期；可选标签 `skip-host-resolver-rules` |
| 无 TrustBundle | 矩阵 honest cap ~88；99+ 仍依赖 L0 继承或 live CreepJS ≥85 |

### 历史未通过项（已修复或归档）

| 项 | 原现象 | 现状 |
|----|--------|------|
| XHS explore 可达 | `ERR_NO_SUPPORTED_PROXIES` / chrome-error | 已修 `socks5://` scheme + SSH 桥 |
| 验收 spawn 多 core | `pid=0` / 状态分裂 | 已改 `Get-OrStartCapabilityHarness` 复用 19876 |
| workbench navigate 假阳性 | HTTP 200 + chrome-error URL | 已校验 `Test-ValidNavigationUrl` |

## XHS 参考实例

| 字段 | 值 |
|------|-----|
| Profile ID | `a4b108a0-afb3-4e93-843e-f5ef22278b9d` |
| 手机 | `19202757042` |
| UserDataDir | `data/browser/user-data/xhs-19202757042` |
| 代理 | UDEAL SOCKS5；本机直连握手失败，live 需 URL 追加 `?pp_via_ssh=panda` |
| Launch API | `http://127.0.0.1:19876` |

## 常用命令

```powershell
# 重建 sidecar（代理修复后必做）
cd backend
go build -o ..\bin\personal-pilot-core.exe ./cmd/personal-pilot-core/

# PGS / XBS 离线门禁
.\scripts\platform_99_gate.ps1 -Track PGS
.\scripts\platform_99_gate.ps1 -Track XBS -Platform xhs

# XHS live 验收（代理修复后；默认复用 LaunchServer；ViaSSH 按需）
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda -StartTimeoutSec 300

# 验收后保留浏览器（例如手动看 CreepJS 页）— 默认会 stop 实例
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda -KeepInstance

# 强制 spawn 新 core（仅隔离调试；勿与桌面 sidecar 并行）
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda -ForceNewCore

# 实例状态
Invoke-RestMethod "http://127.0.0.1:19876/api/instances/status?profileId=a4b108a0-afb3-4e93-843e-f5ef22278b9d"

# 全量能力雷达（离线）
.\scripts\capability_scenario_suite.ps1 -Tier all -GenerateRadar
```

## Live 验收纪律（强制）

1. **页面可达**：`pageUrl` 以 `https://` 目标域开头，且不含 `chrome-error`。
2. **代理生效**：实例启动参数为 `socks5://127.0.0.1:<bridge>`，不是 `socks5h://`。
3. **注入就绪**：`injectionReady=true` 后再做 workbench 写操作。
4. **拟人动作**：登录/养号场景用 humanize type/click/scroll，禁止默认 `script`（除非 profile 标签 `allow-script-bypass`）。
5. **鼠标可见**（若用户要求）：humanize 动作后检查 overlay，或显式调用 mouse show API。
6. **审计**：敏感 API 调用可在 `GET /api/audit/logs` 核对。
7. **CreepJS（L-12）**：`POST /api/workbench/stealth-probe` 会打开 CreepJS 页；UI 可能仍在 Computing，以 API `creepTrust` 为准（≥85）。
8. **清理**：验收脚本 `finally` 默认 stop 实例；勿把「停在 CreepJS 无动作」当卡死，除非用了 `-KeepInstance`。

## `--host-resolver-rules` 说明

来源：`AppendProxyHardeningArgs`（`backend/internal/browser/proxy_launch.go`）。

含义：除 `127.0.0.1` 外禁止本机 DNS 解析，逼流量走 sing-box 远程解析，防 DNS 泄漏。

副作用：Chromium 顶部黄条「不受支持的命令行标记」；有指纹/自动化检测张力，见 `docs/42-asymmetric-stealth-architecture.md` 权衡说明。

远程 DNS **不**依赖 Chrome 的 `socks5h` scheme，而依赖 sing-box 出站 + 上述 resolver rules。

## 实例标签

| 标签 | 用途 |
|------|------|
| `auto-99` | Stealth Autopilot |
| `allow-script-bypass` | 允许 workbench `script`（仅调试） |
| `allow-direct-fallback` | 允许桥接失败时切 `direct://` |
| `show-mouse-pointer` | 调试时开启页面内鼠标圆点 overlay |
| `skip-host-resolver-rules` | 跳过 `--host-resolver-rules`（降低 DNS 硬ening 黄条，有泄漏风险） |

## 下一步（按优先级）

1. XHS **登录** bootstrap 去掉 `script`，对齐 `account-login-v1` + Input Plane S0
2. CreepJS 探针：可选延长等待或轮询直到 trust 文本出现（减少启发式 100 分）
3. 评估 `host-resolver-rules` 是否改为「仅桥接模式启用」以降低 flag 暴露
4. G2：XHS Cookie → ProfileTrustBundle（L0 继承）

## 相关代码索引

| 能力 | 路径 |
|------|------|
| Chrome 代理规范化 | `backend/internal/browser/proxy_launch.go` |
| sing-box 桥 + 端口 hold | `backend/internal/proxy/singbox.go`, `port_reserve.go` |
| 注入门控 | `backend/app_environment_injection.go`, `Profile.injectionReady` |
| API 审计 | `backend/internal/launchcode/server_audit.go` |
| script 禁令 | `backend/app_launchcode.go` → `validateWorkbenchScriptPolicy` |
| 鼠标 overlay | `backend/app_instance_mouse.go`, `cdp_executor.go` |
