@echo off
chcp 65001 >nul
set CARGO_TARGET_DIR=C:\bot3-build
set PATH=C:\tools\mingw64\mingw64\bin;C:\Users\Маруся\.cargo\bin;%PATH%
cd /d "%~dp0"
cargo build --target wasm32-unknown-unknown --release
pause