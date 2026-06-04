@echo off
chcp 65001 >nul
rem 一键打包 loghound：双击运行即可。实际逻辑在 scripts\package.ps1。
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\package.ps1" %*
pause
