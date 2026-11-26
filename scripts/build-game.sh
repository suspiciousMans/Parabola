#!/usr/bin/env bash
# Rebuild the 3D board (Rust, WebAssembly) and copy it where the site serves it:
# src/client/public/game/. Needs emsdk on PATH (see web/build.sh). The built files
# are committed so `npm run build` and deploys don't need Rust or emsdk.
set -euo pipefail
cd "$(dirname "$0")/.."
web/build.sh
mkdir -p src/client/public/game
cp dist-web/ember_reach.js dist-web/ember_reach.wasm src/client/public/game/
ls -la src/client/public/game
