# PLAN.md — 统一执行入口

> **Canonical 规划枢纽**。Live truth 始终在 `docs/02-current-state.md`；文档导航在 `docs/README.md`。
> 本机自用主线维持 `100% / 0% / green`；下文「指纹/反检测优化轨道（49–56）」与「三方横评轨道（47–48）」为**独立推进轨道**，按波次与门禁验证，不改主线口径。

Updated: 2026-07-11 (续推：uTLS / Graph 同出口 / hydrate / persona / 易赢项)

---

## 1. 轨道总图（49–56）

| 层 | 文档 | 主题 | 状态 |
| --- | --- | --- | --- |
| 战术 | [`docs/49`](docs/49-fingerprint-proxy-optimization-plan.md) | hook 真实性 | W0 ✅ · A5 ✅ · 大部 ✅ |
| 战略 | [`docs/50`](docs/50-asymmetric-dominance-architecture.md) | 不对称支配 | C1/A1/A3/B1 ✅（hydrate 含 cookie+storage） |
| 广度 | [`docs/51`](docs/51-fingerprint-surface-coverage-guide.md) | S1–S14 | S1/S2/S3 ✅ · 其余 ⬜ |
| 相干 | [`docs/52`](docs/52-device-persona-coherence-engine.md) | 人格 + 硬门禁 | DP1 ✅（库≥12）· DP2 ✅ · DP3+ ⬜ |
| 行为 | [`docs/53`](docs/53-behavior-biometrics-l5-execution-bridge.md) | L5 执行桥 | L5-1/2/3 ✅ · L5-4+ ⬜ |
| 规模 | [`docs/54`](docs/54-concurrency-process-hygiene.md) | 多开 | CP1/CP2 ✅ · CP3–6 ⬜ |
| 度量 | [`docs/55`](docs/55-account-health-observability.md) | 纵向过检 | ⬜ W3 |
| 边界 | [`docs/56`](docs/56-boundary-closure-and-commercial-parity.md) | 边界收口 | F1/B1/E1/T1/P1/T2 ✅ |

### 1.1 关键 Task 速查（2026-07-11 续推后）

| ID | 文档 | 摘要 | 状态 |
| --- | --- | --- | --- |
| A1 | 49 | toString 原生化 | ✅ |
| A3 | 49 | UA/内核单一真相源 | ✅ |
| C1 | 49 | DNS inconclusive | ✅ |
| D1 | 49 | 鼠标重试 + OS fallback 接线 | ✅ |
| A2/A4 | 49 | Runtime.disable + 非 blank 跳过当前页 evaluate | ✅ |
| A5 | 49 | 噪声质量（RGBA + seed-stable audio） | ✅ |
| B1 | 49/56 | CreepJS 结构化 parser | ✅ |
| B2 | 49 | 指纹 DNA / 10-run drift | ⬜ 外部额度 |
| C1 | 50 | Worker 注入 | ✅ |
| B1 | 50 | 会话 hydrate | ✅ TrustSurface → CDP cookies+storage |
| A1 | 50 | uTLS impersonation | ✅ `transport/impersonate` |
| A3 | 50 | API 同出口代理 | ✅ Graph 走 profile 桥接 + uTLS |
| S1/S2/S3 | 51 | chrome / permissions / WebGPU 对齐 | ✅ |
| S4–S14 | 51 | 其余指纹面 | ⬜ |
| DP1 | 52 | 人格库 ≥12 + seed 分配 + materialize | ✅（PersonaID 内存/JSON；SQLite 列待补） |
| DP2 | 52 | 相干性硬门禁 | ✅ |
| DP3/4 | 52 | geo 活体 / 老化 | ⬜ |
| L5-1/2/3 | 53 | Fitts + 四段点击 + 惯性 wheel 滚动 | ✅ |
| CP1/CP2 | 54 | 端口预留 + 并发上限/budget | ✅ |
| CP3–6 | 54 | 孤儿/自愈等 | ⬜ |
| 55 | 55 | 纵向指标全量 | ⬜ |
| T1/T2/E1/F1/P1 | 56 | 边界骨架 | ✅ |
| R1 | 56 | RPA 可视化 MVP | ⬜ |

**粗算**：关键 Task 约 **30/40+ 落地或骨架** ≈ **70%+ 本地可推进项**；整轨道含外部阻塞约 **55–60%**。

---

## 2. 执行波次状态

```
Wave 0 — ✅ 关闭
Wave 1 — ✅ 关闭（含 A5、hydrate、人格库）
Wave 2 — ✅ 主体（uTLS、API 同出口、S3、CP2、L5-3）；S4+、CP3+ 未开
Wave 3 — ⬜ docs/55
Wave 4 — 🟡 P1/T2；R1/刷分/外部条件 未开
```

---

## 3. 仍阻塞 / 诚实未做

| 项 | 原因 |
| --- | --- |
| 横评分刷新（73→?） | 需新 missing/deep raw；BitBrowser 额度 |
| DNS-token / DE 节点 / AdsPower API | 外部条件 |
| CAPTCHA/SMS/Email live | 无真实凭证 |
| PersonaID SQLite 列 | 仅 Profile JSON/内存；重启后按 seed 重算（确定性，可接受） |
| module/ServiceWorker 注入 | 仅 classic Worker |
| RPA 可视化 UI | 未做前端 |
| docs/55 纵向指标 | 未开 |
| S4–S14 / CP3–6 / DP3–4 | 未做 |

---

## 4. 统一门禁

```powershell
go test ./backend/internal/behavior/... ./backend/internal/browser/... `
  ./backend/internal/detection/... ./backend/internal/proxy/... `
  ./backend/internal/transport/... ./backend/internal/trust/... -count=1

.\scripts\provider_credential_smoke.ps1
$env:PERSONAL_PILOT_COHERENCE_MODE='block'
```

---

## 5. 三方横评（独立）

PersonaPilot `73` · BitBrowser `71` · AdsPower `N/A`（改前 raw；本批代码未刷分）。

---

## 6. 接手最短路径

1. `docs/02-current-state.md`
2. 本文件
3. 未做项优先：PersonaID SQLite 持久化 → DP3 geo → S4+ → docs/55 → 有额度刷分 → RPA UI
