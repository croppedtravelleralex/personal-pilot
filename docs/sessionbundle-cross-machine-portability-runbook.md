# SessionBundle Cross-Machine Portability Runbook

Updated: 2026-05-23 (Asia/Shanghai)

## 目标

把当前本机 `SessionBundle` contract 推进到真实跨机器 portability evidence。当前状态是：export/import preflight/dry-run/confirmed local restore 已落地，但第二台或干净 Win11 环境 smoke 未执行。

## 前置条件

- Source machine：已有测试 profile，包含可公开测试的 cookie/localStorage/sessionStorage 证据。
- Target machine：干净 Win11 环境，安装同版本 PersonaPilot。
- 不使用生产账号或真实敏感 profile。
- bundle 默认脱敏；只有明确测试时才启用敏感 payload。

## 执行步骤

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

## 必须记录的字段

- source machine id 或备注
- target machine id 或备注
- bundle path
- source profile id
- target profile id
- preflight status
- dry-run status
- confirmed restore status
- restored session binding count
- restart continuity status
- failure reason
- report paths

## 退出条件

- target machine 上生成 portability evidence report。
- confirmed restore 后 app restart continuity 仍成立。
- 敏感 payload 的包含/排除状态可解释。
- 失败时保留 failure reason，不能只写“未通过”。
