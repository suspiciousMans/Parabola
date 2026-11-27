#!/usr/bin/env bash
# Build Ember Reach for the browser into dist-web/. Locates emsdk itself
# (EMSDK, ~/emsdk or /opt/emsdk), so no PATH setup or manual sourcing is
# needed. Requires `rustup target add wasm32-unknown-emscripten`.
# If emcc can't download the SDL2 port, clone libsdl-org/SDL at release-2.30.9
# and set EMCC_LOCAL_PORTS="sdl2=<clone>".
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v emcc >/dev/null 2>&1; then
    for env_script in \
        "${EMSDK:-}/emsdk_env.sh" \
        "$HOME/emsdk/emsdk_env.sh" \
        "${EMSDK_ROOT:-}/emsdk_env.sh" \
        "/opt/emsdk/emsdk_env.sh"; do
        if [ -n "$env_script" ] && [ -f "$env_script" ]; then
            # emsdk_env.sh is not `set -u` clean in every version.
            set +u
            # shellcheck disable=SC1090
            source "$env_script" >/dev/null 2>&1 || true
            set -u
            command -v emcc >/dev/null 2>&1 && break
        fi
    done
fi

if ! command -v emcc >/dev/null 2>&1; then
    echo "emcc not found: install emsdk and set EMSDK to its directory, or place it at \$HOME/emsdk or /opt/emsdk." >&2
    exit 1
fi

cargo build --release -p ember_reach --target wasm32-unknown-emscripten
out=target/wasm32-unknown-emscripten/release
mkdir -p dist-web
cp web/index.html dist-web/
cp "$out"/ember_reach.js "$out"/ember_reach.wasm dist-web/
echo "built dist-web/ (serve: python3 -m http.server -d dist-web)"
echo "smoke test: node web/web_smoke.js dist-web ember_reach shot.png"
