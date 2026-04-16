# Scripts

Updated: 2026-04-15 (Asia/Shanghai)

## 当前默认入口

- [windows_local_verify.ps1](D:/SelfMadeTool/persona-pilot/scripts/windows_local_verify.ps1)

## 当前原则

- 仅保留本地 Windows 交付路径需要的脚本入口
- `PowerShell` 是默认验证与维护入口
- 新增验证脚本优先使用 `.ps1`
- 其他脚本如果继续保留，仅作为开发期辅助工具

## 使用方式

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1
```
