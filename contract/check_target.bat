@echo off
SET PATH=C:\Users\2761~1\.cargo\bin;C:\tools\mingw64\mingw64\bin;%PATH%
cd /d C:\bot3\contract
echo Checking target...
rustc --version
rustup target list --installed 2>&1 | findstr wasm
echo DONE