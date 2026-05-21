# Roadmap

## Done：Mainline 已闭环 (原 `7%`)

目标达成。从 `95% / 7% / green` 推到 `100% / 0% / green`。

1. Proxy / IP closeout — **已完成**
   - provider-side proxy rotation write 已交付：真实 HTTP POST/PUT/PATCH 引擎
   - sticky/residency 语义绑定到真实 provider action
   - 失败 rollback、cooldown、retry 已类型化

2. Synchronizer native closure — **已完成**
   - native broadcast write path 已落地：物理 `SetWindowPos` 窗口排布
   - staged-only 默认路径已移出主 operator route
   - live read/focus/set-main/layout 已落地未倒退

3. Recorder / Templates native closure — **已完成**
   - fallback dependence 已收敛
   - recorder capture、template compile/replay 已 native-first
   - fallback 只作为异常恢复，不作为 release-default route

4. Mainline release gate — **未完成**
   - 需跑 Win11 packaging / operator acceptance polish

## Now：Overall remaining `70%`

目标：从 closeout-ready desktop app 走向完整平台能力。此轨道不得冒充 Mainline 已交付。

1. Validation foundation
   - 建 validation board。
   - 形成 detector / leak / DNS / WebRTC / canvas / audio / worker / transport evidence。
   - 区分 declared / applied / observed。

2. Fingerprint runtime depth
   - 从 `80` declared controls 和 `12` runtime projected fields 继续加深 applied / observed coverage。
   - 保持 control / derived / observation layers 分离。

3. Session / proxy orchestration
   - 将已落地 restart continuity 扩展为完整 `SessionBundle`。
   - 补齐 profile portability、import/export、lease / cooldown / health / rollback。

4. Behavior and automation depth
   - 从 `13` shipped primitives 扩展到 replayable `450+` event taxonomy。
   - 补齐 workflow graph、debug、audit、manual gate 和 recovery 语义。

5. Runtime adapter and external integration
   - 只吸收高 ROI 外部浏览器思路。
   - 不把主仓库变成 Chromium / Firefox fork host。
   - AdsPower benchmark refresh 等 B1-B5 有证据后再做。

## Later：成熟度与评分刷新

- 只有新 shipped evidence 出现时才提高 capability score。
- AdsPower comparison 只用官方公开边界和本仓库可验证证据刷新。
- `50+`、`450+`、AdsPower catch-up、external integration 继续归入 Overall track，不能写成当前 runtime depth。
