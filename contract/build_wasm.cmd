@echo off
chcp 65001 >nul
setlocal

set CARGO_TARGET_DIR=C:\bot3-build
set PATH=C:\tools\mingw64\mingw64\bin;C:\Users\Маруся\.cargo\bin;%PATH%

cd /d "%~dp0"
echo === Legion Staking WASM Builder ===
echo Target dir: %CARGO_TARGET_DIR%
rustc --version
echo Building...
cargo build --target wasm32-unknown-unknown --release
echo Exit code: %errorlevel%
if exist %CARGO_TARGET_DIR%\wasm32-unknown-unknown\release\legion_staking.wasm (
  echo ======== SUCCESS ========
  dir %CARGO_TARGET_DIR%\wasm32-unknown-unknown\release\legion_staking.wasm
  copy /Y %CARGO_TARGET_DIR%\wasm32-unknown-unknown\release\legion_staking.wasm "%~dp0legion_staking.wasm"
) else (
  echo ======== FAIL ========
)
pause