@echo off
chcp 65001 >nul
setlocal

set TASKNAME=loghound

rem --- 自动提权 ---
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo 需要管理员权限，正在请求提权...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

echo 正在停止并卸载 loghound...
schtasks /End /TN "%TASKNAME%" >nul 2>&1
taskkill /IM loghound.exe /F >nul 2>&1
schtasks /Delete /TN "%TASKNAME%" /F

echo.
echo 完成。
pause
