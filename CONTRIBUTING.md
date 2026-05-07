# Contributing to Pumas Library

This guide explains how to make changes to Pumas Library without fighting the
repo. A good contribution here is not just "code that works". It is a change
that is clearly scoped, validated at the right layer, documented where the
contract changed, and committed in a way that keeps history readable.

## What Good Contributions Look Like

- Solve one clear problem at a time.
- Keep changes aligned with the repo's architecture boundaries.
- Run the smallest verification set that proves the change is correct.
- Update docs, plans, or architecture notes when behavior or contracts change.
- Leave a commit history that explains what changed and why.

If you are unsure where to start, prefer a smaller slice with strong
verification over a large rewrite with unclear boundaries.

## Before You Start

Read the current project docs before making a non-trivial change:

- [README.md](README.md): project purpose, setup, and release-facing workflows
- [docs/README.md](docs/README.md): documentation index
- [docs/STANDARDS_ADOPTION.md](docs/STANDARDS_ADOPTION.md): shared standards adoption, enforcement, and exceptions
- [RELEASING.md](RELEASING.md): release validation and artifact expectations
- [scripts/launcher/README.md](scripts/launcher/README.md): shared launcher contract

This repository also follows the shared standards in the Coding Standards repo:

- `CODING-STANDARDS.md`
- `ARCHITECTURE-PATTERNS.md`
- `PLAN-STANDARDS.md`
- `COMMIT-STANDARDS.md`
- `TESTING-STANDARDS.md`
- `CROSS-PLATFORM-STANDARDS.md`
- `LAUNCHER-STANDARDS.md`
- `RELEASE-STANDARDS.md`

Use those standards as the source of truth when repo docs are intentionally
brief.

## Development Environment

Toolchain pins used by local development and CI:

| Tool | Pin Source | Current Pin |
| ---- | ---------- | ----------- |
| Rust | `rust-toolchain.toml` | `1.92.0` |
| Node.js | `.node-version` | `24.15.0` |
| pnpm | root `package.json#packageManager` | `10.33.0` |
| Python | `.python-version` | `3.12.3` |

General prerequisites:

- Rust toolchain with `clippy` and `rustfmt`
- Node.js with Corepack enabled
- Python 3.12 for helper scripts and local tooling
- A desktop environment or virtual display when validating Electron runtime
  startup

Initial setup from the repo root:

```bash
corepack enable
corepack pnpm install --frozen-lockfile
```

For desktop-app work, prefer the launcher wrappers instead of ad hoc command
sequences:

```bash
# Linux / macOS
./launcher.sh --install
./launcher.sh --build-release
./launcher.sh --run
```

```powershell
# Windows PowerShell
./launcher.ps1 --install
./launcher.ps1 --build-release
./launcher.ps1 --run
```

If PowerShell blocks local scripts, run:

```powershell
powershell -ExecutionPolicy Bypass -File .\launcher.ps1 --help
```

## Repository Map

| Path | Responsibility |
| ---- | -------------- |
| `rust/crates/pumas-core` | Core library logic, indexing, reconciliation, IPC-facing API surface |
| `rust/crates/pumas-rpc` | Desktop sidecar / RPC backend |
| `rust/crates/pumas-uniffi` | UniFFI bindings surface |
| `rust/crates/pumas-rustler` | Rustler bindings for Elixir/Erlang |
| `frontend/` | React application |
| `electron/` | Desktop shell, packaging, runtime integration |
| `scripts/launcher/` | Shared cross-platform launcher implementation |
| `docs/` | Architecture, plans, testing, and repo-specific guidance |

When choosing where a change belongs:

- Put reusable library behavior in Rust crates, not in Electron.
- Put desktop runtime glue in `electron/` or `scripts/launcher/`, not in the
  frontend.
- Put UI behavior in `frontend/`, not in ad hoc Electron renderer glue.
- Keep platform-specific behavior behind platform modules or factories.

## Change Planning

Write or update a plan when the change is:

- cross-module
- cross-platform
- cross-layer
- release-affecting
- likely to take more than one commit

Plans belong under `docs/plans/` and should follow the shared plan standards.

Small single-file fixes usually do not need a formal plan, but they still need
clear verification and an intentional commit.

## Code Expectations

### Architecture and Boundaries

- Follow the existing ownership boundaries in the repo.
- Do not hide business logic inside build scripts, shell wrappers, or UI event
  handlers.
- Keep desktop launcher behavior in the launcher layer.
- Keep machine-consumed contracts stable, or update producers and consumers in
  the same slice.

### Cross-Platform Discipline

- Use platform APIs and path helpers rather than string concatenation.
- Support spaces in paths end to end.
- When code resolves filesystem identity, compare canonical paths rather than
  raw display strings.
- Keep platform-specific behavior isolated rather than scattering checks across
  business logic.

### Error Handling and Logging

- Prefer typed or domain-specific errors over generic catch-all handling.
- Preserve context when re-throwing or mapping errors.
- Log useful operational context at process and boundary layers.
- Do not swallow failures silently.

For frontend code, keep error handling explicit and actionable. If a boundary
translates one error type into another, retain the original cause where
possible.

### Test Safety

- Tests that mutate global or durable state must isolate that state per test or
  deliberately serialize access.
- Avoid shared temp roots, shared sqlite files, and shared environment-variable
  mutation without guards.
- Add acceptance or integration verification when a change crosses layers.

## Validation Expectations

Run the smallest set of checks that proves your slice is correct. Use the table
below as the default baseline.

| Change Area | Minimum Verification |
| ----------- | -------------------- |
| Docs only | Read the rendered markdown and check links/commands you changed |
| Rust library logic | `cargo test -p pumas-library --manifest-path rust/Cargo.toml <targeted tests>` |
| Rust workspace or release-facing Rust change | `cargo test --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler` |
| Frontend UI or hooks | `npm run -w frontend test:run` and `npm run -w frontend check:types` |
| Electron shell | `npm run -w electron validate` and `npm run -w electron build` |
| Launcher changes | `npm run test:launcher` |
| Desktop packaging / runtime startup | `./launcher.sh --build-release` and `./launcher.sh --release-smoke` on a machine that can launch Electron |

Common commands:

```bash
# Rust
cargo test --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler
cargo clippy --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler -- -D warnings

# Frontend
npm run -w frontend test:run
npm run -w frontend check:types
npm run -w frontend build

# Electron
npm run -w electron validate
npm run -w electron build

# Launcher
npm run test:launcher
./launcher.sh --build-release
./launcher.sh --release-smoke
```

If your change affects `pumas_rustler`, validate it on a machine with
Erlang/OTP installed.

## Documentation Expectations

Update documentation in the same change when you alter:

- user-facing setup or usage flows
- launcher flags or canonical commands
- release steps or artifact expectations
- architecture boundaries
- machine-consumed contracts or generated artifacts

Typical documentation updates live in:

- `README.md`
- `CONTRIBUTING.md`
- `RELEASING.md`
- `docs/`
- directory `README.md` files required by the documentation standards

Do not leave new behavior discoverable only from code or commit history.

## Commits

Use conventional commits and keep each commit to one logical slice.

Preferred shape:

```text
type(scope): short description

Why the change was needed.
What approach was taken.
How it was verified.
```

Examples:

- `fix(library): stabilize canonical root comparisons in tests`
- `docs(contributing): rewrite contributor workflow for current repo`
- `ci(workflow): bootstrap pnpm in every frontend job`

Commit expectations for this repo:

- Make atomic commits.
- Include a meaningful body for non-trivial changes.
- Do not mix unrelated cleanup into the same commit.
- If code and docs must change together to stay accurate, commit them together.

## Pull Requests

A strong pull request makes review cheap. Include:

- the problem being solved
- the chosen approach
- the verification you ran
- any cross-platform or release risk
- screenshots when UI behavior changed

If a change is incomplete, say so explicitly instead of implying release
readiness.

## When You Are Unsure

Default to these decisions:

- prefer smaller, reviewable slices
- prefer explicit contracts over implied behavior
- prefer canonical launcher workflows over scattered raw commands
- prefer updating docs now instead of leaving drift for later
- prefer one well-verified fix over several speculative changes
