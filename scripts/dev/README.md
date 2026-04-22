# Development Scripts

## Purpose
This directory contains developer-oriented setup, build, run, and SBOM helper scripts.

## Contents
| File | Description |
| ---- | ----------- |
| `build.sh` | Delegates release build behavior to the root launcher contract. |
| `check-commit-message.sh` | Validates commit subjects against the project conventional commit format. |
| `check-readme-coverage.sh` | Verifies that standards-controlled source and support directories include `README.md` contracts. |
| `generate-sbom.sh` | Generates dependency SBOM snapshots for supported ecosystems. |
| `list-audit-files.sh` | Prints the source tree view used by standards audits while excluding generated/runtime paths and retaining tracked plugin manifests. |
| `run-dev.sh` | Delegates development launch behavior to the root launcher contract. |
| `setup.sh` | Delegates local setup behavior to the root launcher contract. |

## Producer Contract
Scripts should be idempotent where practical, fail fast, and document externally installed tools they require. Release-affecting scripts must keep artifact names aligned with the release artifact contract.

## Consumer Contract
Developers and CI jobs may call these scripts from the repository root. Scripts should not require untracked local state unless the requirement is documented at the top of the script.

## Non-Goals
Runtime launcher behavior is out of scope. Reason: production launcher behavior belongs to `launcher.sh` and `scripts/launcher/`. Revisit trigger: promote a development script into a shipped runtime path.
