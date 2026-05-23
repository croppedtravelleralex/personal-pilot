# Scripts

Updated: 2026-04-15 (Asia/Shanghai)

## 当前默认入口

- [windows_local_verify.ps1](D:/SelfMadeTool/persona-pilot/scripts/windows_local_verify.ps1)
- [validation_lightpanda_smoke.ps1](D:/SelfMadeTool/persona-pilot/scripts/validation_lightpanda_smoke.ps1)：P5 真实 Lightpanda/CDP validation smoke 入口

## 当前原则

- 仅保留本地 Windows 交付路径需要的脚本入口
- `PowerShell` 是默认验证与维护入口
- 新增验证脚本优先使用 `.ps1`
- 其他脚本如果继续保留，仅作为开发期辅助工具

## 使用方式

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1
```

P5 Lightpanda/CDP validation smoke：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/validation_lightpanda_smoke.ps1 -Repetitions 2 -TimeoutSeconds 20
```

该脚本要求 `lightpanda` 在 PATH 中，或通过 `LIGHTPANDA_BIN` 指向真实可执行文件。没有真实二进制时，脚本会生成 `blocked` evidence report，不应被当作真实 profile-browser runtime proof。
