$env:RUSTFLAGS="--cfg=web_sys_unstable_apis --cfg=getrandom_backend=`"wasm_js`""
trunk build --features server_build