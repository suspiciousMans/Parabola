#!/usr/bin/env bash
# Build Ember Reach for the browser into dist-web/. Needs emsdk on PATH
# (`source emsdk_env.sh`) and `rustup target add wasm32-unknown-emscripten`.
# If emcc can't download the SDL2 port, clone libsdl-org/SDL at release-2.30.9
# and set EMCC_LOCAL_PORTS="sdl2=<clone>".
set -euo pipefail
cd "$(dirname "$0")/.."
command -v emcc >/dev/null || { echo "emcc not found: source emsdk_env.sh first" >&2; exit 1; }
cargo build --release -p ember_reach --target wasm32-unknown-emscripten
out=target/wasm32-unknown-emscripten/release
mkdir -p dist-web
cp web/index.html dist-web/
cp "$out"/ember_reach.js "$out"/ember_reach.wasm dist-web/
echo "built dist-web/ (serve: python3 -m http.server -d dist-web)"
echo "smoke test: node web/web_smoke.js dist-web ember_reach shot.png"
