#!/usr/bin/env bash
# Rebuild the rules WASM from rules/ox_gen/reach_gen.rs.
# Needs rustup with the wasm32-unknown-unknown target.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
cargo build --release --manifest-path rules/wasm/Cargo.toml --target wasm32-unknown-unknown
cp rules/wasm/target/wasm32-unknown-unknown/release/reach_rules_wasm.wasm src/game/reach_rules.wasm
echo "rules wasm: $(wc -c < src/game/reach_rules.wasm) bytes"
