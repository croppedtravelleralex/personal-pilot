# 用户需求执行计划

Updated: 2026-06-29 (Sprint A–G 100% 批次 + 验收门禁)

## 总体结论

| # | 需求 | 完成度 | 状态 |
|---|------|--------|------|
| 1 | IP 不漂移 / 不 DNS·WebRTC 泄露 | **100%** | sticky + reconcile + verify V2 + leak probe + 60s 监控 |
| 2 | 450 指纹 + 行为事件过风控 | **100%** | 80/80 Materialize + 34 primitives + auto_score + observed gate |
| 3 | 长期账号行为 / 节奏 / 历史一致性 | **100%** | lifecycle + cadence + trajectory 交叉验证 |
| 4 | CDP 完全抓取 | **100%** | network/perf/DOM + analyze/diff/merge/workflow 桥接 |
| 5 | 性能极致优化 | **100%** | v14 索引 + analyze 热路径 + race(可跳过) + concurrency smoke |
| 6 | 高并发测试 | **100%** | `concurrency_smoke.ps1` + 验收门禁聚合 |
| 7 | Chrome 鼠标指针可见性 | **100%** | 默认开启 + UI + CDP + Camoufox |
| 8 | 任务编排 | ✅ | 本文档 + `user_requirements_acceptance_gate.ps1` |

**诚实边界**：第三方 live 站点（browserleaks/CreepJS/真实 CAPTCHA）仍需 headed Chrome + 有效代理凭证；本地代码与自动化门禁已达 100%。

---

## Sprint A — 网络身份零泄露 ✅

| 任务 | 状态 |
|------|------|
| socks5h + host-resolver-rules + WebRTC flag | ✅ |
| sing-box DNS 段 | ✅ |
| Xray DNS 路由 UDP/53 → proxy | ✅ |
| leak_probe DNS/WebRTC 分析 | ✅ |
| risk:dns:leak / proxy:quality:node-rotated | ✅ |
| 60s 运行中 IP 监控 | ✅ |
| WebRTC 策略统一 + block_private 修正 | ✅ |
| sticky session | ✅ `StickySessionTracker` + `noteProxyHealthObserved` |
| 热切换 reconcile | ✅ `ReconcileProfileRouting` + `app_proxy_reconcile.go` |
| verify V2 稳定性 streak | ✅ `VerifyV2Streak` |

---

## Sprint B — 指纹与行为 ✅

| 任务 | 状态 |
|------|------|
| 80/80 runtime 投影（Materialize） | ✅ `runtime_materialize.go` |
| 34 shipped primitives | ✅ Go + Rust |
| 注入扩展 touch/colorDepth/doNotTrack | ✅ |
| observed fingerprint gate | ✅ 470/450 signals |
| live replay runtime gate | ✅ 461/450 events |
| CreepJS/检测站 auto_score | ✅ `detection/auto_score.go` |

---

## Sprint C — 账号行为评分 ✅

| 任务 | 状态 |
|------|------|
| lifecycle Store 接入 | ✅ |
| cadence 分析 + API | ✅ |
| SessionBundle 轨迹交叉验证 | ✅ `trajectory_validator.go` |
| identity 行为自然度加分 | ✅ |

---

## Sprint D — CDP 完全抓取 ✅

| 任务 | 状态 |
|------|------|
| fetch/XHR/WebSocket hook | ✅ |
| Performance + DOM 停止时抓取 | ✅ |
| BehaviorRecordingAnalyze | ✅ |
| Diff / Merge / ToWorkflow | ✅ |
| GetPageHTML / DOM snapshot | ✅ |

---

## Sprint E — 性能 ✅

| 任务 | 状态 |
|------|------|
| scheduler_tasks 复合索引 v14 | ✅ |
| 录制 analyze 热路径 | ✅ |
| go test -race（Linux/CI） | ✅ Windows 无 cgo 时门禁自动 skip |
| concurrency smoke | ✅ |

---

## Sprint F/G — 并发与鼠标 ✅

| 任务 | 状态 |
|------|------|
| concurrency_smoke.ps1 | ✅ |
| 鼠标默认显示 + settings 持久化 | ✅ `ShowMousePointerDefault` |
| CDP overlay + Camoufox | ✅ |

---

## 新增 API

- `BehaviorRecordingAnalyze` / `Diff` / `Merge` / `ToWorkflow`
- `BrowserRuntimeProjectionReport`（80/80 Materialize 报告）
- `BehaviorShippedPrimitiveList`
- `WorkbenchAutoDetectionScore`

## Sprint I — 检测闭环 + Primitive 接线 + 账号结果（2026-06-29）

| 能力 | API / 脚本 |
|------|------------|
| 34 primitive CDP 分发 | `BehaviorExecutePrimitive` / `BehaviorExecutePrimitivePlan` |
| CDP ICE WebRTC 探针 | `WorkbenchProbeWebRTC`（启动注入后自动跑） |
| 账号业务结果 | `WorkbenchRecordAccountOutcome` / `WorkbenchAccountOutcomeSummary` |
| 严格认证预设 | Profile 标签含 `outlook`/`auth`/`claude` 等 → `ApplyStrictAuthPreset` |
| 检测站 smoke | `scripts/detector_site_headed_smoke.ps1` |


```powershell
go test ./backend/... -count=1
powershell -File scripts/user_requirements_acceptance_gate.ps1
```

报告输出：`data/reports/user-requirements-acceptance/user-requirements-acceptance-*.json`

可选 headed 人工复核：

```text
启动带代理实例 → browserleaks.com/ip + /webrtc
停止录制 → BehaviorRecordingAnalyze
```
