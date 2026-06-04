@echo off
chcp 65001 >nul
setlocal

set TASKNAME=loghound
set APPDIR=%~dp0app

rem --- 自动提权 ---
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo 需要管理员权限，正在请求提权...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

if not exist "%APPDIR%\loghound.exe" (
    echo [错误] 未找到 %APPDIR%\loghound.exe
    pause
    exit /b 1
)

echo 正在注册并启动 loghound（登录自启，后台隐藏运行）...
schtasks /Create /TN "%TASKNAME%" /TR "wscript.exe \"%APPDIR%\run-hidden.vbs\"" /SC ONLOGON /RL LIMITED /F
if %errorlevel% neq 0 (
    echo [错误] 注册失败。
    pause
    exit /b 1
)
schtasks /Run /TN "%TASKNAME%" >nul 2>&1

echo.
echo 完成。
pause
