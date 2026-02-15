@echo off
:: Kill lingering processes that may hold port 1420
powershell -Command "Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue }"
taskkill /f /im signalflow-gui.exe >nul 2>&1

:: Launch Tauri dev
cd /d "G:\Misc\Dev\signalFlow\src-tauri"
"G:\Misc\Dev\signalFlow\gui\node_modules\.bin\tauri.cmd" dev
