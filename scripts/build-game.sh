#!/usr/bin/env bash
# Rebuild the 3D board (Rust, WebAssembly) and copy it where the site serves it:
# src/client/public/game/. The script locates emsdk itself (EMSDK or the usual
# ~/emsdk install) so `npm run build:game` works from a shell without a manually
# prepared PATH. The built files are committed so `npm run build` and deploys
# don't need Rust or emsdk.
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
    echo "build-game: emcc not found. Install emsdk and set EMSDK to its directory, or place it at \$HOME/emsdk (expected \$EMSDK/emsdk_env.sh)." >&2
    exit 1
fi

web/build.sh
mkdir -p src/client/public/game
cp dist-web/ember_reach.js dist-web/ember_reach.wasm src/client/public/game/
ls -la src/client/public/game
