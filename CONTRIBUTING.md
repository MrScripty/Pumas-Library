# Contributing to Pumas Library

## Start Here

Read [Architecture](docs/ARCHITECTURE.md) before changing ownership or boundary
behavior, and [Development](docs/DEVELOPMENT.md) for setup and commands.
Release-affecting changes also require [Releasing](RELEASING.md).

Pumas follows the standards in:

```text
/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/
```

Begin with `CORE-STANDARDS.md`, then route the change through
`STANDARDS-ROUTER.md`. Legacy monolithic standards filenames are not normative.

## Toolchains

| Tool | Pin |
| --- | --- |
| Rust | `rust-toolchain.toml` |
| Node.js | `.node-version` |
| pnpm | root `package.json#packageManager` |
| Python | `.python-version` |

Install workspace dependencies with:

```bash
corepack enable
corepack pnpm install --frozen-lockfile
```

The root launchers provide the supported desktop workflow:

```bash
./launcher.sh --install
./launcher.sh --build
./launcher.sh --run
```

Use the same flags with `.\launcher.ps1` on Windows.

## Put Changes With Their Owner

| Concern | Owner |
| --- | --- |
| Library state, metadata, persistence, downloads | `pumas-core` |
| Runtime installation and external service clients | `pumas-app-manager` |
| HTTP/RPC transport and gateway | `pumas-rpc` |
| Desktop privileges, process launch, IPC | `electron` |
| UI state and presentation | `frontend` |
| Torch inference service | `torch-server` |
| Host-language conversion | binding adapter crate |

Decode untrusted values at the receiving boundary. Keep backend-owned state in
the backend and make cached, degraded, invalid, unsupported, and empty outcomes
distinguishable.

## Planning

A bounded local change with an obvious write set, regression check, and
acceptance path does not need a written plan merely because it touches multiple
files or layers.

Use a written plan when uncertainty, persistence migration, contract evolution,
cross-team coordination, or destructive rollout needs durable sequencing. An
active plan must have a clear status, acceptance claims, current phase,
blockers, and exactly one next slice. Remove terminal execution plans once
their durable decisions and current behavior are documented elsewhere; Git is
the historical archive.

## Verification

Run the smallest evidence set that proves the change. Common baselines are:

| Area | Commands |
| --- | --- |
| Rust | `./scripts/rust/check.sh` or targeted `cargo test --manifest-path rust/Cargo.toml -p <crate>` |
| Frontend | `npm run -w frontend lint`, `check:types`, and targeted/full `test:run` |
| Electron | `npm run -w electron lint`, `test`, and `validate` |
| Desktop contract | `pnpm --dir electron check:desktop-contract` after changing a Rust wire type or an `electron/contracts/` schema |
| Launcher | `npm run test:launcher` |
| Torch | Ruff checks plus `python3 -m unittest discover -s torch-server/tests` |
| Release startup | `./launcher.sh --build-release` and `--release-smoke` |

[Development](docs/DEVELOPMENT.md#permanent-verification-inventory) owns the
exact claims, proof limits, blocking authority, environments, and schedules of
the retained permanent gates.

Tests must isolate filesystem, database, process, port, and environment state.
For cross-process changes, include producer and consumer evidence. For UI
changes, verify the interaction in a representative Electron/browser runtime;
jsdom alone does not prove focus, layout, or assistive-technology behavior.

## Documentation

Update docs in the same slice when a user workflow, ownership boundary,
machine-consumed contract, security posture, or release process changes.

Prefer updating one current owner document over adding another partial guide.
Do not add a README because a directory exists. Completed plans, investigation
logs, command transcripts, and generated dependency reports should not remain
as permanent guidance unless they have an explicit continuing consumer.

## Commits

Use conventional commits and keep each commit to one coherent change:

```text
type(scope): concise outcome
```

Examples:

- `fix(model-library): preserve artifact identity during recovery`
- `docs(architecture): clarify desktop state ownership`
- `test(rpc): reject malformed model import requests`

Before committing, inspect the staged diff, run the relevant evidence, and make
sure documentation describes the behavior that actually landed.
