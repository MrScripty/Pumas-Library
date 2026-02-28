# Testing Guide

Testing and build validation for the current Rust + Electron/React codebase.

## Quick Release Validation

Run from repository root:

```bash
# Rust (default workspace members)
cd rust
cargo test
cargo build --workspace
cd ..

# Frontend
npm run -w frontend test:run
npm run -w frontend check:types
npm run -w frontend build

# Electron
npm run -w electron validate
npm run -w electron build
```

## Rust Validation

### Default test suite

```bash
cd rust
cargo test
```

This runs the default workspace members (`pumas-core`, `pumas-app-manager`, `pumas-rpc`, `pumas-uniffi`).

### Full workspace except Rustler

```bash
cd rust
cargo test --workspace --exclude pumas_rustler
```

Use this for release readiness when you want all workspace crates except the Erlang NIF crate.

### Rustler crate (optional)

```bash
cd rust
cargo test -p pumas_rustler
```

`pumas_rustler` tests require Erlang/OTP runtime symbols and are only expected to run on machines with BEAM tooling installed.

## Frontend Validation

```bash
npm run -w frontend test:run
npm run -w frontend check:types
npm run -w frontend build
```

- `test:run` executes Vitest in non-watch mode.
- `check:types` runs TypeScript type checking.
- `build` validates production bundling.

## Electron Validation

```bash
npm run -w electron validate
npm run -w electron build
```

- `validate` runs TypeScript checks.
- `build` compiles Electron main/preload sources.

## Notes

- IPC tests bind localhost sockets. In restricted sandbox environments this can fail with `Operation not permitted` even when the code is correct.
- For release work, run final validation on a normal host environment (or CI) where local socket binding is allowed.
