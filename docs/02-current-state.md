# Current State

Updated: 2026-05-21 (Asia/Shanghai)

## 当前 live truth

- Mainline delivery：`100% / 0% / green` (3 P0 items closed)
- Overall end-state：`30% / 70% / yellow`
- 第一族控制 schema 已声明 `80` 个 core control fields。
- 当前 runtime projection 仍是 `12` 个 env-backed fingerprint fields，包括 derived `platform`。
- 当前 behavior runtime 已交付 `13` 个 primitives。
- cookie / localStorage / sessionStorage 跨 app restart 持久化与恢复已落地。

历史 `77% / 23%` 或 `82% / 18%` 不再作为当前进度口径。

## 已确认落地

- Win11 desktop shell 基于 Tauri 2 + Vite + React + TypeScript 已落地。
- `src/services/desktop.ts` 是 native / invoke 的统一边界。
- Dashboard / Profiles / Proxies / Automation / Synchronizer / Logs / Settings 已在真实 operator surface 上。
- `Tasks -> Automation` surface unification 已完成。
- provider-aware / sticky-aware `changeProxyIp` local desktop contract 已落地。
- Recorder desktop step-write 已落地。
- Synchronizer live desktop snapshot、native focus、native set-main、work-area-aware native physical layout 已落地。
- full Rust / integration gate 已恢复 green。
- route-level code splitting 已清掉旧 Vite chunk warning。

## 未完成边界

### Mainline remaining

- provider-side proxy rotation write 已闭环 (multi-agent worktree closeout)。
- Synchronizer native broadcast write path 已闭环 (physical SetWindowPos + deterministic ordering)。
- Recorder / Templates native-first de-fallback closure 已闭环 (desktop session guard + empty-state fix)。
- 最终 Win11 packaging / operator acceptance polish 仍需在不重开 scope 的前提下完成（本轮已收敛 Mainline P0）。

### Overall remaining `70%`

- validation board 未落地。
- runtime materialization depth 仍窄，当前只应报告 `12` projected fields。
- `450+` fingerprint signal observation / audit coverage 未落地。
- `450+` event taxonomy 未落地。
- 完整 `SessionBundle`、profile portability、import/export contract 未落地。
- headed runtime realism、kernel strategy、AdsPower-grade catch-up 仍是整体目标轨道。
- external browser integration 已有计划，但不是已交付 runtime depth。

## 当前下一步

Mainline P0 已全部闭环。下一步方向：
1. 跑主线 release gate (Win11 packaging / operator acceptance polish)
2. 规划 Overall `70%` 阶段 1：validation board + fingerprint depth
3. 将 Multi-Agent worktree 工作流标准化为常规执行模式
