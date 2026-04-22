# Development Scripts

## Purpose
This directory contains developer-oriented setup, build, run, and SBOM helper scripts.

## Producer Contract
Scripts should be idempotent where practical, fail fast, and document externally installed tools they require. Release-affecting scripts must keep artifact names aligned with the release artifact contract.

## Consumer Contract
Developers and CI jobs may call these scripts from the repository root. Scripts should not require untracked local state unless the requirement is documented at the top of the script.

## Non-Goals
Runtime launcher behavior is out of scope. Reason: production launcher behavior belongs to `launcher.sh` and `scripts/launcher/`. Revisit trigger: promote a development script into a shipped runtime path.
