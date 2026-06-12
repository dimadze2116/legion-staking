@echo off
setlocal EnableDelayedExpansion
set PATH=C:\Users\Маруся\.cargo\bin;%PATH%
cd /d C:\bot3\contract
echo Checking Rust tools...
where rustc && rustc --version
echo.
echo Checking cargo...
where cargo && cargo --version
echo.
echo Checking wasm target...
rustup target list --installed | findstr wasm
echo.
echo Searching for rust-lld...
dir /s /b "%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\*lld*" 2>nul
echo.
echo Searching for link.exe...
dir /s /b "%USERPROFILE%\..\..\Program Files\Microsoft Visual Studio\*\*\*\bin\Hostx64\x64\link.exe" 2>nul
echo.
echo Attempting cargo build...
cargo build --target wasm32-unknown-unknown --release 2>&1
echo.
echo Build exit code: %ERRORLEVEL%