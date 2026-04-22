# Pass 04 - Python Torch Server, Launcher, Scripts

## Standards Consulted
- `LAUNCHER-STANDARDS.md`
- `SECURITY-STANDARDS.md`
- `CONCURRENCY-STANDARDS.md`
- `CROSS-PLATFORM-STANDARDS.md`
- `DEPENDENCY-STANDARDS.md`
- `TESTING-STANDARDS.md`
- `DOCUMENTATION-STANDARDS.md`

## Positive Baseline
- `launcher.sh` is root-level, Bash, uses `set -euo pipefail`, quotes paths, and delegates to `scripts/launcher/cli.mjs`.
- Launcher parser supports long-form flags, exactly one action, `--` passthrough only for run actions, usage output, and exit code `2` for usage errors.
- Launcher exposes `--install`, `--build`, `--build-release`, `--run`, `--run-release`, `--test`, and `--release-smoke`.
- The launcher has unit tests under `scripts/launcher/*.test.mjs`.
- `scripts/launcher/commands.mjs` uses `spawn(..., { shell: false })`.
- Torch server defaults to `127.0.0.1`.

## Findings

### P01 - Torch Server Request Validation Is Too Thin for Externally Reachable Mode
Status: non-compliant when LAN mode is enabled

`torch-server/control_api.py` accepts:

- `model_path: str`
- `model_name: str`
- `device: str`
- `host: Optional[str]`
- `api_port: Optional[int]`
- `max_loaded_models: Optional[int]`
- `lan_access: Optional[bool]`

Pydantic validates only broad Python types. The standards require boundary validation for paths, names, numeric ranges, and listener policy.

Rectification:
- Use Pydantic field validators:
  - `model_path` must be canonicalized and inside approved model roots;
  - `model_name` must have length and character constraints;
  - `device` must be `auto`, `cpu`, `cuda`, or a recognized device ID;
  - `api_port` must be 1-65535 with reserved-port policy;
  - `max_loaded_models` must be bounded;
  - `host` must be loopback unless LAN mode is explicitly enabled by trusted config.
- Add a shared path validator module for Torch server instead of inline checks.

### P02 - Torch Server Has No Visible Auth or Origin Policy for LAN Mode
Status: security risk

`configure` can set LAN mode by switching host to `0.0.0.0`. The server exposes model load/unload/configure endpoints. That may be intended, but the standards require listener limits and transport safety.

Rectification:
- Add a documented LAN threat model.
- Require an auth token, same-machine bridge token, or explicit trusted-network configuration before binding non-loopback.
- Add request concurrency limits and model-load queue limits.
- Log LAN mode startup prominently.

### P03 - Python ModelManager Has Shared Mutable State Without a Single Lock
Status: concurrency risk

`torch-server/model_manager.py` mutates:

- `self.slots`
- slot state fields;
- `self._device_locks`;
- `self.max_loaded_models`

Load operations use per-device locks only around the synchronous model load. Slot registry changes and max-model checks are not protected by a single manager lock. Concurrent `/api/load`, `/api/unload`, and `/api/configure` calls can race.

Rectification:
- Add an `asyncio.Lock` protecting slot registry and max-model state transitions.
- Keep expensive model loading outside the registry lock, but reserve slots atomically first.
- Make load/unload/configure state transitions explicit and idempotent.
- Add concurrent request tests.

### P04 - Torch Server Composition Root Uses Module-Global FastAPI App
Status: partial architecture issue

`torch-server/serve.py` declares a module-global `app = FastAPI(...)`, and `create_app()` mutates this global by adding routers and state. Multiple invocations in tests or embedded contexts can duplicate routers and leak state.

Rectification:
- Construct a fresh `FastAPI` inside `create_app`.
- Attach state and routers to the fresh instance.
- Add a test that calling `create_app()` twice does not duplicate routes or share managers.

### P05 - Python Tooling and Test Contract Is Missing
Status: partially remediated

No Python test/lint/type-check configuration was found in the audited files. `torch-server/requirements.txt` exists, but no `pytest`, `ruff`, `mypy`, or launcher/CI command is visible.

Rectification:
- Completed: `torch-server/README.md` documents the runtime, API boundary, and sidecar verification command.
- Completed: focused `unittest` coverage now exercises `create_app`, request validation, and model manager state transitions with fakes.
- Completed: `launcher.sh --test` runs the sidecar suite through platform-specific Python module commands, and CI runs the same suite in launcher verification.
- Remaining: add a formal Python lint/format/type-check policy, or document why the sidecar remains unit-tested only until a dedicated Python tooling package is introduced.

### P06 - Launcher Contract Is Mostly Implemented but Build/Release Semantics Need Clarification
Status: remediated

Launcher positives are strong. Remaining gaps:

- Completed: `--run` now checks the debug backend binary produced by `--build` and passes that path to Electron through `PUMAS_RPC_BINARY`.
- Completed: `--run-release` and `--release-smoke` pass the release backend binary produced by `--build-release`.
- Completed: Electron backend path resolution has package-local tests for launcher overrides, source build profiles, packaged resource paths, and platform executable names.
- Completed: CI smoke behavior is visible in `.github/workflows/build.yml` through bounded release smoke.
- Completed: `installDependencies` has injectable plan coverage for dependency command checks, workspace install invocation, check/install/recheck sequencing, failed installs, failed verification, and missing runtime dependency errors.

Rectification:
- No remaining follow-up in this finding.

### P07 - Shell Template TODO Uses `/tmp` Directly
Status: cross-platform/script hygiene issue

`scripts/templates/comfyui_run.sh` contains:

```text
TEMP_PROFILE_DIR="$(mktemp -d /tmp/comfyui-profile.XXXXXX)"
```

The standards require platform-aware paths and support for spaces in paths. This template is Linux-oriented, but the assumption should be documented or abstracted.

Rectification:
- Use `${TMPDIR:-/tmp}` and quote consistently.
- Document that the template is Linux-only if that is intended.
- Add a shellcheck pass to launcher/script tooling.

### P08 - Script and Generated-Artifact Directory Docs Are Missing
Status: documentation non-compliance

Missing READMEs from pass 1 include:

- `scripts/templates`
- `scripts/dev`
- `torch-server`
- `torch-server/loaders`

Rectification:
- Add READMEs with command contract, generated artifact ownership, runtime constraints, and dependencies.

## Pass 04 Refactor Inputs
- Torch request validation and path validator.
- Torch concurrency/state lock.
- Torch composition root cleanup.
- Python test/tooling addition.
- Launcher run/build semantic correction or documentation.
- Script template portability cleanup.
