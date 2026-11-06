# Comprehensive Audit — 2026-11-02

## Baseline and method

- Branch `feat/v080-consumer-api`. The audit began at `1b1438ec` (manifests `0.8.0-rc.1`).
  Three commits landed during the audit: `79fd6c51` (native QA tests), `8d03e4da`
  (llama.cpp pooled batch sizing in `runtime_profiles.rs`), `b1196838` (version
  set to `0.8.0`, attribution regenerated under `docs/release-attribution/0.8.0`).
  Line numbers below refer to the start commit; only `runtime_profiles.rs` and
  `acquisition/workspace.rs` moved materially.
- Static review only. Nothing was built, compiled, type-checked, linted, tested
  or installed. No source, test, config or lockfile was changed; this file is the
  only addition and is untracked.
- Nine parallel area reviews (core persistence; HF acquisition and S3; runtime,
  process and IPC; `pumas-rpc`; `pumas-app-manager`; Electron; React frontend;
  `torch-server`; CI, release, dependencies, docs and hygiene). Every High
  finding was independently re-verified against the source by the coordinating
  reviewer. Confidence is marked CONFIRMED (code path traced) or SUSPECTED.
- Not performed: `cargo audit`, `cargo deny`, `pip-audit` (not installed);
  an OSV batch query over all three lockfiles (1,285 package-versions) returned
  matches but the per-advisory detail fetch was interrupted and the rerun and
  `pnpm audit` were cancelled by the owner. This audit therefore carries **no
  vulnerability inventory**; see GOV-02.
- Severity follows the repository's own scale (Critical / High / Medium / Low).

| Area | Approx. lines | Notes |
| --- | --- | --- |
| `pumas-core` | 251K | `hf/download.rs` 23.9K (70% tests), `library.rs` 15.1K |
| `pumas-rpc` | 54K | `contract.rs` 9.1K |
| `pumas-app-manager` | 34K | |
| `pumas-uniffi` + `pumas-rustler` | 2.3K | experimental bindings |
| Electron | 56K | includes 2.2MB generated validators |
| Frontend | 102K | 139 test files, 752 cases |
| `torch-server` | 20K | 354 tests |
| Scripts / CI | 22K + 1,043-line workflow | |

Static counters: TypeScript has 0 `any`, 0 `eslint-disable`, 0 `ts-ignore`,
0 `console.log`; Rust has 83 `unsafe` expressions in 13 opted-in files (0 found
unsound; 24 undocumented, see RT-05/APM-08), ~2,500 `unwrap` and ~315 `expect`
in non-test paths (none found on raw network or on-disk input; several are
test-injection hooks compiled into production modules); 7 module-level
`#![allow(dead_code)]` all in the audio runtime; no secrets in tracked files.

## Verdict

Pumas Library is **strong inside each process and still exposed at four
boundaries**. Since the September standards audit, the team closed launcher-root
durability, the launcher kill chain, the old line-count standards, the credential
logging path in Rust, most async ownership, and the dialog/popover accessibility
primitives. The acquisition pipeline (HF, S3, llama.cpp) is now a model of
verify-before-trust.

What remains is concentrated: (1) the Electron window-creation boundary is
unguarded while the renderer opens metadata-controlled links in-app; (2) legacy
process control kills by command-line substring and trusts PID files; (3) the
Ollama runtime is downloaded, cached and executed with no integrity check while
the llama.cpp path beside it is exemplary; (4) model deletion and the governance
table rebuild are not atomic in SQLite; (5) the Torch text path blocks the event
loop and the sidecar is launched unauthenticated and unconfined; (6) the release
has no vulnerability or license evidence producer and an inert dependency
override block. None of these is Critical by the repository scale, but ELE-01
is one user click away from handing a remote page the full privileged API and
should be fixed before 0.8.0 ships.

Prior findings: 4 closed, 3 largely closed, 7 partial, 1 open (table below).

## High findings (8)

| ID | Confidence | Finding | Evidence |
| --- | --- | --- | --- |
| **ELE-01 / FE-01** | CONFIRMED | **No `setWindowOpenHandler`, `will-navigate`, `will-redirect` or permission handler anywhere in Electron**, while the renderer renders `target="_blank"` anchors whose `href` comes from embedded GGUF/HF metadata and user notes, and calls `window.open` in four hooks. A child window inherits the opener's webPreferences including the preload; `api:call` has no sender validation (3 of 20 IPC channels check `senderFrame`). One click on a metadata-supplied link gives a remote page all 183 `window.electronAPI` members (178 RPC methods incl. `set_hf_token`, `delete_model_with_cascade`, `import_model`). | `electron/src/main.ts:276-293,520`; `frontend/src/components/ModelMetadataGrid.tsx:128-131`, `ModelNotesMarkdownPreview.tsx:39-44`, `model-import/ImportMetadataDetailsParts.tsx:118-121`, `ImportLookupCard.tsx:49-50`; `hooks/useInstallDialogLinks.ts:38-64`, `useModelLibraryActions.ts:77`, `HuggingFaceAuthDialog.tsx:210`. Safe path exists: `api.open_url` → `validateExternalUrl` (`ipc-validation.ts:330-342`). |
| **RT-01** | CONFIRMED | **Legacy stop kills unrelated processes by case-insensitive command-line substring** (`"ollama serve"`, `"serve.py"`) via `ps -eo pid=,args=`; every non-owned match gets SIGTERM→SIGKILL. Reachable from RPC `stop_ollama`/`stop_torch` and the desktop. A user's system Ollama, another project's `python serve.py`, or `vim serve.py` dies; a match owned by another user fails the whole stop. | `pumas-core/src/process/manager.rs:245,427,504-514`; `platform/process.rs:476-512`; `pumas-rpc/src/contract.rs:558-560`. |
| **RT-02** | CONFIRMED | **PID-file stops have no identity check**: `ollama.pid`/`torch.pid` persist across reboots; liveness is `kill(pid,0)`; a reused PID that leads a group gets `killpg`. | `process/manager.rs:225-228,408-411`; `process/launcher.rs:398-401`; `platform/process.rs:55-66,427-448`. |
| **PERS-01** | CONFIRMED | **Index `delete` commits the row removal and the durable update-feed event as two autocommits**; `upsert` correctly wraps both in one transaction. A crash in between leaves cursor-following consumers never learning of the removal (the CS-03 "row without event" case). Production path for model deletion. | `pumas-core/src/index/model_index.rs:813-835` vs `:423-435`; `model_library/library.rs:2900-2906`. |
| **PERS-02** | CONFIRMED | **Governance table rebuilds run RENAME→CREATE→INSERT→DROP via `execute_batch` with no BEGIN**. Crash after RENAME: next start creates an empty table, reseeds system rows, and user-authored rules sit orphaned in `*_legacy` with no error. Crash after CREATE: the marker text is present so the rebuild never re-runs. | `index/model_index/governance.rs:245-293,295-340`; `model_index.rs:297`. |
| **APM-01** | CONFIRMED | **Ollama runtime downloaded and extracted with no integrity check**: a cached archive is trusted when only its size matches, `download_archive` never hashes, the client is not `https_only`, and `finalize_ollama_binary` skips `validate_native_output` so `set_permissions(0o755)` can act through an archive-supplied symlink. llama.cpp beside it requires the publisher digest and goes through the verified acquisition workspace. | `pumas-app-manager/src/version_manager/installer.rs:1831-1856,2341-2406,2821-2857,3053-3144,3287-3452` vs `:1939-1954`. |
| **TS-01** | CONFIRMED | **Torch text inference runs synchronously on the asyncio loop**: `async def` handlers call `_generate` → `model.generate` inline, without the device lease. While a generation runs, `/health` (Rust liveness), the owned-generation disconnect poll and all control routes stall; CUDA OOM surfaces as a bare 500 with no reason or `empty_cache`. The image path already does this correctly. | `torch-server/openai_api.py:139-167,194-213`; `model_manager.py:518-528`. |
| **GOV-02** | CONFIRMED | **No vulnerability or license evidence producer exists**: `build.yml` has no audit/deny/pip-audit/SBOM step; `.github/` has no Dependabot config; the only evidence is `docs/release-evidence/0.7.0`; `RELEASING.md:347` points 0.8.0 readers to the 0.7.0 dependency review; `artifact-plan.json:517` requires `current-vulnerability-results` with nothing producing it. Combined with this audit's incomplete scan, 0.8.0 has no dependency-risk statement. | `.github/workflows/build.yml` (grep), `scripts/release/artifact-plan.json:517`, `RELEASING.md:347`. |

## Medium findings (39)

### Security boundary and trust

- **TS-02** CONFIRMED — Torch model-path confinement is dormant: `PUMAS_TORCH_MODEL_ROOTS` is read only by `validation.py:46-47` and set nowhere in Rust/Electron/scripts, so `/api/load` accepts any existing path; `pipeline_path`, `vae_path`, `model_type` are unvalidated `Optional[str]` flowing into `from_pretrained`/`load_file` (`control_api.py:43-45`, `diffusion.py:93-114`, `flux2.py:84-110`).
- **TS-03** CONFIRMED — The sidecar runs unauthenticated: auth middleware installs only if `PUMAS_TORCH_API_TOKEN` is set and the launcher never sets it (`serve.py:66-67`; `pumas-core/src/runtime_profiles/launch_specs.rs:180-185` sets only offline/nousersite vars). Compare is constant-time and the token is never in argv, so a per-launch random token is cheap to add.
- **APM-03** CONFIRMED — RPC `install_version_dependencies` → `DependencyManager` runs PATH `python3 -m venv` before checking for `requirements.txt`, pip without `--require-hashes`/`--isolated`, and `version_path(tag)` is a bare `join`, so a `../x` tag escapes `<app>-versions/` (`pumas-rpc/src/handlers/versions/deps.rs:18-26`; `version_manager/mod.rs:618-621,1550-1566`; `dependencies.rs:217-345`).
- **APM-02** CONFIRMED — Torch direct install records hashes after the fact: `resolve_runtime.py:1320-1343` runs pip with a mixed PyTorch+PyPI index set and no `--require-hashes`/`--no-deps`; staged files are compared by size only (`installer/torch.rs:1741-1797`). The retained-plan path does it right (`:2354-2371`).
- **APM-04** CONFIRMED — Exported but unwired `OllamaVersionManager::extract_zip` is zip-slip vulnerable (`ollama.rs:627-660`, no `enclosed_name()`, no hash).
- **RPC-03** CONFIRMED — Provider-facing reqwest clients honor `HTTP_PROXY`/`ALL_PROXY` (no `.no_proxy()`), so prompts and embeddings destined for 127.0.0.1 providers can route through a user proxy (`handlers/openai_gateway.rs:51-61`, `server.rs:730-742`, `model_operations/decision.rs:42-49`, `vision.rs:52-59`, `handlers/plugins.rs:54-57`). `owner_retention.rs:242` and pumas-core clients already use `.no_proxy()`.
- **RPC-04** SUSPECTED — CS-11 residual: default mode still substitutes `std::env::temp_dir()/pumas-plugins-fallback` when the configured plugin loader fails and loads every `*.json` from that world-writable path (`startup.rs:144-153`; `pumas-core/src/plugins/loader.rs:24-76`). The `unwrap` is gone; bootstrap mode fails closed.
- **ELE-05** CONFIRMED — CS-01 residual in Electron: every sidecar stdout/stderr chunk is persisted at `info` (`python-bridge.ts:550-564`, `main.ts:56`); `--dev/--debug` sets `RUST_LOG=debug`; `Invalid JSON response: ${data}` carries the raw body to the renderer (`python-bridge.ts:1063`). Safety now rests on Rust emitting nothing sensitive.
- **RT-09** SUSPECTED — Local IPC auth rests on registry DB file mode: UUID token in `registry.db` created under default umask, loopback TCP with no peer-credential check (`registry/library_registry.rs:163-171,687,895`; `ipc/server.rs:199,250`).
- **ACQ-01** CONFIRMED — `repo_id` is interpolated unvalidated into authenticated HF API URLs on read paths (`hf/metadata.rs:112-119,256-289`; `hf/bundles.rs:54-64`; `hf/search.rs:309-318`); `validate_hf_repo_id` is applied only on the download path. Same-origin injection only; host is constant.

### Data integrity and persistence

- **PERS-03** CONFIRMED — The main index has no explicit schema identity (`CREATE TABLE IF NOT EXISTS` + sqlite_master text sniffing, `governance.rs:191-243`, `fts5.rs:57-82`); unknown or newer schemas are accepted. The intent tables and download store refuse unknown schemas with reasons.
- **PERS-04** CONFIRMED — Package-facts cache mutations and events are separate autocommits (`package_facts_cache.rs:~60-131,~255-285`).
- **PERS-05** CONFIRMED — Corrupt JSON columns decode as empty/Null and present as valid models (`model_index.rs:1046-1055` `unwrap_or_default`), contradicting `list_all`'s stated behavior (`:790-792`); `strict_catalog_snapshot` does it correctly.
- **PERS-06** CONFIRMED — `MetadataManager` read-modify-write outside the lock (`metadata/manager.rs:288-355`).
- **PERS-07** CONFIRMED — Review edits span three independent commits (overlay tx, metadata.json, index row+event) (`library.rs:2358-2366,2407-2417`).
- **PERS-08** CONFIRMED — Legacy `atomic_write_json` lacks parent-directory fsync and plain-copies `.bak`; it writes model `metadata.json`, link registry, mapper, migration checkpoints and launcher metadata (`metadata/atomic.rs:982-1083`). The durable `AtomicJsonTarget` (used for downloads.json) already solves this.

### Runtime, process and lifecycle

- **RT-03** CONFIRMED — ONNX load and inference run synchronously inside `async fn` under one std Mutex; callers do not offload (`onnx_runtime/real_backend.rs:90-98,132-148`; `pumas-rpc/src/handlers/openai_gateway_onnx.rs:62-65`).
- **RT-04** CONFIRMED — Flat ONNX limits (`MAX_EMBEDDING_INPUTS=128`, 65,536 chars, 8,192 tokens/dims) never consult `max_position_embeddings`; over-length input fails as opaque "ONNX Runtime inference failed" (`onnx_runtime/mod.rs:37-40,360-399`; `tokenizer.rs:8,55,78-94`; `config.rs:41-55`). Violates "no flat limits / every rejection has a reason".
- **RT-05** CONFIRMED — Windows `ManagedChild` has a module-level `#[allow(unsafe_code)]` with 23 unsafe expressions and zero `SAFETY:` comments (`platform/managed_child.rs:596-598,639-850`). Traced sound.
- **RT-06** SUSPECTED — Linux/macOS owned children survive a parent crash (no PDEATHSIG/pidfd; Windows has a kill-on-close Job) and the leftover PID file then blocks relaunch (`managed_child.rs:165`; `runtime_profiles/process_owner.rs:900-908`).
- **TS-04** CONFIRMED — Image width/height have no upper bound; `MAX_PNG_BYTES` is checked after generation; admission never consults `mem_get_info` (`image_api.py:33,40-41,114`; `diffusion.py:31-44`).
- **TS-05** CONFIRMED — Text limits are flat constants (`INPUT_TEXT_MAX_CHARS=1_000_000`, `OUTPUT_TOKENS_MAX_ITEMS=4_096`) with no truncation against the model's positions (`openai_api.py:19-22,200`).

### Contracts, decoding and reasons

- **RPC-01** CONFIRMED — Pre-dispatch rejections return bare status codes with empty bodies: fence 400/503/412 (`discovery.rs:134-147,197`), shutdown 503 (`server.rs:723-725`), owner retention 501/428/400/503/412/429 (`owner_retention.rs:56-79`), transport 500/503 (`http_transport.rs:151-153`). Direct violation of the owner's rule that every rejection carries a reason.
- **RPC-02** CONFIRMED — Typed JSON-RPC admission maps every serde error to the fixed sentence "Request parameters are invalid." (`contract.rs:6977-7017`); S3 admission discards `validate()` detail (`s3_imports.rs:101-356`). Legacy handlers and model operations do name the field.
- **ELE-02** CONFIRMED — Generated response and stream validators are closed (`additionalProperties:false` on every DTO, `pumas-rpc/src/contract/export.rs:1809-1811`), so an additive backend field or enum value breaks the desktop until regeneration. Intentional and documented, but it conflicts with "additive = non-breaking" when `PUMAS_RPC_BINARY` lets a packaged desktop run a different backend (`backend-path.ts:19-22`). Owner decision needed.
- **ELE-03** CONFIRMED — CS-02 residual: of 178 allowlisted methods, 88 have request validation beyond "plain object" (31 Rust-generated, 47 hand-written, 15 empty-record) and 90 accept any object, including `set_hf_token`, `import_batch`, `import_model`, `torch_configure`, `delete_model_with_cascade`; ~88 responses return raw (`rpc-method-registry.ts:10-189,255-456`; `ipc-validation.ts:65-175`).
- **ELE-04** CONFIRMED — Four of five SSE streams report transport loss only to the main log; only serving-status has an error channel (`python-bridge.ts:270-291`; `main.ts:700-783`).
- **FE-02** CONFIRMED — Pull-path responses reach hooks undecoded while the matching push path is decoded: 88 plain `apiCall` vs 68 `validatedApiCall` in preload; `get_status_telemetry_snapshot`, `get_runtime_profiles_snapshot`, `get_serving_status`, `search_hf_models`, `get_network_status`, `get_library_status` are consumed typed (`preload.ts:294-730`; `statusTelemetryStore.ts:151-153`; `useRemoteModelSearch.ts:114`).
- **FE-03** CONFIRMED — Nine silent-default sites: `scanModels`/`searchModelsFTS` catch-and-log, APIError message dropped before "Library unavailable", startup download-list failure indistinguishable from empty, disk space 0% on failure (`useModels.ts:92-116,245-289`; `useModelDownloads.ts:109-111`; `useActiveModelDownload.ts:103-105`; `useDiskSpace.ts:16-33`; two app-panel sections).
- **FE-04** CONFIRMED — CS-05 residue: persisted snapshot has no `writtenAt`/revision, and every debounced refresh re-marks rows `cached` so star/actions flicker off (`modelLibrarySnapshot.ts:3-131`; `useModels.ts:60,262-274`; `LocalModelRowState.ts:168-173`).
- **FE-05** CONFIRMED — Tooltip primitive is hover-only, `role="tooltip"` without `aria-describedby`, no focus trigger; 14 usages (`ui/Tooltip.tsx:26-47`).
- **ACQ-02** CONFIRMED — Search results carry no freshness/provenance field; `CacheStrategy::allow_stale_on_offline` and `NetworkManager::fetch_with_cache` are latent stale-as-fresh paths with no production callers (`model_library/hf_cache.rs:422-506`; `network/manager.rs:380-446`).

### Governance, release and dependencies

- **GOV-01** CONFIRMED — Root `package.json:16-21` uses npm-style top-level `overrides`; pnpm reads only `pnpm.overrides`, the lockfile has no `overrides:` section and resolves `minimatch@10.2.6`/`brace-expansion@1.1.21` (not the pinned 10.2.5/1.1.18). The security-floor mechanism is inert; exposure is nil today because natural resolution is newer. (Downgraded from the area reviewer's High.)
- **GOV-03** CONFIRMED — Bindings claims contradict code and CI (CS-07): `docs/native-bindings.md:5-6` says "CI generates archives" but `build.yml` has no bindings job; `support-matrix.json` lists `removed_surfaces` for all seven surfaces yet all remain; `pumas-uniffi/src/lib.rs:4,13` still claims Go; 88 `uniffi` references and `uniffi::setup_scaffolding!()` remain in `pumas-core`; `generate-bindings.sh:66` runs an unpinned `cargo install` with no `set -e`.
- **GOV-04** CONFIRMED — Action pinning is inconsistent: `build.yml` uses mutable tags (`checkout@v6`, `setup-node@v6`, `rust-cache@v2`, `upload-artifact@v7.0.1`); `scorecard.yml` SHA-pins except `codeql-action/upload-sarif@v3`; actionlint and ORT downloads are hash-pinned.
- **GOV-05** CONFIRMED — Frontend library-only build runs only on tags and its `dist/` is immediately overwritten by the default build, with no artifact check (`build.yml:446-452`; CS-08 frontend half still open).
- **GOV-06** CONFIRMED — `DEVELOPMENT.md` verification inventory under-describes CI: `torch-quality` runs ~45 native cargo-test steps on 3 OSes per PR; `review-inference-linux`, `review-audio-dependencies`, S3 and attribution gates are absent from the table.
- **GOV-07** CONFIRMED — Docs index and plan lifecycle drift (CS-14): `docs/README.md` lists 2 of 3 ADRs and 4 of 17+16 plans; six terminal plans remain under `docs/plans/`; `docs/breif/` (misspelled) is unindexed; `docs/contracts/local-http-consumer.md:82` references `contracts/capability-descriptor.schema.json`, which is not tracked anywhere.
- **GOV-08** CONFIRMED — Python pin divergence: `build.yml:507,646` use floating `python-version: '3.14'` while `.python-version` is 3.12.3; `torch-server/requirements.txt` is `>=`-only and still the documented dev install; CI pip installs are unhashed. The shipped `runtime/requirements.lock` (66 packages, 939 hashes) is good; `SECURITY.md:56-57` saying it is "not reproducibly locked" is stale.
- **GOV-09** SUSPECTED — 19.7MB of 53.5MB tracked bytes are `docs/release-attribution`; the 0.7.0 directories (~9MB) have no consumer; four `command-logs.tar.gz`, a force-added `rpc.log` and 300KB+ ledgers sit in `docs/plans`.

## Low findings (48, condensed)

- **Electron**: ELE-06 orphan sidecar on hard crash, health failures only warn, no parent watchdog in pumas-rpc; ELE-07 model-download subscription counters not reset on window close (macOS leak); ELE-08 `PUMAS_RPC_BINARY` honored when packaged, port pre-allocated then released before spawn, readiness trusts any `{status:'ok'}`; ELE-09 DevTools reachable via default menu in production, CSP `connect-src http://127.0.0.1:*` broader than needed, dev-server origin bypasses the IPC allowlist in `--dev`; ELE-10 packaging ships `.map`/`.d.ts`, unsigned, no mac hardened runtime; ELE-11 launcher-root reason text dropped before the renderer.
- **Frontend**: FE-06 installation poll loop repeats every 800ms forever on transport failure; FE-07 library-only variant unverified beyond compile; FE-08 seven dead modules (`usePlugins.ts` with a 5s interval, `ResourceMonitor`, `StatusDisplay`, `SpringyToggle`, `GenericAppPanel`, `TorchServerConfigSection`, `config/theme.ts` which docs still cite) and no-op `mergeSnapshot`/`force` plumbing; FE-09 CSP informational.
- **torch-server**: TS-06 text loaders omit `use_safetensors=True` (mitigated by torch 2.9.1 `weights_only` default); TS-07 no parent-liveness watchdog, orphan cleanup is substring match (see RT-01); TS-08 control API echoes `str(error)` with resolved paths; TS-09 two request models lack `extra="forbid"`, `torch.cuda.synchronize` in `finally` masks the adapter error on non-CUDA; TS-10 CI never installs runtime deps or runs `tests_native`, no `pip-audit`; TS-11 `model_index.json` class pairs unvalidated under unvalidated `pipeline_path`.
- **Runtime**: RT-07 owned stops go straight to SIGKILL with no graceful phase (`linux_group.rs:94`); RT-08 IPC capacity rejection is a silent EOF, no idle timeout, 64-connection cap; RT-10 ORT dylib preflighted per load but `ort::init_from` is first-wins (TOCTOU on operator path); RT-11 legacy observer thread loops forever on persistent errors.
- **Persistence**: PERS-09 `search_fts5` masks failures as empty results; PERS-10 FTS setup drop/create/populate are separate autocommits; PERS-11 public legacy registry methods bypass the claim protocol (test-only callers); PERS-12 `ModelIndex::clear()` non-transactional (test-only); PERS-13 core errors embed raw launcher-root paths (redaction is RPC's job, per SECURITY.md).
- **App-manager**: APM-05 legacy process factory drops the `Child` (zombies), trusts PID files nothing writes, SIGKILL after fixed 500ms; APM-06 no cumulative size/entry bounds on extraction, zip preserves setuid bits; APM-07 Torch children inherit the full environment (`LD_PRELOAD`, `LD_LIBRARY_PATH`, `SSL_CERT_FILE`) while `managed_python.rs` already `env_clear`s; APM-08 one `unsafe { zeroed() }` without SAFETY (sound); APM-09 Ollama client embeds unbounded upstream bodies in errors, `expect` in constructor, no loopback check on `base_url`.
- **Acquisition**: ACQ-03 public legacy `network::DownloadManager` bypasses the acquisition contract (no callers); ACQ-04 HF token file written under default umask then chmod'd (non-atomic); ACQ-05 `HF_DOWNLOAD_MAX_RETRIES=0` plus `PUMAS_HF_DOWNLOAD_MAX_RETRY_ELAPSED_SECS=0` yields unbounded retry; ACQ-06 corrupt repo-tree JSON cache blocks downloads instead of refetching; ACQ-07 public `HuggingFaceClient::start_download` uses a Weak revision (cannot resume); ACQ-08 per-chunk double copy plus `spawn_blocking` per 16KiB chunk.
- **RPC**: RPC-05 no header-read/idle deadline (hyper builder has no timer, so the 30s default is silently dropped), unbounded connection JoinSet; RPC-06 non-generation gateway client follows redirects, router client reads unbounded bodies; RPC-07 legacy `/v1` gateway masks distinct failures as "unavailable" including the 32MiB bound; RPC-08 undeclared flat decode bounds (`MAX_TOTAL_IMAGE_BYTES`, `MAX_MESSAGES`, `MAX_PARTS`, decision image count, audio token range) not in the contract or `option_bounds`; RPC-09 `deny_unknown_fields` on 98 typed methods, none on 87 legacy; RPC-10 redaction tokenizes on ASCII space only (`token: X`, `"token":"X"`, tab-separated Bearer escape); RPC-11 External profile endpoints accept any http/https host while docs say LAN providers are unsupported; `RUST_LOG` is ignored and `--debug` enables DEBUG for every crate.
- **Governance / hygiene**: GOV-10 `persist-credentials: false` on 5 of 18 checkouts (token is read-only); GOV-11 dead `.flake8`, `.editorconfig` 88 vs ruff 100, `pre-commit-hooks v4.5.0`; GOV-12 three-line `CODE_OF_CONDUCT.md` with typos and no reporting path, LICENSE year 2025; GOV-13 `Z:\tmp\...\wine-home` hidden only by the `.cache/` ignore, empty `onnx-server/` with stale `__pycache__`, `scripts/README.md` omits `release/`, `tests/`, `consumers/`, `acceptance/`, 2.2MB validators tracked twice byte-identical (regeneration gated); GOV-14 `RELEASING.md:30` names a stale branch, `signtoolOptions.publisherName` dead with `CSC_IDENTITY_AUTO_DISCOVERY=false`. Dependency currency lags on several crates with no stated pin policy: axum 0.7, rusqlite 0.32, sysinfo 0.32, notify 7, uniffi 0.28, rustler 0.34, governor 0.7, zip 2, dirs 5; three `rand` majors and five `windows-sys` versions in the lock.

## Prior audit (2026-09-03) status

| ID | Status | Evidence |
| --- | --- | --- |
| CS-01 secrets in logs | Largely closed | Rust: handlers log only method label/outcome class, `AdmittedRpcRequest` has no Debug, `SecretToken` newtype, `public_reason` on every message; acquisition passes the token only as a header, redirects strip it (`reqwest 0.12.28 redirect.rs:239-251`), `https_only`. Open: ELE-05, RPC-10, ACQ-04. |
| CS-02 RPC trust boundary | Partial | LAN refused incl. `localhost` (`server.rs:66-79`); Host/Origin/duplicate/Sec-Fetch-Site admission outermost (`http_admission.rs`). 98 typed / 87 legacy methods in Rust; 88 of 178 validated in Electron. No caller auth, by documented design. ELE-03, RPC-09. |
| CS-03 persistence atomicity | Partial | `upsert`, `replace_model_id_with_expectation`, publication projections, dependency bindings and intent schema install are transactional. Open: PERS-01, 02, 03, 04, 07; remediation plan Milestone 3 still `Planned`. |
| CS-04 async lifecycle | Largely closed | `RuntimeTasks` with abort handles, panic capture, drain and ordered tail; acquisition task custody; Electron RPC ack → grace → SIGTERM → SIGKILL ladder with ~60 tests; frontend installation lifecycle. Open: TS-01, RT-03, ELE-07, FE-06. |
| CS-05 snapshot provenance | Partial | Scope-bound snapshot, per-model `cached` flag, `role="alert"` degraded notice, HF cache requires `observed_at/freshness`. Open: FE-04, FE-02, ACQ-02. |
| CS-06 release evidence | Partial | `artifact-plan.json` + `check-artifacts.mjs` (16 artifacts), SPDX 2.3 + in-toto, checksums; hash-locked Torch recipe. Open: GOV-02, GOV-08, APM-01/02 (no in-repo pins for Ollama or Torch dependency closure). |
| CS-07 bindings claims | Partial | README hedged, matrix declares zero host tuples. Open: GOV-03. |
| CS-08 build configurations | Open | `hf-client`/`gpu-monitor` features gate nothing; `process-manager` only toggles a builder default; `build_info.rs` reports all three (RT). Frontend library-only ungated (GOV-05). Rust dependency-feature graph is guarded by `check-dependency-features.py`. |
| CS-09 accessibility | Largely closed | `ui/ModalDialog.tsx` and `ui/Popover.tsx` own role/aria-modal/focus trap/Escape/restore; all 9 dialogs and 30 files use them; progress has valuenow and live regions. Open: FE-05, two progress rings with no aria. |
| CS-10 old standards | Closed | No max-lines, no README-coverage script, `DEVELOPMENT.md:27-28`. |
| CS-11 plugin fallback | Partial | `unwrap` removed, bootstrap fails closed. Open: RPC-04. |
| CS-12 launcher-root durability | Closed | Tri-state absent/invalid/unavailable; `wx` 0o600 temp + fsync + rename + parent fsync; torn-write tests (`launcher-root.ts:155-174,253-270,356-449,523-556`). |
| CS-13 `check:errors` gate | Closed | Retired with recorded disposition. |
| CS-14 doc authority | Partial | Stale RPC method doc and SYSTEM_ARCHITECTURE removed; all plans have status lines. Open: GOV-07. |
| CS-15 launcher platform/kill | Closed | Exit 5 on unknown platform; SIGTERM grace → SIGKILL → forced-close deadline; bounded `taskkill`. |

## Owner API rules (stated 2026-11-02)

| Rule | Result | Evidence |
| --- | --- | --- |
| Every rejection carries a human-readable reason | **Fail** | RPC-01 (14 bare-status sites), RPC-02 (generic "Request parameters are invalid."), RPC-07, RT-04, TS-01/TS-05 (OOM/oversize as bare 500), RT-08 (IPC silent EOF). The typed model-operations lane itself passes. |
| Additive changes never break consumers | Pass in Rust, conflict in desktop | `supported_contract_versions` advertised, response types tolerate unknown fields, `unsupported_contract` with reason. Desktop validators are closed (ELE-02); owner decision needed on open responses vs. backend-hash pinning. |
| No flat model-agnostic limits | Pass with exceptions | llama.cpp `text_bounds` derive from served context, `max_tokens` never capped server-side, llama.cpp batch derived from GGUF/context. Exceptions: RT-04 (ONNX), TS-04/TS-05 (Torch), RPC-08 (undeclared decode bounds). |
| Consumers discover models via Pumas; `/v1/served-models` lists profiles and capabilities without duplicate IDs | Pass | Sort + `dedup_by` on (alias, profile), `profile_required` from alias counts (`model_operations.rs:117-169`); `/v1/models` one ID with `profiles`. |
| Secrets never in logs/errors | Pass in Rust | 0 log sites emit request params; 0 Debug derives on token-holding types. Electron residual ELE-05. |

## Strengths worth keeping

- **Acquisition pipeline**: RFC 9110-tight range/conditional handling, identity bound before and after hashing, no overwrite of unverified files, signed-URL identity allowlist, `https_only`, token stripped on cross-host redirect, warm checkpoints bounded, S3 reader with SDK-owned signing and bounded XML/pagination.
- **llama.cpp installer, uv bootstrap and audio cohort**: publisher digest required, verify-before-extract, symlink/special-file rejection, stage → fsync → rename publish with receipts, `env_clear` with transport allowlist, fixed .deb recipe with exact lengths.
- **Linux audio confinement**: Landlock ABI≥6 + seccomp, every unsafe block documented, fail-closed arch gating, opt-in kernel-enforcement test in CI.
- **Non-reaping process custody**: `waitid(WNOWAIT)` pins PID/PGID until `/proc` proves drain; Windows children created suspended and assigned to a kill-on-close Job before their first instruction.
- **Capability-based path authority**: `O_NOFOLLOW|O_DIRECTORY` walks, `RENAME_NOREPLACE`/`RENAME_EXCL`/`ReplaceIfExists` non-replacing renames, identity rechecked around every unlink; no `remove_dir_all` on derived paths.
- **Durable JSON publisher** for downloads.json with explicit `Durable / PublishedDurabilityUnknown / VisibilityUnknown` outcomes and duplicate-key refusal; download-store schema refusal with named backups.
- **RPC transport**: handler tasks outlive lost callers, panics contained per task and reported in the shutdown receipt; generation lifetime implemented exactly; typed SSE decoder with 256KiB/32MiB bounds; provider results re-validated, never forwarded; no client headers reach providers.
- **Electron**: Rust-exported AJV-standalone contract pipeline with CI drift check; launcher-root tri-state recovery with atomic publish; `PythonBridge` shutdown receipts with ~60 lifecycle tests; S3 RPC path with non-pooled agent, 64KiB cap and static error text; sandboxed, context-isolated renderer with `script-src 'self'` enforced by test.
- **Frontend**: shared `ModalDialog`/`Popover` primitives with behavioral tests; sequence/generation guards in every async hook checked; optimistic download commands use transitional states with explicit rollback matrices; 0 `any`, 0 suppressions; conformance suite loads the real built preload against producer fixtures.
- **torch-server**: image generation lifecycle with step-checkpoint cancellation and lease custody; private owned channel with bounded frames and `-I -B -S` bootstrap refusing ambient imports; `trust_remote_code=False` + `local_files_only=True` at all 13 `from_pretrained` sites; hash-pinned uv lock with official wheel URLs.
- **CI**: `check-ci-release-gating.test.mjs` asserts workflow invariants (tag gating, `contents: read`, no publish verbs); ORT provisioning hashes a private snapshot before any member read; `--locked`/`--frozen-lockfile` everywhere; exact toolchain pins; `artifact-plan.json` as single source of truth with filename pinning tests.

## Recommended order

1. **ELE-01**: `setWindowOpenHandler` → deny + `shell.openExternal(validateExternalUrl(url))`; `will-navigate`/`will-redirect` deny non-app URLs; `setPermissionRequestHandler` deny; `validateSender` on every `ipcMain` handler; replace the four `target="_blank"` anchors with a shared `ExternalLink` over `api.open_url`; add a test asserting the handlers are installed. Ship before 0.8.0.
2. **RT-01 / RT-02 / TS-07**: delete the substring fallback; stop only children with proven custody; store boot-id + start-time (or canonical `/proc/<pid>/exe` under the version dir) in PID files and refuse on mismatch; return "skipped: not owned" with a reason.
3. **APM-01**: route Ollama through `resolve_release_asset` + `acquire_http` like llama.cpp; run `validate_native_output`; drop the size-only cache check. Then APM-02 (`--require-hashes --no-deps` two-phase) and APM-03 (reject unsafe tags at the `VersionManager` boundary; gate or remove `DependencyManager`).
4. **PERS-01 / PERS-02 / PERS-04**: wrap delete and package-facts mutation+event in one transaction like `upsert`; `BEGIN IMMEDIATE` around each governance rebuild and refuse startup when a `*_legacy` table exists; then PERS-03 (schema marker row, refuse unknown) and PERS-08 (route legacy writers through `AtomicJsonTarget`).
5. **TS-01 / TS-02 / TS-03**: `asyncio.to_thread` + device lease + `StoppingCriteria` cancellation for text; launcher sets `PUMAS_TORCH_MODEL_ROOTS` and a per-launch `PUMAS_TORCH_API_TOKEN`; validate `pipeline_path`/`vae_path`/`model_type`; map OOM to 507 with reason and `empty_cache`.
6. **GOV-02 / GOV-01 / GOV-04**: add `cargo audit` or `cargo deny`, `pnpm audit`, `pip-audit -r torch-server/runtime/requirements.lock` (advisory on PR, blocking on tag, feeding `current-vulnerability-results`); commit `.github/dependabot.yml` for cargo, npm, pip and github-actions; move overrides to `pnpm.overrides` or delete them; SHA-pin actions.
7. **RPC-01 / RPC-02 / RPC-03**: one helper producing `{"error":{"code","message"}}` for the 14 bare-status sites; route serde messages through `public_reason`; `.no_proxy()` on every provider client.
8. **ELE-02 decision**, then ELE-03 (migrate the 90 shape-only methods to Rust-exported decoders), FE-02 (decode pull paths), FE-03 (surface the nine silent failures), FE-04 (snapshot `writtenAt`).
9. **RT-04 / TS-04 / TS-05 / RPC-08**: derive per-model token and pixel budgets from config/device and advertise them in capabilities; document the remaining decode-safety bounds.
10. **CS-08, GOV-03, GOV-07**: make `hf-client`/`gpu-monitor` gate code or remove the features; execute or retract the bindings removal list and correct `native-bindings.md`; fold terminal plans into ADRs, regenerate the docs index, rename `docs/breif`.

## Appendix A: CI gate matrix (as read from `build.yml`)

| Gate | PR | push main | tag / candidate dispatch |
| --- | --- | --- | --- |
| lint-workflows (actionlint, dep-ownership, release-versions, attribution ×2, release node tests, S3 provenance) | Y | Y | Y |
| `scripts/rust/check.sh` | Y | Y | Y |
| headless no-default core/RPC + S3 + audio confinement | Y | Y | Y (+ no-inference release build/smoke) |
| Frontend lint / check:types / test:run | Y | Y | Y |
| Frontend default build | – | – | Y |
| Frontend library-only build | – | – | Y (overwritten, unchecked) |
| Electron lint / validate / test + desktop-contract checks | Y | Y | Y |
| `test:launcher` | Y | Y | Y |
| torch-quality: ruff, unit, ~45 native cargo tests ×3 OS | Y | Y | Y |
| review-inference-linux (full inference candidate + synthetic ONNX) | Y | – | – |
| review-audio-dependencies | Y | – | – |
| torch-native-e2e ×3 OS; headless-rpc ×3; headless-inference ×3; build-rust; windows-release; build-electron ±no-inference; verify-launcher (Xvfb); release-candidate (inventory + SPDX + provenance + checksums) | – | – | Y |
| Vulnerability/license scan; bindings generation/host test; signing | never | never | never |

Push-to-main runs strictly fewer gates than a PR.

## Appendix B: runtime supply chain

| Runtime | Source | Pin | Verified before extract |
| --- | --- | --- | --- |
| llama.cpp | github.com API + asset | GitHub publisher `digest`, required | Yes (`https_only`, acquisition workspace) |
| Ollama (`installer.rs`) | github.com | none | **No** (size only) |
| Ollama (`ollama.rs`, legacy, unwired) | github.com | none | **No**, zip-slip |
| uv bootstrap | releases.astral.sh | sha256 per target in `managed_python.rs` | Yes (40MB cap, re-verified on reuse) |
| CPython (python-build-standalone) | via uv | host/path prefix; hashes delegated to uv | Partial |
| Torch qualified recipe 2.9.1 Linux | download-r2.pytorch.org | `torch.rs` + `runtime/requirements.lock` | Yes (`--require-hashes --no-deps`) |
| Torch direct install (other builds) | pytorch + pythonhosted | torch wheel `#sha256` when found; deps unpinned | Partial (post-hoc report, size-only tree check) |
| Audio native cohort (14 .deb) | deb.debian.org | `recipe.json` size + sha256 | Yes |
| ONNX Runtime | in-process, provisioned by release scripts | `onnx-runtime-pins.json` URL + size + sha256 | Yes |

## Appendix C: housekeeping observed, not changed

- The temp volume (`/`) hit 0 bytes free while the reviews started; `/tmp/claude-1000` holds 5.9GB of session data. Nothing was deleted.
- Repository root contains ignored working state: `tmp/` (229 entries), `Z:\tmp\...\wine-home`, `.download-identity-repair.*`, `.download-store-cutover.*`, `.coverage`, `onnx-server/` with only `__pycache__`.
- `docs/breif/future.md` is untracked and unindexed.
