@echo off
SET PATH=C:\Users\2761~1\.cargo\bin;C:\tools\mingw64\mingw64\bin;%PATH%
cd /d C:\bot3\contract
echo === Building NEAR WASM contract ===
cargo build --target wasm32-unknown-unknown --release 2>&1
set EXITCODE=%ERRORLEVEL%
echo === EXIT_CODE: %EXITCODE% ===
exit /b %EXITCODE%