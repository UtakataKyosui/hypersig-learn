#!/bin/sh
set -eu

# Prefer rustup's proxies over a system Rust without the Wasm target.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
cargo install --quiet --locked worker-build@0.8.7
worker-build --release
