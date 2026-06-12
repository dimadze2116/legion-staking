@echo off
SET PATH=C:\Users\2761~1\.cargo\bin;%PATH%
cd /d C:\bot3\contract
rustc --print sysroot
echo.
echo Trying to create junction...
mklink /J C:\rustsysroot "C:\Users\Маруся\.rustup\toolchains\stable-x86_64-pc-windows-gnu"
echo Done.