# Roadmap

## Now：Mainline remaining `7%`

目标：把当前可交付 Win11 desktop mainline 从 `95% / 7% / green` 推到真正闭环，不吸收 Overall 范围。

1. Proxy / IP closeout
   - 完成 provider-side proxy rotation write。
   - 将 sticky / residency 语义绑定到真实 provider action。
   - 补齐失败 rollback、cooldown、retry 的 typed path。

2. Synchronizer native closure
   - native set-main 与 work-area-aware physical layout 已落地，继续收敛 native broadcast write path。
   - 将 staged-only 默认路径移出主 operator route。
   - 保持 live read / focus / set-main / layout 已落地事实不倒退。

3. Recorder / Templates native closure
   - 收敛 remaining fallback dependence。
   - 深化 recorder capture、template compile / replay native-first path。
   - fallback 只作为异常恢复，不作为 release-default route。

4. Mainline release gate
   - 跑 Rust gate。
   - 跑 Win11 local verify。
   - 跑 release build。
   - 确认 Tauri / single-window / service-boundary 规则没有漂移。

## Next：Overall remaining `70%`

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
