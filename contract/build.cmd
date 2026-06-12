@echo off
set PATH=C:\Users\Маруся\.cargo\bin;%PATH%
cd /d C:\bot3\contract
cargo build --target wasm32-unknown-unknown --release
pause