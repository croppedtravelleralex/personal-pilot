# STATUS.md

**兼容入口** — canonical 状态见 [`docs/02-current-state.md`](docs/02-current-state.md)。

Updated: 2026-07-11

## 当前真实标记

- Mainline / Local self-use：`100% / 0% / green`
- 唯一未验：CAPTCHA / SMS / Email 真实账号凭证 smoke
- Fingerprint：`80` declared / `26` runtime projected / observed `450/450`
- Behavior：Go `30` shipped / Rust `13` active-runner-backed；replay `461/450`
- **Stealth 轨道**：W0（A1/A3/C1 + D1 主体）已落地；D1 OS fallback 与横评刷分未关。下一批 W1 见 [`PLAN.md`](PLAN.md)
- **Benchmark**：PersonaPilot `73/100`、BitBrowser `71/100`、AdsPower `N/A`（改前 raw；W0 后未刷分）。详见 [`docs/48`](docs/48-three-browser-benchmark-matrix-plan.md)

## 先读

1. `PLAN.md`（指纹/反检测执行枢纽）
2. `docs/README.md`
3. `docs/02-current-state.md`
4. `docs/05-ai-maintenance-playbook.md`

历史段落（2026-04 / 2026-05 Mainline delta）见 git 历史；勿当作 live truth。
