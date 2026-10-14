#!/bin/bash
# Rebuild checked-in browser crypto artifacts from Cargo.lock and shared Rust source.
set -euo pipefail
cd "$(dirname "$0")/../.."
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
if [[ "$(wasm-bindgen --version)" != 'wasm-bindgen 0.2.122' ]]; then
  echo 'Install wasm-bindgen-cli 0.2.122 and rustup target wasm32-unknown-unknown' >&2
  exit 1
fi
cargo build --locked -p tealdrive-channel --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir src/ui/sdk/noise --out-name noise target/wasm32-unknown-unknown/release/tealdrive_channel.wasm
