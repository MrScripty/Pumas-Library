import { build } from 'esbuild';

// tsc compiles src/desktop-contract.ts to a require of the shared workspace
// package. Replace that output with the package bundled into the shell's
// CommonJS output cohort, decoders and validators together, so packaged
// builds do not depend on workspace node_modules.
await build({
  entryPoints: ['src/desktop-contract.ts'],
  outfile: 'dist/desktop-contract.js',
  bundle: true,
  format: 'cjs',
  platform: 'node',
  target: 'node22',
  sourcemap: true,
});

// Sandboxed Electron preload cannot require sibling Node modules. Keep the
// accepted sandbox and bundle only this process entry point's dependencies.
await build({
  entryPoints: ['src/preload.ts'],
  outfile: 'dist/preload.js',
  bundle: true,
  platform: 'browser',
  format: 'cjs',
  target: 'chrome142',
  external: ['electron'],
  sourcemap: true,
});
