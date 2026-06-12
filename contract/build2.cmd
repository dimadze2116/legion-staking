@echo off
setlocal

:: Find the 8.3 short name for Маруся
for /f "skip=5 tokens=4" %%a in ('dir /x "C:\Users" 2^>nul') do (
  echo %%a | findstr /c:"~" >nul && (
    set "SHORT=%%a"
  )
)

:: Direct approach - use the short name if available
set "CARGO_HOME=C:\Users\Маруся\.cargo"
set "RUSTUP_HOME=C:\Users\Маруся\.rustup"

:: Just test if gcc works
set PATH=C:\tools\mingw64\mingw64\bin;C:\Users\Маруся\.cargo\bin;%PATH%

cd /d "%~dp0"
echo Path:
where gcc
where rustc
rustc --version
echo.
echo Starting build...
cargo build --target wasm32-unknown-unknown --release 2>&1
if errorlevel 1 echo Build FAILED
if exist target\wasm32-unknown-unknown\release\legion_staking.wasm (
  echo ======== SUCCESS ========
  dir target\wasm32-unknown-unknown\release\legion_staking.wasm
)
pause