# 不对称隐身架构（99 分目标）

> **历史摘要（2026-06-29）**：本文记录不对称策略的首次接入。详细证据基线、Task、AC 与波次以 **`docs/50-asymmetric-dominance-architecture.md`** 与 **`PLAN.md`** 为准；勿将本文当作执行清单。

Updated: 2026-06-29

## 核心领悟

**目标不是「完全不被检测」**，而是让风控的 **拦截成本 > 放行成本**。

| 风控动作 | 对真实用户成本 | 对我们的成本 | 不对称 |
|----------|----------------|--------------|--------|
| 弹验证码 | 转化率下降 | 打码/API 绕行 | 我们更低 |
| 封 IP | 误伤访客 | 换住宅 IP | 我们更低 |
| 收紧阈值 | 误杀收入 | 切 API/信任继承 | 我们可降级 |

## 三层策略（已接入 Personal Pilot）

### 1. 信任继承（跳出浏览器赛场）

```
手动高质量 bootstrap → ProfileTrustBundle(refresh_token)
    → GraphAPIMailList / 后续 Graph 操作
    → 90%+ 日常操作不经登录页
```

**API**

- `ProfileTrustBundleImportFromJSON(profileId, json)`
- `ProfileTrustBundleGet` / `ProfileTrustBundleSave`
- `GraphAPIMailList(profileId, top)`

### 2. 成本 + 熵欺骗（让流量「不值得拦」）

- **人类时间窗**：`AsymmetricShouldExecute` — 默认 7:00–23:00 本地，深夜禁止
- **Shannon 熵**：`AsymmetricStealthReport` — H≈2.2–3.6 为 human_like；>4.2 为 bot_uniform
- **调度门禁**：`SchedulerRunTaskNow` 在非人类窗口 / 挑战冷却期内自动 block
- **生物噪声**：`humanize.NaturalDelay` — 3% 长暂停 + jitter，已接入 primitive idle/pause

### 3. 动态反馈（阈值收紧时自适应）

```
CAPTCHA/403 → AsymmetricRecordChallenge
    → DeriveFeedback (rotate seed / pause 24h / prefer API)
    → ProfileRotateFingerprintSeed
```

**API**

- `AsymmetricRecordChallenge(profileId, site, type)`
- `AsymmetricStealthReport(profileId)` → stealthScore 0–99
- `ProfileRotateFingerprintSeed(profileId)`

## 浏览器降级路径（必须前端时）

仍用现有栈，但叠加：

| 层 | 实现 |
|----|------|
| 指纹 | Materialize 80/80 + `ApplyGeoLocale(country)` 对齐代理国家 |
| 严格 OAuth | `ApplyStrictAuthPreset`（outlook/auth/claude 标签） |
| 行为 | 34 primitive + bio noise + auth plan 编排 |
| 网络 | WebRTC CDP 探针 + sticky + DNS probe |

## 地理一致性（时区漂移修复）

实例启动时：`BrowserProxyCheckIPHealth` → `ApplyGeoLocale(US→en-US/NY, CN→zh-CN/Shanghai …)`

## 99+ vs 85 分（Sprint K）

| 维度 | 85 分 | 99+ 分 |
|------|-------|--------|
| 评分模型 | `EvaluateCostAsymmetry` 单维 | `EvaluateStealthMatrix` 五层加权 + bonus |
| 等级 | level 字符串 | S+ / S / A / B / C |
| 探针 | detection bundle | `WorkbenchRunStealthProbeSuite`（WebRTC + CreepJS） |
| IP 预算 | 无 | 5 次/天/出口 IP |
| Cookie 继承 | 无 | trust bundle → CDP `Network.setCookie` |
| Token 刷新 | 手动 | 30min 后台 loop |
| 自动反馈 | 手动 rotate | `AsymmetricRecordChallenge` → auto pause/API/seed |

**新 API**

- `AsymmetricStealthReportV2(profileId)` → matrix + target99Plus
- `AsymmetricBootstrapGaps(profileId)` → 距 99+ 的缺口清单
- `AsymmetricApplyFeedbackAuto(profileId)`
- `WorkbenchRunStealthProbeSuite(profileId)`

**验收**

```powershell
./scripts/asymmetric_stealth_gate.ps1
./scripts/asymmetric_autopilot_gate.ps1
```

## 零人工 99+ 全自动（Sprint L）

给 Profile 打标签 `stealth-autopilot` 或 `auto-99`，实例启动后会自动跑完整 pipeline；也可手动调用：

```
AsymmetricAutoReach99Plus(profileId, optsJSON)
```

**自动化步骤（无需人工点击）**

1. 自动绑定池中最佳住宅代理（`IsResidential` + 最低 fraudScore）
2. Materialize 80/80 + ApplyGeoLocale + ApplyStrictAuthPreset 并持久化到 Profile
3. 从环境变量导入 `refresh_token`（`PERSONAL_PILOT_MS_REFRESH_TOKEN` 或 per-profile）
4. 若未运行则自动 `BrowserInstanceStart`，等待 CDP + 环境注入
5. CDP 采集 Cookie + Outlook MSAL localStorage/IndexedDB → `ProfileTrustBundle`
6. 自动 Graph token refresh + `WorkbenchRunStealthProbeSuite`
7. 循环最多 3 轮直到 `target99Plus=true`

**环境变量（一次性部署，运行时零人工）**

| 变量 | 用途 |
|------|------|
| `PERSONAL_PILOT_MS_CLIENT_ID` | OAuth 应用 ID |
| `PERSONAL_PILOT_MS_CLIENT_SECRET` | OAuth secret（可选） |
| `PERSONAL_PILOT_MS_REFRESH_TOKEN` | 全局 refresh_token |
| `PERSONAL_PILOT_MS_REFRESH_TOKEN_<PROFILEID>` | 按实例（`-`→`_`，大写） |

若 Profile 的 userDataDir 里已有 Microsoft 登录态，步骤 5 可自动 harvest refresh_token，无需 env。

## 99 分 vs 85 分

| 维度 | 85 分（Sprint I） | 99 分（本架构） |
|------|-------------------|-----------------|
| 思路 | 补丁对抗 | 成本不对称 + 赛道切换 |
| 登录 | 每次浏览器 | 一次 bootstrap，token 继承 |
| 时间 | 忽略 | 熵 + 人类窗口 |
| 失败 | 静态 | 挑战反馈闭环 |
| 评分 | detection pass | `AsymmetricStealthReport` 多维 |

## 诚实边界

- **不能**保证 Outlook/Claude/Microsoft 永远放行
- **curl_cffi / CloakBrowser / nodriver** 为外部增强层；本仓库提供 Graph/trust/entropy/feedback 编排
- **99** = 在收紧阈值下仍具成本优势，不是数学意义上的 invisible
- 个人学习研究：单账号、低频率 — 天然落在风控「防误杀线」以下

## 推荐工作流（Outlook 示例）

1. 真实设备 + 住宅 IP **手动**完成首次 Microsoft 登录
2. 导出 refresh_token → `ProfileTrustBundleImportFromJSON`
3. 日常：`GraphAPIMailList`；仅 UI 操作时用 Profile + strict-auth 标签启动
4. 每周：`AsymmetricStealthReport` ≥ 92 且 `WorkbenchAccountOutcomeSummary` successRate ≥ 85%
5. 遇 CAPTCHA：`AsymmetricRecordChallenge` → 冷却 24–48h → 必要时 `ProfileRotateFingerprintSeed`

## 验收

```powershell
go test ./backend/internal/asymmetric/... -count=1
go test ./backend/... -count=1
# App: AsymmetricStealthReport(profileId)
```
