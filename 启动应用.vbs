' 双击此文件直接启动应用，完全无黑窗
' 首次使用请先运行 启动应用.bat 完成构建
Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

exePath = fso.GetParentFolderName(WScript.ScriptFullName) & "\build\bin\personal-pilot.exe"

If fso.FileExists(exePath) Then
    WshShell.Run """" & exePath & """", 0, False
Else
    MsgBox "未找到 personal-pilot.exe" & vbCrLf & vbCrLf & _
           "请先运行 ""启动应用.bat"" 完成构建。", vbInformation, "提示"
End If
