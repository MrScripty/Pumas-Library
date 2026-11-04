# Developer Scripts

Scripts automate repeatable repository workflows. Their executable behavior is
the command contract; keep this guide focused on entry points rather than
duplicating implementation details.

## Desktop Launcher

`launcher.sh` and `launcher.ps1` delegate to the shared Node implementation in
`scripts/launcher/`.

```bash
./launcher.sh --install
./launcher.sh --build
./launcher.sh --run
./launcher.sh --build-release
./launcher.sh --release-smoke
npm run test:launcher
```

Run either root launcher with `--help` for all actions and exit semantics. Add
cross-platform behavior to the shared implementation unless the difference is
inherently shell- or platform-specific.

## Rust

```bash
./scripts/rust/check.sh
./scripts/rust/check.sh test-isolation
./scripts/rust/check.sh blocking-audit
```

The default command runs the Rust workspace evidence set while excluding the
Erlang-hosted Rustler crate. Individual subcommands are listed by `--help`.

## Bindings

```bash
./scripts/generate-bindings.sh <language|all>
./scripts/check-uniffi-surface.sh
./scripts/check-uniffi-csharp-smoke.sh
./scripts/package-uniffi-csharp-artifacts.sh
```

Generation and packaging can require external host toolchains. See
[Native bindings](../docs/native-bindings.md) before publishing output.

## Release, Qualification and Consumers

| Directory | Purpose |
| --- | --- |
| `release/` | Artifact plan and inventory checker, runtime staging and probes, package smoke checks, notices and release metadata; [Releasing](../RELEASING.md) owns their use |
| `tests/` | Qualification runners and contract tests for S3 RPC paths and Hugging Face cached search |
| `acceptance/` | Real-process acceptance helpers (download-location reconciliation, FLUX.2 RPC); point them only at temporary libraries |
| `consumers/` | Reference consumers of the local HTTP owner contract, not supported SDKs; see the [local HTTP consumer contract](../docs/contracts/local-http-consumer.md) |

## Governance and Development Helpers

| Script | Purpose |
| --- | --- |
| `dev/check-commit-message.sh` | Validate conventional commit subjects |
| `dev/check-release-version-alignment.mjs` | Compare release manifest versions |
| `dev/check-workspace-dependency-ownership.mjs` | Enforce pnpm dependency ownership |
| `dev/list-audit-files.sh` | List non-generated files in standards-audit scope |
| `system-check.sh` | Inspect required local tools |

There is deliberately no blanket directory-README checker. Current standards
require documentation for durable users, decisions, contracts, and workflows,
not one file per directory.

When changing a script, verify its normal path, invalid arguments, failure
propagation, working-directory independence, and any platform-specific branch.
