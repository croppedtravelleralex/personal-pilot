# Improvement Backlog

## Mainline remaining `7%`

| 领域 | 待改进项 | 当前状态 | 退出条件 |
| --- | --- | --- | --- |
| Proxy / IP | provider-side proxy rotation write | 未闭环 | 真实 provider action、rollback、cooldown、retry 均走 typed native chain |
| Synchronizer | native broadcast write closure；set-main / work-area-aware layout 已落地 | 未闭环 | staged-only 不再是主 operator route，broadcast write 走 typed native chain |
| Recorder / Templates | native-first de-fallback closure | 未闭环 | release-default path 不依赖 fallback |
| Release | Win11 final packaging / operator acceptance polish | 待收敛 | Rust gate、Win11 verify、release build 同时通过 |

## Overall remaining `70%`

| 领域 | 待改进项 | 当前状态 | 退出条件 |
| --- | --- | --- | --- |
| Validation | detector / leak / DNS / WebRTC / canvas / audio / worker / transport board | 未落地 | 有可重复 evidence report |
| Fingerprint | 从 `12` runtime projected fields 扩展 runtime depth | 未落地 | declared / applied / observed 可解释且可验证 |
| Session | 完整 `SessionBundle` 和 profile portability | 未落地 | import/export/restore 与 restart continuity 一起成立 |
| Behavior | 从 `13` primitives 扩展 replayable `450+` taxonomy | 未落地 | 事件数量来自真实 taxonomy，不是目标口号 |
| Runtime | headed realism、kernel strategy、adapter boundary | 未落地 | 有稳定 adapter contract 和验证证据 |
| Benchmark | AdsPower boundary refresh | 暂缓 | B1-B5 有新证据后再刷新评分 |

## 固定风险

- 把 `80` declared controls 误报成 `80` runtime-applied fields。
- 把 `450+` fingerprint / event target 误报成当前已交付。
- 把 mock / fallback / staged 默认路径算作 delivery closure。
- 用历史 `77% / 23%` 或 `82% / 18%` 覆盖当前 live truth。
- 在 Mainline closeout 阶段引入 Overall 级大范围重构。

## 维护规则

新增 backlog 时必须标清属于 Mainline `7%` 还是 Overall `70%`。不能确定归属时先放 Overall，等有代码证据后再提升到 Mainline closeout。
