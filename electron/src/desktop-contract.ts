// The shared generated desktop contract (packages/desktop-contract).
//
// tsc compiles this re-export to a require of the workspace package, which
// resolves during development. scripts/bundle-preload.mjs then replaces
// dist/desktop-contract.js with a self-contained CommonJS bundle, so the
// packaged app needs no workspace package at runtime.
export * from '@pumas/desktop-contract';
