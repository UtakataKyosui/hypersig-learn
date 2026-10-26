#!/bin/sh
set -eu

# Prefer rustup's proxies over a system Rust without the Wasm target.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
if [ "$(worker-build --version 2>/dev/null || true)" != "0.8.7" ]; then
  cargo install --quiet --locked worker-build@0.8.7
fi
worker-build --release
