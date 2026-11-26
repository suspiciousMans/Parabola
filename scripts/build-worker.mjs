import { build } from 'esbuild';
import { copyFileSync } from 'node:fs';

await build({
  entryPoints: ['src/worker/index.ts'],
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  target: 'es2022',
  outfile: 'dist/worker.js',
  // The Worker imports the rules as a WebAssembly module; keep that import as
  // written so Sites can attach the .wasm module next to the Worker.
  external: ['./reach_rules.wasm', '*.wasm'],
  logLevel: 'info',
});

copyFileSync('src/game/reach_rules.wasm', 'dist/reach_rules.wasm');
