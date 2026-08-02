# PLAN.md — 统一执行入口

> **Canonical 规划枢纽**。Live truth 始终在 `docs/02-current-state.md`；文档导航在 `docs/README.md`。
> 本机自用主线维持 `100% / 0% / green`；下文「指纹/反检测优化轨道（49–56）」与「三方横评轨道（47–48）」为**独立推进轨道**。

Updated: 2026-07-17 (99+ matrix + evidence hardening)

---

## 1. 轨道总图（49–56）

| 层 | 文档 | 主题 | 状态 |
| --- | --- | --- | --- |
| 战术 | [`docs/49`](docs/49-fingerprint-proxy-optimization-plan.md) | hook 真实性 | W0–A5 ✅ · C2/C3/C4 ✅ · C4 AC2 partial ✅ |
| 战略 | [`docs/50`](docs/50-asymmetric-dominance-architecture.md) | 不对称支配 | A1/A3/B1/C1 ✅ · module/SW ✅ · local_session trust ✅ |
| 广度 | [`docs/51`](docs/51-fingerprint-surface-coverage-guide.md) | S1–S14 | **全部 ✅** |
| 相干 | [`docs/52`](docs/52-device-persona-coherence-engine.md) | 人格 | DP1–DP5 ✅ · geo+Windows force ✅ |
| 行为 | [`docs/53`](docs/53-behavior-biometrics-l5-execution-bridge.md) | L5 | L5-1..4 ✅ · OS BioNoise ✅ · cadence/entropy cold-start ✅ |
| 规模 | [`docs/54`](docs/54-concurrency-process-hygiene.md) | 多开 | CP1–CP6 ✅ |
| 度量 | [`docs/55`](docs/55-account-health-observability.md) | 纵向 | AH1–AH5 ✅ |
| 边界 | [`docs/56`](docs/56-boundary-closure-and-commercial-parity.md) | 边界 | F1/B1/E1/T1/P1/T2/R1 ✅ · CreepJS structured ✅ |

### 1.1 2026-07-17 本轮关项

| ID | 摘要 | 状态 |
| --- | --- | --- |
| Trust local_session | `ProfileTrustBundleBootstrapLocal` + HasLocalContinuity | ✅ |
| DNS policy score | host-resolver + exitIP + WebRTC clean → consistent | ✅ |
| Cold-start entropy/cadence | seeded human band + AccountSuccess fallback | ✅ |
| Probe cache | cached exit IP in stealth/detection paths | ✅ |
| CreepJS structured | DOM/regex/heuristic 三级 + poll | ✅ |
| Drift gate | `scripts/fingerprint_drift_gate.ps1` 10-capture | ✅ |
| TLS/UA path | `scripts/tls_ua_coherence_probe.ps1` peet.ws + unit | ✅ |
| Live Stealth 99+ | **101.7 / S+ / target99Plus=true** | ✅ |

---

## 2. 仍未做（诚实）

### 本地残余（非阻塞）

| 项 | 说明 |
| --- | --- |
| Persona 真机采样扩库 | public-spec 机型，非本机 probe |
| Browser-context JA3 | peet.ws 路径已通；headed CDP ClientHello 更深证明仍可选 |
| Drift cross-session restarts | 默认 single-instance multi-capture 已过；`-CrossSessionRestarts` 仍可能撞 start lock |
| ops UA 白名单统一 | 仍有 PersonalPilot/1.0 等 ops 出口 |
| drop flags 审计字段 | materialize 已 drop，operator 报告仍弱 |
| Scheduler cancel/pause/history | 任务中心控制面 |
| 订阅刷新桌面通知 | lastError 可查，主动告警卡未接 |

### 2026-07-17 内部 P0 补强

| 项 | 状态 |
| --- | --- |
| proxy_vs_exit 真校验 | ✅ declared name/group vs exit IP |
| Workbench tab 绑定 | ✅ OpenUrl 新开 tab + HTTP tabId + ExecuteActions tabId |
| full workflow smoke 脚本 | ✅ `scripts/full_workflow_smoke.ps1` |

### 外部条件阻塞

| 项 | 阻塞 |
| --- | --- |
| 横评分刷新 | BitBrowser 额度 + 新 deep/missing raw |
| 住宅节点 / DNS-token / DE / AdsPower API | 外部条件 |
| 真 OAuth refresh_token | 现为 local_session 连续性 |
| CAPTCHA/SMS/Email live | 真实凭证 |

---

## 3. 门禁

```powershell
go test ./backend/internal/behavior/... ./backend/internal/browser/... `
  ./backend/internal/detection/... ./backend/internal/proxy/... `
  ./backend/internal/transport/... ./backend/internal/trust/... `
  ./backend/internal/platformpack/... ./backend/internal/asymmetric/... -count=1
go build ./backend/
.\scriptsingerprint_drift_gate.ps1 -Runs 10 -NoProxy
.\scripts	ls_ua_coherence_probe.ps1 -ProxyUrl http://127.0.0.1:7897 -ExpectedUAMajor 139
.\scripts\concurrency_smoke.ps1
.\scriptsccount_health_observability_gate.ps1
```

### 最新 99+ 证据

- `data/reports/app-instance-live/fingerprint-stealth-99plus-exec-1784292199772.json` → Stealth **101.7 S+**
- `data/reports/fingerprint-drift/fingerprint-drift-1784296043332.json` → **passed** 10/10
- `data/reports/tls-ua-coherence/tls-ua-coherence-1784295310559.json` → **passed**
- `data/reports/full-workflow-smoke/full-workflow-smoke-1784300724839.json` → **passed** tab-bound

---

## 4. 接手最短路径

1. `docs/02-current-state.md` → 本文件
2. 有额度：刷横评分 / 真 OAuth TrustBundle
3. 可选：headed browser-context JA3 / Persona 真机采样
