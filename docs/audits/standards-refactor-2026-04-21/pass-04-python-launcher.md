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
Status: remediated

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
- Completed: `torch-server/validation.py` owns shared validation for model names, canonical model paths, approved model roots, device selectors, bind hosts, API ports, LAN policy, and model-slot limits.
- Completed: `control_api.py` validates request models at the Pydantic boundary before route handlers mutate manager state.
- Completed: `serve.py` validates startup host, port, and max-model configuration before constructing app state.
- Completed: sidecar unit tests cover path canonicalization, approved-root rejection, listener policy, and invalid configure behavior.

### P02 - Torch Server Has No Visible Auth or Origin Policy for LAN Mode
Status: remediated

`configure` can set LAN mode by switching host to `0.0.0.0`. The server exposes model load/unload/configure endpoints. That may be intended, but the standards require listener limits and transport safety.

Rectification:
- Completed: non-loopback bind hosts and `lan_access=true` require explicit `PUMAS_TORCH_ALLOW_LAN=1` opt-in.
- Completed: LAN binding also requires `PUMAS_TORCH_API_TOKEN`, and configured tokens are enforced on sidecar API routes through `X-Pumas-Torch-Token` or `Authorization: Bearer`.
- Completed: startup logs a warning when LAN access is enabled.

### P03 - Python ModelManager Has Shared Mutable State Without a Single Lock
Status: remediated; integration-test expansion tracked by D06

`torch-server/model_manager.py` mutates:

- `self.slots`
- slot state fields;
- `self._device_locks`;
- `self.max_loaded_models`

Load operations use per-device locks only around the synchronous model load. Slot registry changes and max-model checks are not protected by a single manager lock. Concurrent `/api/load`, `/api/unload`, and `/api/configure` calls can race.

Rectification:
- Completed: `ModelManager` owns a manager-level async registry lock for slot reservations, unload transitions, and `max_loaded_models` updates.
- Completed: expensive model loading runs outside the registry lock while slot capacity is reserved atomically first.
- Completed: tests cover concurrent load reservation and rejection behavior plus active-slot limit updates.
- Remaining: add route-level concurrent request tests when a heavier Python API integration harness is introduced.

### P04 - Torch Server Composition Root Uses Module-Global FastAPI App
Status: remediated

`torch-server/serve.py` declares a module-global `app = FastAPI(...)`, and `create_app()` mutates this global by adding routers and state. Multiple invocations in tests or embedded contexts can duplicate routers and leak state.

Rectification:
- Completed: `create_app()` constructs a fresh `FastAPI` instance, attaches fresh state, and includes routers on the new instance.
- Completed: tests verify repeated app construction does not duplicate routes or share model managers.

### P05 - Python Tooling and Test Contract Is Missing
Status: remediated

No Python test/lint/type-check configuration was found in the audited files. `torch-server/requirements.txt` exists, but no `pytest`, `ruff`, `mypy`, or launcher/CI command is visible.

Rectification:
- Completed: `torch-server/README.md` documents the runtime, API boundary, and sidecar verification command.
- Completed: focused `unittest` coverage now exercises `create_app`, request validation, and model manager state transitions with fakes.
- Completed: `launcher.sh --test` runs the sidecar suite through platform-specific Python module commands, and CI runs the same suite in launcher verification.
- Completed: `ruff.toml` defines the Python lint/format policy, `torch-server/requirements-dev.txt` pins the Python developer tool, and launcher/CI checks run Ruff lint and format verification before sidecar unit tests.

### P06 - Launcher Contract Is Mostly Implemented but Build/Release Semantics Need Clarification
Status: remediated

Launcher positives are strong. Remaining gaps:

- Completed: `--run` now checks the debug backend binary produced by `--build` and passes that path to Electron through `PUMAS_RPC_BINARY`.
- Completed: `--run-release` and `--release-smoke` pass the release backend binary produced by `--build-release`.
- Completed: `launcher.sh` no longer short-circuits `--run-release` into a stale `electron/release/linux-unpacked` binary when one happens to exist; the flag now consistently delegates through `scripts/launcher/cli.mjs` and runs the current release build outputs from `frontend/dist`, `electron/dist`, and `rust/target/release`.
- Completed: Electron backend path resolution has package-local tests for launcher overrides, source build profiles, packaged resource paths, and platform executable names.
- Completed: CI smoke behavior is visible in `.github/workflows/build.yml` through bounded release smoke.
- Completed: `installDependencies` has injectable plan coverage for dependency command checks, workspace install invocation, check/install/recheck sequencing, failed installs, failed verification, and missing runtime dependency errors.

Rectification:
- No remaining follow-up in this finding.

### P07 - Shell Template TODO Uses `/tmp` Directly
Status: remediated

`scripts/templates/comfyui_run.sh` contains:

```text
TEMP_PROFILE_DIR="$(mktemp -d /tmp/comfyui-profile.XXXXXX)"
```

The standards require platform-aware paths and support for spaces in paths. This template is Linux-oriented, but the assumption should be documented or abstracted.

Rectification:
- Completed: the template now uses `${TMPDIR:-/tmp}` as a temporary base and quotes the `mktemp` template.
- Completed: `scripts/templates/README.md` documents the temporary-directory contract for shell templates.
- Remaining: shellcheck enforcement remains a broader script-tooling follow-up outside this finding.

### P08 - Script and Generated-Artifact Directory Docs Are Missing
Status: remediated

Missing READMEs from pass 1 include:

- `scripts/templates`
- `scripts/dev`
- `torch-server`
- `torch-server/loaders`

Rectification:
- Completed: `scripts/dev/README.md`, `scripts/templates/README.md`, `torch-server/README.md`, and `torch-server/loaders/README.md` document command contracts, ownership, runtime constraints, and dependencies.

## Pass 04 Refactor Inputs
- Completed: Torch request validation and shared path/listener validator.
- Completed: Torch concurrency/state lock.
- Completed: Torch composition root cleanup.
- Completed: Python test/tooling addition.
- Completed: Launcher run/build semantic correction.
- Completed: Script template portability cleanup.
- Completed: LAN token authentication beyond explicit trusted-network opt-in.
