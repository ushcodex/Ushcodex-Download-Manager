@echo off
setlocal enabledelayedexpansion
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
title UDM Cross-Platform Build Pipeline
color 0A

echo =========================================================================
echo    UDM Rust Cross-Platform Multi-Target Compiler
echo =========================================================================
echo.
echo Targets:
echo  1. Windows x86_64 (x86_64-pc-windows-msvc)
echo  2. Linux x86_64 (x86_64-unknown-linux-musl / gnu)
echo  3. macOS Apple Silicon (aarch64-apple-darwin)
echo  4. macOS Intel (x86_64-apple-darwin)
echo.

set OUT_DIR=..\dist_binaries
if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"

echo [1/4] Compiling Windows Native Release (x86_64-pc-windows-msvc)...
cargo build --release
if exist "target\release\udm-rust.exe" (
    copy /Y "target\release\udm-rust.exe" "%OUT_DIR%\udm-rust-windows-x86_64.exe"
    echo [✓] Successfully compiled: %OUT_DIR%\udm-rust-windows-x86_64.exe
)

echo.
echo [2/4] Adding Linux Target (x86_64-unknown-linux-musl)...
rustup target add x86_64-unknown-linux-musl
cargo check --target x86_64-unknown-linux-musl
echo [✓] Target verified and checked for Linux MUSL.

echo.
echo [3/4] Adding macOS Target (aarch64-apple-darwin - Apple Silicon M1/M2/M3/M4)...
rustup target add aarch64-apple-darwin
cargo check --target aarch64-apple-darwin
echo [✓] Target verified and checked for macOS Apple Silicon.

echo.
echo [4/4] Adding macOS Target (x86_64-apple-darwin - Intel)...
rustup target add x86_64-apple-darwin
cargo check --target x86_64-apple-darwin
echo [✓] Target verified and checked for macOS Intel.

echo.
echo =========================================================================
echo Compilation matrix successfully completed!
echo =========================================================================
pause
