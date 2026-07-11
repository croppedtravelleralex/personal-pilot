# PLAN.md — 统一执行入口

> **Canonical 规划枢纽**。Live truth 始终在 `docs/02-current-state.md`；文档导航在 `docs/README.md`。
> 本机自用主线维持 `100% / 0% / green`；下文「指纹/反检测优化轨道（49–56）」与「三方横评轨道（47–48）」为**独立推进轨道**。

Updated: 2026-07-11 (本地小残余收口)

---

## 1. 轨道总图（49–56）

| 层 | 文档 | 主题 | 状态 |
| --- | --- | --- | --- |
| 战术 | [`docs/49`](docs/49-fingerprint-proxy-optimization-plan.md) | hook 真实性 | W0–A5 ✅ · C2/C3/C4 ✅ |
| 战略 | [`docs/50`](docs/50-asymmetric-dominance-architecture.md) | 不对称支配 | A1/A3/B1/C1 ✅ · module/SW ✅ |
| 广度 | [`docs/51`](docs/51-fingerprint-surface-coverage-guide.md) | S1–S14 | **全部 ✅** |
| 相干 | [`docs/52`](docs/52-device-persona-coherence-engine.md) | 人格 | DP1–DP5 ✅ |
| 行为 | [`docs/53`](docs/53-behavior-biometrics-l5-execution-bridge.md) | L5 | L5-1..4 ✅ · OS BioNoise ✅ |
| 规模 | [`docs/54`](docs/54-concurrency-process-hygiene.md) | 多开 | CP1–CP6 ✅（CP5=ADR 不接入 pool） |
| 度量 | [`docs/55`](docs/55-account-health-observability.md) | 纵向 | AH1–AH5 ✅ |
| 边界 | [`docs/56`](docs/56-boundary-closure-and-commercial-parity.md) | 边界 | F1/B1/E1/T1/P1/T2/R1 ✅ |

### 1.1 本轮新关项

| ID | 摘要 | 状态 |
| --- | --- | --- |
| DP1 残余 | PersonaID SQLite 列 + migration 17 | ✅ |
| DP3 | Geo 坐标表 + `Emulation.setGeolocationOverride` | ✅ |
| DP4 | `EvolvePersona` 字体只增/慢老化 | ✅ |
| DP5 | `AssignPersonaIDDiverse` 池多样性 | ✅ |
| S4–S14 | 全指纹面注入 | ✅ |
| module/SW | module Worker + ServiceWorker.register bootstrap | ✅ |
| Harvest LS | harvest 写入 LocalStorage/SessionStorage | ✅ |
| L5 dwell | `DwellFromSeed` / `BioNoiseConfigFromSeed` | ✅ |
| CP3/CP4 | orphan reconcile + detached 10m 超时 | ✅ |
| CP5 | ADR 明确不接入 pool | ✅ |
| AH1/AH2 | `AccountHealthTrend` + lifecycle 接线 | ✅ |
| R1 | RecordingDetailModal 步骤时间线 | ✅ |
| C2/C3 | ProxyIPMonitor 并发5 + VerifyV2 bootstrap | ✅ |
| Ops 同出口 | `AssertSameExitIP` | ✅ |
| **49-C4** | `ChromeMajorTLSBaseline` → sing-box/xray + tlsUaCoherent | ✅ |
| **CP6** | `concurrency_smoke.ps1` v2（N≥10 + CP1–CP5 断言） | ✅ |
| **AH3–AH5** | 归因报告 / 挑战率闭环 / gate + Dashboard 趋势 | ✅ |
| **cadence.yaml** | `platformpack.LoadCadence` → `PlanDailySession` | ✅ |
| **L5 OS BioNoise** | inputplane/wininput 同源 seed 轨迹 | ✅ |

---

## 2. 仍未做（诚实）

### 本地残余（极小 / 非阻塞）

| 项 | 说明 |
| --- | --- |
| **Persona 真机采样扩库** | 现为 public-spec 标注机型，非本机 probe 采集 |
| **C4 AC2 live** | tls.browserleaks.com 经代理 JA3↔UA ≥2/3 需 headed live |

### 外部条件阻塞

| 项 | 阻塞 |
| --- | --- |
| 横评分刷新 / 10-run drift | BitBrowser 额度 + 新 raw |
| DNS-token / DE 节点 / AdsPower API | 外部条件 |
| CAPTCHA/SMS/Email live | 真实凭证 |

---

## 3. 门禁

```powershell
go test ./backend/internal/behavior/... ./backend/internal/browser/... `
  ./backend/internal/detection/... ./backend/internal/proxy/... `
  ./backend/internal/transport/... ./backend/internal/trust/... `
  ./backend/internal/platformpack/... -count=1
go build ./backend/
.\scripts\concurrency_smoke.ps1
.\scripts\account_health_observability_gate.ps1
```

---

## 4. 接手最短路径

1. `docs/02-current-state.md` → 本文件
2. 有额度：刷横评分 / C4 live JA3
3. Persona 真机采样（可选）
