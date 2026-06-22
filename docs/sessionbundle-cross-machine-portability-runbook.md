# SessionBundle Cross-Machine Portability Runbook（CANCELLED / historical-only）

Updated: 2026-06-22 (Asia/Shanghai)

## 结论

2026-06-22 范围重置后，第二机、干净 Win11 target 和跨机器 `SessionBundle` portability 已取消。当前项目只按本机自用维护：保留 profile-scoped export、import preflight、dry-run、confirmed local restore、failure reason 和 report history。

不要按本文件执行第二环境 smoke；不要要求 `session_bundle_portability_smoke.ps1 -CrossMachine`；不要把缺少 `cross_machine_passed` report 写成当前阻塞。

## 仍然有效的本机目标

- 本机测试 profile 可导出 bundle。
- import preflight 能解释 schema、引用缺失和 profile 冲突。
- dry-run restore 保持 `writePerformed=false`。
- confirmed local restore 可 upsert target profile 和 `proxy_session_bindings`。
- 本机 restart continuity 以恢复后的 cookie / localStorage / sessionStorage 持久化证据为准。
- restore plan、blockers、restored binding count 和 failure reason 可在 Settings / report history 中查看。
- 敏感 payload 默认脱敏，只有显式本地开关和授权记录齐全时才纳入 bundle。

## 历史步骤

以下步骤只作为历史设计参考，当前不执行：

1. 在 source machine 导出测试 profile bundle。
2. 记录 bundle id、profile id、collector/export version、导出时间。
3. 将 bundle 复制到 target machine。
4. 在 target machine 运行 import preflight，使用新的 target profile id。
5. 运行 dry-run restore，确认 `writePerformed=false`。
6. 运行 confirmed restore，确认 target profile 和 `proxy_session_bindings` 已写入。
7. 重启 PersonaPilot。
8. 验证 target profile 仍可读取 session continuity evidence。
9. 导出 target profile evidence package。
10. 保存 source/target 两侧 report path。

## 如果未来重新开启跨机器目标

需要用户先确认新目标、第二环境、授权边界和敏感数据处理方式，再重建 runbook 和验收脚本。旧步骤不能直接作为当前 acceptance。
