import subprocess, os, sys

os.chdir("C:\\bot3\\contract")

# Use the stable GNU toolchain which has wasm32 support and is designed for this
cargo = "C:\\Users\\Маруся\\.rustup\\toolchains\\stable-x86_64-pc-windows-gnu\\bin\\cargo.exe"
gnu_bin = "C:\\Users\\Маруся\\.rustup\\toolchains\\stable-x86_64-pc-windows-gnu\\bin"
self_contained = "C:\\Users\\Маруся\\.rustup\\toolchains\\stable-x86_64-pc-windows-gnu\\lib\\rustlib\\x86_64-pc-windows-gnu\\bin\\self-contained"
cargo_bin = "C:\\Users\\Маруся\\.cargo\\bin"

env = os.environ.copy()
env["CARGO_TARGET_DIR"] = "C:\\bot3-build"
env["PATH"] = self_contained + ";" + gnu_bin + ";" + cargo_bin + ";" + env.get("PATH", "")

r = subprocess.run(
    [cargo, "build", "--target", "wasm32-unknown-unknown", "--release"],
    capture_output=True, text=True, timeout=600, env=env
)
print(r.stdout[-3000:])
print(r.stderr[-3000:])
print(f"EXIT: {r.returncode}")
wasm = "C:\\bot3-build\\wasm32-unknown-unknown\\release\\legion_staking.wasm"
if os.path.exists(wasm):
    print(f"SUCCESS: {os.path.getsize(wasm)} bytes")
else:
    print("FAIL")