Set Fso = CreateObject("Scripting.FileSystemObject")
Set WshShell = CreateObject("WScript.Shell")

scriptDir = Fso.GetParentFolderName(WScript.ScriptFullName)
WshShell.Run "cmd /c """ & scriptDir & "\tmp-start-app.cmd""", 0, False
