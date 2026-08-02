# 全局 99+ 最优架构（L0–L6）

Updated: 2026-06-30

> **定位**：Personal Pilot 在「画像统一 + 住宅 IP + 真实环境」下的**全局最优解**，不是「100% 不被检测」，而是在收紧阈值下仍具备 **成本不对称优势**（见 `docs/42-asymmetric-stealth-architecture.md`）。
>
> **与现有文档关系**
> - 评分引擎：`backend/internal/asymmetric/stealth_matrix.go` → `AsymmetricStealthReportV2`
> - 双轨验收：`docs/44-dual-track-99plus-spec.md`（PGS / XBS）
> - 交接纪律：`docs/45-stealth-platform-handoff.md`
> - 本文件 = **分层栈 + 输入平面 + 实施顺序** 的 canonical 蓝图

---

## 1. 99+ 的精确定义

| 维度 | 99+（S+） | 不是 99+ |
|------|-----------|----------|
| 数学 | `EvaluateStealthMatrix` ≥ **99.0** + bonus | 离线 capability 34/34 |
| 探针 | CreepJS trust ≥ **85**；WebRTC/DNS clean | 仅 detection bundle pass |
| Live | 目标站 `pageUrl` 为 **https 目标域**，非 chrome-error | navigate HTTP 200 |
| 信任 | **TrustBundle 有效** 或平台 Session 继承就绪 | 纯浏览器补丁 |
| 输入 | 高敏动作经 **Input Plane Router → OS** | 全链 CDP `Input.dispatch*` |
| 运营 | 人类时间窗内；挑战率 ≤ **5%**；无 pause 冷却 | 24h 高频硬刚 |

**硬上限（诚实边界）**

- 无 Trust/API 继承时，矩阵缺口清单会写：`cap ~88 without API trust inheritance`。
- 因此 **99+ 的全局最优路径 = L0 信任继承优先 + L1–L6 兜底**，而非「全 OS 输入接管一切」。

---

## 2. 七层栈（L0–L6）

```text
┌──────────────────────────────────────────────────────────────────┐
│ L0 业务 / 信任继承     API · Session · Cookie · Token refresh     │  权重 25%
├──────────────────────────────────────────────────────────────────┤
│ L1 身份 / 网络         住宅 IP · sticky · geo · DNS/WebRTC/TLS    │  权重 22%
├──────────────────────────────────────────────────────────────────┤
│ L2 引擎 / 指纹         单一真相源 · Materialize · 禁双层 patch   │  权重 22%
├──────────────────────────────────────────────────────────────────┤
│ L3 协议 / 控制平面     CDP-minimal · 注入门控 · 审计 · 禁 script  │  （计入 L2/L5）
├──────────────────────────────────────────────────────────────────┤
│ L4 输入平面            CDP 定位 + 分级执行（OS / 引擎 humanize）   │  （计入 L2 行为）
├──────────────────────────────────────────────────────────────────┤
│ L5 行为 / 熵           Fitts · 打字 · 滚动 · 时间窗 · Shannon 熵   │  权重 16%
├──────────────────────────────────────────────────────────────────┤
│ L6 反馈 / 降级         挑战检测 → pause · rotate · API 切换        │  权重 15%
└──────────────────────────────────────────────────────────────────┘
```

### L0 — 业务 / 信任继承（最优解的第一优先级）

**原则**：能不出浏览器就不出浏览器。

| 能力 | 已有 API / 模块 | 99+ 要求 |
|------|-----------------|----------|
| Microsoft Graph | `ProfileTrustBundle*`、`GraphAPIMailList` | refresh 30min 后台 loop |
| 平台 Cookie 继承 | CDP `Network.setCookie` | XBS pack 定义域 + harvest 流程 |
| 平台 Token（若有） | 待扩展 `platform-packs/*/trust.yaml` | 小红书若仅有 Cookie，则 L0 = Cookie + 设备指纹一致 |
| 操作路由 | **Operation Matrix**（见 §4） | 读/列表/通知 → API；UI 独占 → 浏览器 |

**99+ 门禁**：`TrustBundleValid && (GraphTokenFresh || CookiesInjected)` + `APIFirstReady` → bonus **+2.5**。

### L1 — 身份 / 网络

| 子项 | 实现 | 99+ 门槛 |
|------|------|----------|
| 住宅出口 | sing-box / SSH 桥；`IsResidential` | 必选；非住宅 → 缺口清单 |
| Geo 一致 | `ApplyGeoLocale(country)` | timezone/locale/Accept-Language 对齐 |
| DNS | `--host-resolver-rules` + sing-box 远程解析 | `DNSConsistent=true`；可用标签 `skip-host-resolver-rules` 仅调试 |
| WebRTC | 注入 + CDP 探针 | `WebRTCClean=true` |
| TLS/H2 | `transport/` + xray/singbox outbound | 按 runtime family 选模板（Phase 6.3 待完） |
| IP 预算 | 5 次/天/出口 | `IPBudgetHeadroom=true` |

**Chrome 代理**：必须 `socks5://127.0.0.1:<bridge>`，禁止 `socks5h://`（`proxy_launch.go`）。

### L2 — 引擎 / 指纹（单一真相源）

**全局规则**：一个 Profile **只选一条引擎链**，禁止混用。

| 轨道 | 引擎 | 适用 |
|------|------|------|
| **主轨 A**（XHS 推荐） | fingerprint-chromium + seed Materialize | Chrome 生态站点、Client Hints 一致 |
| **副轨 B**（A/B） | Camoufox + Juggler/Playwright | 指纹/协议压测；Firefox 兼容风险自担 |

**99+ 指纹 checklist**

- [ ] Materialize **80/80**（`Runtime80of80`）
- [ ] `ApplyGeoLocale` 与代理国家一致
- [ ] `navigator.webdriver` hidden + 无 Playwright 残留
- [ ] **禁** JS 注入叠 Camoufox C++ patch 同页
- [ ] CreepJS trust ≥ **85**（`WorkbenchRunStealthProbeSuite`）

**反模式**：Chromium flag（`--fingerprint-*`）打到 Camoufox；双层 lies → CreepJS 崩盘。

### L3 — 协议 / 控制平面

| 规则 | 实现 |
|------|------|
| 注入就绪门控 | H-P3 `injectionReady` |
| Workbench 写操作 | 必须 `runningProfileForWorkbench` + audit |
| 默认禁 script | H-P5；仅标签 `allow-script-bypass` |
| CDP 面最小化 | 定位/DOM/截图可用；**高敏输入不走 CDP dispatch** |
| Navigate 验收 | `IsNavigationErrorPage` — chrome-error **FAIL** |

**CDP-minimal 模式（待实现 P1）**

- 连接后：Page + DOM + Input（只读坐标）+ Network（cookie）
- 避免长期 `Runtime.enable` 监听；一次性 evaluate 后 detach
- 参考：nodriver / patchright 的「少开域」策略

### L4 — 输入平面（全局最优的核心增量）

**架构**：`Input Plane Router` — **CDP 只负责「看」与「算坐标」，「动手」分级路由**。

```text
                    Workbench Action (click/type/scroll)
                                    │
                                    ▼
                         ┌─────────────────────┐
                         │  InputPlaneRouter   │
                         │  sensitivity + ctx  │
                         └──────────┬──────────┘
                ┌──────────────────┼──────────────────┐
                ▼                  ▼                  ▼
         ┌────────────┐    ┌────────────┐    ┌────────────┐
         │ OS (win32) │    │ CDP humanize│   │  hybrid    │
         │ isTrusted  │    │ 低敏/批量   │   │ OS click   │
         │ headed 前台│    │ scroll 等   │   │ CDP type*  │
         └────────────┘    └────────────┘    └────────────┘
                * 仅非 Arkose/非 Lexical 编辑器
```

#### 灵敏度分级（默认策略）

| 级别 | 动作示例 | 执行器 | 前置条件 |
|------|----------|--------|----------|
| **S0 OS** | 登录按钮、OTP、验证码框、支付、OAuth consent | `wininput` | headed、窗口前台、active tab |
| **S1 CDP-humanize** | Feed 滚动、普通链接、关闭弹窗 | `CDPExecutor` + Fitts/贝塞尔 | `injectionReady` |
| **S2 CDP-raw** | 禁止默认；仅 `allow-script-bypass` 调试 | — | — |
| **S3 API** | 读邮件、读通知、列表 API | Graph / 平台 API | L0 trust |

#### wininput 接入点（P0 实施）

复用 `app_deepseek_register_win32.go` 模式：

1. `wininput.FindChromeWindow(pid)` + `MeasureGeometry`
2. CDP `DOM.getBoxModel` / `Runtime.evaluate` → viewport 坐标
3. `MouseSender` 贝塞尔（OS 层）+ `KeyboardSender` 逐键
4. Workbench `ActionRequest` 增加 `inputMode: "auto"|"os"|"cdp"`（默认 `auto`）

**Plans / XBS**：`account-login-v1.yaml` 的 type/click 默认 `inputMode: auto` → 登录流 S0。

### L5 — 行为 / 熵

| 机制 | 模块 | 99+ |
|------|------|-----|
| 轨迹 | `humanize/trajectory_*` | Fitts + 1/f 噪声 |
| 打字 | `typing_*` + OS 逐键 | 密码慢 40%；IME 模拟 |
| 滚动 | `scroll_*` | 回读 15–25%；内容感知暂停 |
| 时间窗 | `AsymmetricShouldExecute` | 7:00–23:00 |
| Shannon 熵 | `AsymmetricStealthReport` | H≈2.2–3.6 human_like |
| 调度 | `SchedulerRunTaskNow` | 非窗口 / pause 期 block |

### L6 — 反馈 / 降级

```text
CAPTCHA / 403 / 异常 pageUrl
        → AsymmetricRecordChallenge
        → DeriveFeedback
              ├─ pause 24–48h
              ├─ ProfileRotateFingerprintSeed
              ├─ prefer API (L0)
              └─ rotate residential exit
```

**99+ 运营**：`ChallengeRatePct ≤ 5%`；`PauseActive` → 总分 **-8**。

---

## 3. 评分 → 99+ 达标路径

现有权重（`stealth_matrix.go`）：

| 维度 | Weight | 99+ 目标分（维内） |
|------|--------|-------------------|
| network | 0.22 | ≥ 92 |
| fingerprint | 0.22 | ≥ 90 |
| behavior | 0.16 | ≥ 88 |
| trust_api | 0.25 | ≥ **95** |
| operational | 0.15 | ≥ 88 |

**Bonus（最高 +5.0）**

- API-first + trust + graph fresh：**+2.5**
- 80/80 + geo + WebRTC + DNS：**+1.5**
- CreepJS ≥ 85：**+1.0**

**典型 99+ 组合（小红书养exc表格例）**

```text
trust_api  98  × 0.25 = 24.5
network    95  × 0.22 = 20.9
fingerprint 92 × 0.22 = 20.2
behavior   90  × 0.16 = 14.4
operational 90 × 0.15 = 13.5
bonus (API+geo+CreepJS)      = +5.0
─────────────────────────────────
total                         ≈ 98.5 → 补 operational/behavior → 99+
```

**无 L0 时天花板 ~88**：必须接受或补 manual=no 的替代（Cookie harvest + 低敏养号）。

---

## 4. Operation Matrix（按平台扩展）

### 4.1 通用模板

| 操作类 | L0 API | L4 浏览器 | 输入级别 |
|--------|--------|-----------|----------|
| 读列表/通知 | ✅ 优先 | 降级 | — |
| 登录 / OTP | Cookie 继承 | 必须 | S0 OS |
| 养号浏览 | — | 是 | S1 |
| 发布 / 支付 | 若开放 API | 是 | S0 |
| 探针 / 自检 | — | 是 | S1 |

### 4.2 XHS（`platform-packs/xhs/`）

| 场景 | 最优路径 | 输入 |
|------|----------|------|
| explore 可达 | UDEAL + SSH 桥 + socks5:// | navigate only |
| 已登录会话 | Cookie harvest → trust bundle | 免登录 |
| 手机号登录 | plan `account-login-v1` | S0 OS click/type |
| 保守养号 | `account-nurture-conservative-v1` | S1 scroll + 偶发 S0 |
| 创作者只读 | API 若无 → readonly plan | S1 |

---

## 5. 实施路线图（99+ 专用）

与 `docs/99-implementation-roadmap.md`（功能广度）正交；本表只列 **stealth 99+ 关键路径**。

### Sprint G0 — 输入平面（P0，阻塞 XBS live 99+）

| ID | 交付 | 验收 | 状态 |
|----|------|------|------|
| G0-1 | `InputPlaneRouter` in `backend/internal/behavior/inputplane/` | unit: auto 路由 S0/S1 | **已落地** |
| G0-2 | Workbench click/type 接 wininput | headed 实例 login plan 无 script | **已落地**（Windows；auto 失败回退 CDP） |
| G0-3 | `ActionRequest.inputMode` + audit 字段 `inputPlane` | API 契约 + gate | **已落地** |
| G0-4 | XHS live 复验 | `xhs_live_acceptance.ps1` 12/12；explore HTTPS + CreepJS ≥85 | **已通过**（2026-06-30；trust=100；默认复用 LaunchServer + finally stop 实例） |

### Sprint G1 — 协议 + 探针（P1）

| ID | 交付 | 验收 | 状态 |
|----|------|------|------|
| G1-1 | CDP-minimal 连接配置 | `EnableMinimalSession` Page-only；workbench pool 启用 | **已落地** |
| G1-2 | CreepJS ≥ 85 纳入 XBS gate | `TargetCreepJSTrust99Plus` + `/api/workbench/stealth-probe` | **已落地** |
| G1-3 | 禁 Camoufox 吃 Chromium flags | `FilterLaunchArgsForCamoufox` + `MaterializeRuntimeArgsForCore` | **已落地** |

### Sprint G2 — L0 平台信任（P1）

| ID | 交付 | 验收 |
|----|------|------|
| G2-1 | `platform-packs/*/trust.yaml` | Cookie 域 + harvest 步骤 |
| G2-2 | XHS cookie → ProfileTrustBundle | 二次启动免登录 |
| G2-3 | `AsymmetricAutoReach99Plus` 含 XHS 步骤 | autopilot gaps 空 |

### Sprint G3 — 传输 + 设备族（P2）

| ID | 交付 | 验收 |
|----|------|------|
| G3-1 | runtime family → transport 模板 | singbox/xray smoke |
| G3-2 | 设备族 generator + consistency | 2 族预置 |
| G3-3 | Camoufox Juggler 副轨 executor | A/B 报告，不阻塞主轨 |

---

## 6. 验收命令（99+ 签字）

```powershell
# 1. PGS 硬门槛
.\scripts\platform_99_gate.ps1 -Track PGS

# 2. 矩阵分 + 缺口
# App: AsymmetricStealthReportV2(profileId) → totalScore ≥ 99, target99Plus=true

# 3. 零人工 pipeline
# App: AsymmetricAutoReach99Plus(profileId, optsJSON)

# 4. XBS live（必须）
.\scripts\xhs_live_acceptance.ps1 -ViaSSH panda -StartTimeoutSec 300

# 5. 不对称 gate
.\scripts\asymmetric_stealth_gate.ps1
.\scripts\asymmetric_autopilot_gate.ps1
```

**签字条件（全部满足才可称 99+）**

1. `totalScore ≥ 99.0` 且 `displayGrade = S+`
2. `gaps` 不含 residential / creepjs / trust 硬缺口
3. XBS live：`pageUrl` 目标域 + 非 chrome-error
4. 登录/发布场景：**零 script**；高敏 **OS 输入** 或 API
5. 7 日内 `ChallengeRatePct ≤ 5%`（运营期）

---

## 7. 架构决策记录（ADR）

| 决策 | 选择 | 理由 |
|------|------|------|
| 主引擎 | fingerprint-chromium | XHS 偏 Chrome；已有 proxy/workbench 栈 |
| 输入默认 | auto → S0 登录/S1 浏览 | OS 不能 scale 全量；分级最优 |
| Camoufox | 副轨 A/B，非默认 | 未接 Juggler；Firefox 画像风险 |
| 100% OS 输入 | **拒绝** | 不解决指纹/协议；headed-only |
| 100% CDP 输入 | **拒绝** | Arkose/Lexical/CoalescedEvents |
| 99 vs 100 | 99+ = 成本不对称 S+ | 诚实产品边界 |

---

## 8. 一页总结

**全局 99+ 最优解 = L0 信任继承 + L1 住宅/geo 网络 + L2 单一引擎指纹 + L3 CDP-minimal + L4 分级输入（高敏 OS） + L5 行为熵 + L6 反馈降级**，用 `EvaluateStealthMatrix` 量化，用 **XBS live** 落地验证。

**下一步 P0**：G2 L0 平台信任（XHS Cookie → TrustBundle）；登录 bootstrap 去 script + headed S0 登录 live。
