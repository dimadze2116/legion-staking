@echo off
SET PATH=C:\Users\2761~1\.cargo\bin;%PATH%
cd /d C:\bot3\contract
echo Copying toolchain to short path (this may take a moment)...
if not exist C:\rustsysroot\lib\rustlib\x86_64-pc-windows-gnu\lib\libcore-c3a1ea6652048df9.rlib (
    mkdir C:\rustsysroot 2>nul
    robocopy "C:\Users\Маруся\.rustup\toolchains\stable-x86_64-pc-windows-gnu" "C:\rustsysroot" /E /R:0 /W:0 /NFL /NDL /NJH /NJS >nul
)
echo Copy complete.
dir /b C:\rustsysroot\lib\rustlib\x86_64-pc-windows-gnu\lib\*.rlib | head -3
echo Done.