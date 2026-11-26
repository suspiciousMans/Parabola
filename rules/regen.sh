#!/usr/bin/env bash
# Refresh the committed transpiled copy after editing ox/reach.ox.
# Needs the `oxidized` binary (OXIDIZED=/path/to/oxidized or on PATH).
set -euo pipefail
cd "$(dirname "$0")"
OXIDIZED="${OXIDIZED:-oxidized}"
"$OXIDIZED" test ox/tests
mkdir -p ox_gen
cargo build -p reach_rules
cp "$(ls -t ../target/debug/build/reach_rules-*/out/reach_gen.rs | head -1)" ox_gen/reach_gen.rs
echo "ox_gen/reach_gen.rs updated"
