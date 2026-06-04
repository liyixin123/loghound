' run-hidden.vbs
' Launch loghound.exe in the CURRENT interactive session with no visible window.
' Used by the logon scheduled task so capture runs silently in session 1.
Option Explicit
Dim fso, sh, scriptDir, exePath
Set fso = CreateObject("Scripting.FileSystemObject")
scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)
exePath = scriptDir & "\loghound.exe"
Set sh = CreateObject("WScript.Shell")
sh.CurrentDirectory = scriptDir
' 0 = hidden window, False = do not wait
sh.Run """" & exePath & """ run", 0, False
