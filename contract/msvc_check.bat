@echo off
SET PATH=C:\Users\2761~1\.cargo\bin;%PATH%
cd /d C:\bot3\contract
echo Using MSVC toolchain...
rustup default stable-x86_64-pc-windows-msvc
rustc --version
rustup target list --installed | findstr wasm
echo Done.