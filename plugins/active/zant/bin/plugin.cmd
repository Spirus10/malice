@echo off
setlocal
cargo run --quiet --manifest-path "%~dp0..\worker\Cargo.toml" --
