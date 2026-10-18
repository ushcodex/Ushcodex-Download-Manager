@echo off
setlocal
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
title Ushcodex Download Manager (UDM) - Rust Edition
color 0B

echo =========================================================================
echo    ⚡ Ushcodex Download Manager (UDM) Pro - Rust Edition Launcher
echo =========================================================================
echo.
echo Select Mode:
echo [1] Interactive Download (Paste URL, choose streams, start accelerated download)
echo [2] Start Auto-Detection Daemon (Listen to Chrome/Edge/Firefox & Clipboard)
echo [3] View Download History & SHA-256 Hashes
echo [4] Check Cloudflare WARP Turbo Tunnel Status
echo [5] Exit
echo.

set /p choice="Enter choice (1-5) [Default 1]: "
if "%choice%"=="" set choice=1

if "%choice%"=="1" goto interactive
if "%choice%"=="2" goto daemon
if "%choice%"=="3" goto history
if "%choice%"=="4" goto warp
if "%choice%"=="5" goto end

:interactive
echo.
cargo run --release
pause
goto end

:daemon
echo.
echo Starting UDM Daemon on port 19800 with Clipboard Auto-Detection...
cargo run --release -- daemon --port 19800
pause
goto end

:history
echo.
cargo run --release -- history
pause
goto end

:warp
echo.
cargo run --release -- warp status
pause
goto end

:end
exit /b
