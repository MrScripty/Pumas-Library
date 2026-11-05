# Plan: 2026-11-02 Audit Remediation

**Plan status:** `Active`

**Input:** [comprehensive audit 2026-11-02](../../audits/comprehensive-audit-2026-11-02.md),
written against `b1196838`. Every finding was re-verified against
`7b40d50f` (2026-11-05) by reading current source; High findings were
spot-checked a second time. The verified tree was then compared with
`origin/main` at `b091ac45` (PRs #87–#90). #87 merged that same Torch work as
`729c7868`/`0515650b`. #88–#90 bump action SHAs, flag pre-release tags in
release listings, and adjust a CI test. None of them changes a finding's
status, so `b091ac45` is the baseline for this plan. Results are in
[Appendix: verification](#appendix-verification-at-b091ac45).

**Relationship to other plans:** the
[current-standards remediation program](../current-standards-remediation-2026-09-03/plan.md)
asked for its open issues to be re-checked against v0.8.0. This plan covers
the audit's re-check of CS-01..CS-15; it does not close that program's own
issue list.

**Current phase:** planning. No fixes have started.

**Next slice:** [S1 — Electron window and navigation boundary](#s1--electron-window-and-navigation-boundary).

## Verification summary

| Severity | Audit count | Still valid | Partial | Fixed | Invalid |
| --- | --- | --- | --- | --- | --- |
| High | 8 | 7 | 1 (GOV-02) | 0 | 0 |
| Medium | 39 | ~32 | 4 (PERS-02¹, GOV-03, GOV-07, GOV-08) | 2 (GOV-01, GOV-04) | 0 |
| Low | 48 | ~40 | ~6 | 1 (GOV-06) + APM-04 zip-slip | 0 |

¹ PERS-02 was High; the rebuild is now atomic (savepoint, `53f30947`), so the
remainder is Medium.

Audit corrections found during verification:

- **APM-02 is worse than written.** `729c7868` (PR #87) removed the
  only `--require-hashes --no-deps` Torch path. Every Torch install now uses
  the direct path, and the torch wheel `#sha256` that `resolve_runtime.py`
  accepts is never passed by the installer (`installer/torch.rs:2063-2077`).
  The installer never passed it, before or after #87.
- **ACQ-07 is on a production path.** `start_download` (Weak revision) is used
  by startup interrupted-download recovery (`api/builder.rs:206`) and
  `PumasApi::recover_download`. It should be Medium.
- **RPC-01 has 17 bare-status sites**, not 14, and the router has no fallback,
  so unknown routes and wrong methods return empty 404/405.
- **RPC-10 has a real leak path.** `public_reason_within` redacts before it
  replaces control characters, so `Bearer\nX` escapes redaction. It is
  borderline Medium.
- **RPC-03 extends beyond pumas-rpc.** `pumas-core/.../router_observer.rs:137`
  and the app-manager `torch_client.rs` / `ollama_client.rs` clients also lack
  `.no_proxy()`.
- **ELE-09 is more serious than written.** Packaged builds accept `--dev`
  (which loads `http://127.0.0.1:3000` with the full preload) and `--debug`
  (which opens DevTools).
- **TS-06:** three text loaders also lack `local_files_only=True`. This
  contradicts the audit's "all 13 sites" strength claim.
- **APM-05:** there is no SIGKILL after 500 ms (Unix sends SIGTERM only). It
  is dead public API.
- **RPC-04** should be Low. **APM-04**'s remainder (unwired type, tar
  symlinks) is Low.

New since the audit, all from PR #87 (`729c7868`/`0515650b`):

- **N-1.** The image adapter packages (`diffusers`, `transformers`,
  `accelerate`, `peft`, `sentencepiece`, `protobuf`) are no longer pinned
  (`resolve_runtime.py:321-332`). Nothing exercises the adapters at install;
  the only check is the probe's import check.
- **N-2.** The "verified" match compares versions of direct packages only.
  `is_none_or` lets an absent package pass (`torch_verified.rs:125-149`), and
  `verified-runtimes.json` has no hashes. Legacy installs are marked
  `qualified: true` from `recipe_id` alone (`torch_preview.rs:733-740`).
- **N-3.** Stale references to the removed preset: ADR 0002:58, the
  `bundledPresetAvailable` fixture
  (`electron/tests/preload-rpc-contract.test.mjs:110`), and the Torch
  cross-platform plan.
- **N-4.** `delete_model_package_facts_cache` emits no update event at all.
  This is part of PERS-04.
- **N-5.** The additive rule conflicts on requests. Typed params use
  `deny_unknown_fields`, so a newer consumer sending a new optional field to
  an older backend gets the reasonless "Request parameters are invalid."
  (RPC-09 + RPC-02).

## Owner decisions needed

These block or shape specific slices. Each has a recommendation.

| ID | Question | Recommendation | Blocks |
| --- | --- | --- | --- |
| D1 | ELE-02: closed desktop response validators vs. "additive is non-breaking" | Keep the validators closed. Make the packaged desktop ignore `PUMAS_RPC_BINARY` (ELE-08), so desktop and backend always ship as one unit. The additive rule then applies to external consumers, where it already passes. | S9 |
| D2 | N-5 / RPC-09: unknown request fields | Keep the strict refusal, but make the reason name the field (RPC-02), and point consumers to `supported_contract_versions` for discovery. | S8 |
| D3 | RPC-11: should External runtime profiles accept non-loopback hosts? | Decide first, then either restrict to loopback or document remote providers (require https or warn). This also decides whether RPC-03 uses `.no_proxy()` or a loopback `NoProxy`. | S8 |
| D4 | Torch integrity under "evidence, not pins" (APM-02, N-1, N-2) | At install: resolve once, write a per-install hash lock, then install with `--require-hashes --no-deps`. Pass the discovered torch wheel sha256. Offer the verified record's versions as an opt-in `--constraint`. Narrow "verified" to "versions match a recorded combination" in the contract and docs. Trust-on-first-use, but enforced and recorded. | S4 |
| D5 | GOV-03: native bindings | Either carry out the decided removal (`pumas-uniffi`, `pumas-rustler`, the `uniffi` feature in core, the scripts) or retract it. | S12 |
| D6 | GOV-02: how dependency scans gate | Advisory on PRs and blocking on tag/candidate. Feed `current-vulnerability-results` into the release candidate. | S7 |

## Slices

Each slice is one PR, ordered by exposure. S1–S3 are the security
boundaries. S5 has to come before any new schema work. Slices with no
dependency between them may run in parallel.

### S1 — Electron window and navigation boundary

Findings: ELE-01/FE-01 (High), ELE-09.

- `electron/src/main.ts` `createWindow`:
  - `setWindowOpenHandler` → deny, plus `shell.openExternal` after
    `validateExternalUrl`.
  - `will-navigate` / `will-redirect` → deny anything but the app origin.
  - `session.setPermissionRequestHandler` and `setPermissionCheckHandler` →
    deny.
  - An `app.on('web-contents-created')` guard so any future window gets the
    same handlers.
- An `assertMainFrame(event)` sender check on every `ipcMain.handle`,
  starting with `api:call` (only 3 of about 20 check today).
- Honour `--dev` / `--debug` only when `!app.isPackaged`. Set an
  application menu without DevTools in packaged builds.
- Frontend: add a shared `ExternalLink` over `api.open_url`, and use it for
  the four `target="_blank"` anchors (`ModelMetadataGrid.tsx:130`,
  `ModelNotesMarkdownPreview.tsx:42`, `ImportMetadataDetailsParts.tsx:120`,
  `ImportLookupCard.tsx:50`) and the five `window.open` calls. Remove the
  `window.open` fallback in `useInstallDialogLinks.ts:40-41`.
- **Acceptance:**
  - An Electron test asserts the handlers are installed, and that
    window-open and navigation to a remote URL are denied.
  - A frontend lint rule or test forbids `target="_blank"` and `window.open`.
  - `npm run -w electron lint test validate`, and frontend lint, types and
    tests, pass.
  - A manual click on a metadata link in a representative Electron runtime
    opens the system browser.

### S2 — Process stop by proven custody

Findings: RT-01 (High), RT-02 (High), TS-07 (stop half), RT-11.

- Delete the pattern-based stop: `stop_matching_processes`,
  `ProcessLauncher::stop_processes_by_pattern`, and the
  `find_processes_by_cmdline("ollama")` detection fallback
  (`process/manager.rs:245,297,427,501-519`; `launcher.rs:414`).
- Write a PID record of {pid, boot id, start time, canonical exe}. Check it
  before `terminate_process_tree`, and never `killpg` without matching custody
  (`launcher.rs:523-527`; `manager.rs:225,408,488-499`).
- Return "skipped: not owned by Pumas" with a reason, instead of killing or
  failing the whole stop on EPERM.
- Leave runtime-profile processes to `process_owner`, so the legacy stop never
  touches them.
- Bound the observer loop retries (`manager.rs:545`).
- **Acceptance:** tests where an unrelated `ollama serve` / `serve.py` process
  survives the stop, a stale PID file whose PID was reused is refused, and an
  owned process still stops. Also `./scripts/rust/check.sh`.

### S3 — Ollama runtime integrity

Findings: APM-01 (High), APM-04, APM-05, APM-06, APM-09.

- Route the Ollama install through the llama.cpp acquisition path, requiring
  the GitHub `asset.digest` (`network/github.rs:60,1032`).
- Drop the size-only cache trust (`installer.rs:1817-1842`). Add
  `https_only(true)`.
- Call `validate_native_output` before `finalize_ollama_binary`, and use
  `symlink_metadata` there.
- Mask `mode & 0o777` (`installer.rs:3521`). Add cumulative byte and entry
  limits derived from the asset size to the zip and tar extractors.
- Delete `OllamaVersionManager` (`ollama.rs`, export at `mod.rs:70`) and
  `ProcessManagerFactory` (`process/factory.rs`, export at `lib.rs:23`).
- `OllamaClient`:
  - `new` returns `Result`.
  - Error bodies are bounded.
  - A non-loopback `base_url` is rejected (subject to D3).
- **Acceptance:** tests that the install refuses a wrong digest, a symlinked
  binary, a setuid entry and an oversized archive, and that the cached archive
  is re-verified by hash. Also `cargo test -p pumas-app-manager`.

### S4 — Torch install integrity (after D4)

Findings: APM-02 (raised), N-1, N-2, N-3, APM-03, APM-07, APM-08.

- `resolve_runtime.py` / `installer/torch.rs` `stage_selected_torch_attempt`:
  - Phase 1 is a dry-run report that becomes a hash lock, validated by
    `validate_direct_torch_report`.
  - Phase 2 is `pip install --require-hashes --no-deps --only-binary=:all:`.
  - Pass the torch wheel URL and sha256 discovered in
    `discover_release_options`.
  - `validate_staged_files` compares sha256, not size.
- Fix the verified match: an absent recorded package fails, the match is
  labelled as a version match, and `qualified: true` is no longer derived from
  `recipe_id` alone.
- APM-03:
  - Validate the tag in `VersionManager::version_path` (one normal component).
  - Check for `requirements.txt` before `create_venv`.
  - Use the managed Python, or remove the `install_version_dependencies` RPC.
- APM-07: `env_clear()` plus an allowlist in `process_owner.rs:921` and
  `launcher.rs:458`, reusing the `managed_python.rs` helper.
- Clean up the N-3 stale references. Bring SECURITY.md up to date: it should
  say install-time integrity is trust-on-first-use, and name the pip-audit
  inputs.
- **Acceptance:**
  - Installer tests: a tampered staged file and a hash mismatch are refused,
    and a `../x` tag is refused.
  - Torch unit tests and `cargo test -p pumas-app-manager` pass.
  - One real install on Linux records and enforces the lock.

### S5 — SQLite atomicity

Findings: PERS-01 (High), PERS-02 (remainder), PERS-04 + N-4, PERS-05,
PERS-09, PERS-10, PERS-12.

- `ModelIndex::delete`: DELETE and the event append in one transaction, then
  publish (as `upsert` does at `model_index.rs:424-435`).
- `migrate_model_type_rule_constraints`: detect `*_legacy`. If the live table
  is missing, finish the copy inside `all_or_nothing`; otherwise refuse with a
  reason that names the table.
- Package-facts cache:
  - Wrap each mutation and its event in one transaction.
  - Add the missing delete event.
- FTS `ensure_setup` / `rebuild`: one transaction each.
- Make `clear()` transactional, or `#[cfg(test)]`.
- `row_to_record` returns a conversion error on corrupt JSON. `search_fts5`
  propagates errors; only an invalid MATCH syntax keeps a typed refusal.
- **Acceptance:** fault-injection tests (fail between statements) for delete,
  package facts, FTS and the legacy-table recovery, and a corrupt-JSON row
  that surfaces as an error. Also `cargo test -p pumas-core`.

**Follow-on (separate PR):** PERS-03 schema identity
(`PRAGMA user_version` or a marker row, refusing unknown or newer schemas),
PERS-08 (legacy writers go through `AtomicJsonTarget`), PERS-06 (lock around
load-mutate-save), PERS-07 (review edits re-derived from the overlay).
PERS-03 is a persistence migration, so it needs an upgrade test from a 0.8.0
library.

### S6 — Torch sidecar runtime

Findings: TS-01 (High), TS-02, TS-03, TS-06, TS-09, TS-11, TS-08.

- `openai_api.py` `_generate`:
  - Run it in `asyncio.to_thread` under the device lease the image path uses.
  - Add a `StoppingCriteria` tied to a cancel event.
  - Map OOM to 507 with a reason, plus `empty_cache`.
- In `launch_specs.rs` (TorchServe arm), set `PUMAS_TORCH_MODEL_ROOTS` to the
  library roots and a per-launch random `PUMAS_TORCH_API_TOKEN`; the Rust
  Torch client sends the token.
- `control_api.py`:
  - Validate `pipeline_path` / `vae_path`, and use an enum for `model_type`.
  - Add `extra="forbid"`.
  - Map known errors to stable codes.
- Add `use_safetensors=True, local_files_only=True` in the three text
  loaders. Guard `cuda.synchronize`. Allowlist the `model_index.json` class
  pairs.
- **Acceptance:**
  - `/health` answers during a long generation.
  - An unauthenticated request is refused.
  - A path outside the roots is refused.
  - Ruff and `unittest` pass, plus the torch-quality native tests.

### S7 — Release dependency evidence

Findings: GOV-02 (remainder), TS-10.

- A `dependency-audit` job covering three tools:
  - `cargo deny check advisories licenses`
  - `pnpm audit --prod --json`
  - `pip-audit` on `requirements-dev.lock` and a resolved runtime
    `requirements.txt`
- Gate it per D6. Write the output to the candidate as
  `current-vulnerability-results` (`artifact-plan.json:517`).
- Add the `cargo`, `npm` and `pip` ecosystems to `.github/dependabot.yml`.
- Refresh the exceptions table in SECURITY.md against the actual 0.8.x
  closure.
- **Acceptance:** `check-ci-release-gating.test.mjs` asserts the job and its
  gating. A candidate dispatch produces the artifact. `check-artifacts.mjs`
  passes.

### S8 — RPC rejections, reasons and clients (after D2, D3)

Findings: RPC-01, RPC-02, RPC-03, RPC-04, RPC-05, RPC-06, RPC-07, RPC-10.

- RPC-01: add one `{"error":{"code","message"}}` helper and use it at the 17
  sites. `request_fence` returns a reason enum. Add router `.fallback` and
  `.method_not_allowed_fallback`.
- RPC-02: pass serde errors through `public_reason` at the 34
  `invalid_params()` sites. Surface the S3 `validate()` detail.
- RPC-03: one loopback client builder with `.no_proxy()` for pumas-rpc,
  `router_observer.rs`, and the app-manager torch and ollama clients.
- RPC-05: hyper `TokioTimer` plus header-read timeout. A connection
  semaphore.
- RPC-06: `redirect(Policy::none())`. Bounded provider body reads.
- RPC-07: specific reasons at the remaining roughly 9 `unavailable()` sites.
- RPC-10: normalise before redacting, split on any whitespace, and handle
  `key:`, JSON pairs and `Basic`.
- RPC-04: replace the `/tmp` fallback with an in-memory empty `PluginLoader`.
- Logging: `EnvFilter` honouring `RUST_LOG`.
- **Acceptance:** each former bare-status site returns a JSON reason with its
  original status. Redaction tests for `token: X`, `"token":"X"` and
  `Bearer\nX`. A proxy-env test proves loopback bypass. Also
  `cargo test -p pumas-rpc`.

### S9 — Desktop contract decoding and surfaced failures (after D1)

Findings: ELE-02 (per D1), ELE-03, ELE-04, ELE-05, ELE-07, ELE-08, ELE-11,
FE-02, FE-03, FE-04, FE-06.

- Rust-exported request decoders for the mutating methods first:
  `set_hf_token`, `import_model`, `import_batch`, `torch_configure`,
  `delete_model_with_cascade`. Then the rest of the 90 shape-only methods.
- Decode the six pull paths in `preload.ts` (FE-02).
- Error channels for the four SSE streams.
- An `error` state in the nine silent-default hooks.
- Snapshot `writtenAt` and revision, with display-only marking on cold start
  only.
- Install-poll backoff.
- Log sidecar output at debug, and drop the raw body from
  `Invalid JSON response`.
- Reset the download subscription counter on window close.
- Ignore `PUMAS_RPC_BINARY` when packaged. The sidecar binds port 0 and
  reports its port, and the health check echoes a nonce.
- Pass a sanitised launcher-root reason to the renderer.
- **Acceptance:** desktop-contract check. Electron and frontend tests. A
  representative Electron run showing a forced backend failure surfaced in
  the UI.

### S10 — Limits derived from the model

Findings: RT-03, RT-04, TS-04, TS-05, RPC-08.

- ONNX:
  - `load` / `embed` run under `spawn_blocking`, with a lock per session.
  - The token limit comes from `max_position_embeddings`.
  - Over-length input is refused with a reason.
- Torch:
  - A pixel budget from `mem_get_info` and adapter limits, checked at
    admission.
  - `input_len + max_tokens` is checked against the model's positions.
- Make `RequestLimit` name the limit and its value. Advertise input bounds as
  an additive capability field.
- Fix the audio `MaxOutputTokens` advertisement and the stale 1–2048 line in
  `image-to-text-v0.8.md`.
- **Acceptance:** an over-limit request is refused with a named reason, and
  capabilities advertise the derived values.

### S11 — Lifecycle and acquisition hardening

Findings: RT-06, RT-07, RT-08, RT-09, RT-10, ELE-06, ACQ-01, ACQ-04, ACQ-05,
ACQ-06, ACQ-07.

- Process lifecycle:
  - `PR_SET_PDEATHSIG` in `ManagedChild::spawn`, plus stale-PID reclaim by
    identity.
  - SIGTERM before SIGKILL in `terminate_and_drain`.
  - The IPC server sends a capacity error frame and has an idle timeout.
  - `registry.db` is created 0600.
  - ORT loads the preflighted handle.
  - pumas-rpc exits on stdin EOF.
- Acquisition:
  - ACQ-07: recovery resolves a commit before resuming.
  - ACQ-01: `validate_hf_repo_id` on the read paths, and encode
    `pipeline_tag`.
  - ACQ-04: write the token atomically with mode 0600.
  - ACQ-05: refuse unbounded retry.
  - ACQ-06: a corrupt tree cache counts as a miss.
- **Acceptance:** targeted tests per item. Also `./scripts/rust/check.sh`.

### S12 — Hygiene, docs and dead code

Findings:
- Governance and CI: GOV-03 (per D5), GOV-05, GOV-07, GOV-08, GOV-09, GOV-10,
  GOV-11, GOV-12, GOV-13, GOV-14, CS-08, Appendix A (push-to-main gates).
- Frontend and Electron: FE-05, FE-07, FE-08, FE-09, ELE-10.
- Rust: RT-05, APM-08, PERS-11, PERS-13 (verify RPC redaction only), ACQ-02,
  ACQ-03, ACQ-08.
- Dependency currency policy.

These are independent small PRs; group them as convenient.

## Blockers

- D1–D6 need owner answers before S9, S8, S4, S12 and S7 respectively.
- The audit file and `docs/breif/future.md` are untracked. Commit the audit
  (as point-in-time evidence, indexed in `docs/README.md`) or keep it local.

## Appendix: verification at b091ac45

Status key: **V** still valid · **P** partial · **F** fixed · **↑ / ↓** severity
should rise or fall. Line references are current; paths are under
`rust/crates/` unless noted.

| ID | Status | Current evidence / remainder |
| --- | --- | --- |
| ELE-01/FE-01 | V | No window/navigation/permission handlers in `electron/src`; `main.ts:520` `api:call` ignores sender; anchors and `window.open` unchanged |
| ELE-02 | V (D1) | `pumas-rpc/src/contract/export.rs:1821-1823` |
| ELE-03 | V | `rpc-method-registry.ts:71,100,134,162,167` have no schemas |
| ELE-04 | V | `python-bridge.ts:853-895` |
| ELE-05 | V | `python-bridge.ts:550-563,476,1063` |
| ELE-06 | V | No parent-death check in pumas-rpc; `python-bridge.ts:680` |
| ELE-07 | V | `main.ts:497-499` misses `modelDownloadRendererSubscriptions` |
| ELE-08 | V | `backend-path.ts:19-22`; `python-bridge.ts:411-460,657-664` |
| ELE-09 | V ↑ | `main.ts:234-239,440,472`; no `setApplicationMenu` |
| ELE-10 | V | `electron/tsconfig.json:13-15`; `package.json:147-157` |
| ELE-11 | V | `launcher-root-recovery.ts:57-66` |
| FE-02 | V | `preload.ts:296,426,442,546,690,741` |
| FE-03 | V | `useModels.ts:90-118`; `useModelDownloads.ts:109`; `useActiveModelDownload.ts:103`; `useDiskSpace.ts:15-33` |
| FE-04 | V | `modelLibrarySnapshot.ts`; `useModels.ts:61`; `LocalModelRowState.ts:168,173` |
| FE-05 | V | `ui/Tooltip.tsx:26-41` |
| FE-06 | V | `useInstallationManager.ts:273-287` (visible, not silent) |
| FE-07 | V | `build.yml:458-460` tags only; no LibraryOnly tests |
| FE-08 | V | Seven dead modules; `docs/DEVELOPMENT.md:162`, `frontend/README.md:57` cite `config/theme.ts` |
| FE-09 | V | `frontend/index.html:8` |
| RT-01 | V | `pumas-core/src/process/manager.rs:245,297,427,501-519` |
| RT-02 | V | `process/launcher.rs:523-527`; `platform/process.rs:52-64,418-447` |
| RT-03 | V | `onnx_runtime/real_backend.rs:89-98,132-147` |
| RT-04 | V | `onnx_runtime/mod.rs:38-40,360-399`; `tokenizer.rs:8,77-94` |
| RT-05 | V | `platform/managed_child.rs:598,640-851` |
| RT-06 | V | `managed_child.rs:151-185`; `process_owner.rs:897-908` |
| RT-07 | V | `platform/linux_group.rs:94`; `managed_child.rs:363,388` |
| RT-08 | V | `ipc/server.rs:251-259` |
| RT-09 | V (suspected) | `registry/library_registry.rs:160-171` |
| RT-10 | V | `onnx_runtime/native_library.rs:66-79` |
| RT-11 | V | `process/manager.rs:545-553` |
| APM-01 | V | `version_manager/installer.rs:1817-1842,2818-2854,3051,3284,3313` |
| APM-02 | V ↑ | `torch-server/resolve_runtime.py:1324-1346`; `installer/torch.rs:1688-1694,2063-2077`; locked path removed in `729c7868` |
| APM-03 | V | `version_manager/mod.rs:598-600`; `dependencies.rs:236-243,275,318-326` |
| APM-04 | P ↓ | Zip-slip fixed (`a5d4ff17`); type still exported, tar symlinks |
| APM-05 | V (detail wrong) | `process/factory.rs:143-157,173-200`; no SIGKILL; dead API |
| APM-06 | V | `installer.rs:3517-3524,3033-3048,3539-3549` |
| APM-07 | V | `process_owner.rs:921-927`; `launcher.rs:458-470` |
| APM-08 | V | `installer/torch.rs:223` |
| APM-09 | V | `ollama_client.rs:140,144-151,172,260,269,351` |
| CS-08 | P | `hf-client`/`gpu-monitor` gate nothing; `process-manager` caps a runtime flag |
| PERS-01 | V | `index/model_index.rs:813-835` |
| PERS-02 | P ↓ | Savepoint added (`53f30947`, `governance.rs:253-272`); leftover `*_legacy` undetected |
| PERS-03 | V | `governance.rs:191-247`; `fts5.rs:58-85` |
| PERS-04 | V | `package_facts_cache.rs:60-125,237-266`; delete emits no event (N-4) |
| PERS-05 | V | `model_index.rs:1051-1055` |
| PERS-06 | V | `metadata/manager.rs:287-351` |
| PERS-07 | V | `library.rs:2359-2367,2408-2416` |
| PERS-08 | V | `metadata/atomic.rs:977-1085` |
| PERS-09 | V | `model_index.rs:951-1042` |
| PERS-10 | V | `fts5.rs:87-101,236-250` |
| PERS-11 | V (test-only) | `registry/library_registry.rs:910,957` |
| PERS-12 | V (test-only) | `model_index.rs:1193+` |
| PERS-13 | V | Core embeds paths; confirm RPC redaction |
| ACQ-01 | V (overstated) | `get_model_info*` already validate; `hf/metadata.rs:107-120,261-285`; `bundles.rs:48-60`; `search.rs:318` do not |
| ACQ-02 | V | `model_library/hf_cache.rs:420-475`; stale path is `NetworkManager::execute` (test-only caller) |
| ACQ-03 | V | `network/download.rs:67`, no callers |
| ACQ-04 | V | `hf/auth.rs:84-96` |
| ACQ-05 | V | `config.rs:100-118` |
| ACQ-06 | V | `hf/metadata.rs:270` |
| ACQ-07 | V ↑ | `hf/download.rs:3786-3798`; used by `api/builder.rs:206`, `api/hf.rs:55-81` |
| ACQ-08 | V | `acquisition/service.rs:2502-2517` |
| RPC-01 | V | 17 sites: `discovery.rs:134-147,197`; `owner_retention.rs:56-79`; `http_transport.rs:151,153`; `server.rs:724`; no router fallback |
| RPC-02 | V | `contract.rs:130-136` (34 call sites); `s3_imports.rs` |
| RPC-03 | V (wider) | `server.rs:731,738`; `openai_gateway.rs:54`; `decision.rs:43`; `vision.rs:53`; `plugins.rs:54`; plus core and app-manager clients |
| RPC-04 | V ↓ | `startup.rs:144-153` |
| RPC-05 | V | `http_transport.rs:161,268,294` |
| RPC-06 | V | `server.rs:731-738`; `provider_clients.rs:57-219` |
| RPC-07 | P | Vision guard fixed (`0908c52f`); about 9 bare `unavailable()` sites remain |
| RPC-08 | V (partly wrong) | Image limits documented but not advertised; audio `MaxOutputTokens` advertisement contradicts code |
| RPC-09 | V (D2) | ~109 typed arms, ~74 legacy |
| RPC-10 | V ↑ | `public_text.rs:17-80`; redact-before-normalise |
| RPC-11 | V (D3) | `pumas-core/src/models/runtime_profile.rs:45-59`; `main.rs:175-186` ignores `RUST_LOG` |
| TS-01 | V | `torch-server/openai_api.py:140-213` |
| TS-02 | V | `validation.py:16,46-47,146-151`; `control_api.py:43-45` |
| TS-03 | V | `launch_specs.rs:181-185`; `serve.py:66-67` |
| TS-04 | V | `image_api.py:40-41,114` |
| TS-05 | V | `openai_api.py:19-22,43,194-213` |
| TS-06 | V (wider) | `loaders/safetensors_loader.py:28`, `sherry_loader.py:35`, `dllm_loader.py:35` |
| TS-07 | V | No parent watchdog in `serve.py`; substring stop (RT-01) |
| TS-08 | V | `control_api.py:32,148-166,239` |
| TS-09 | V | `control_api.py:63,67`; `diffusion.py:58` |
| TS-10 | V | No runtime deps / `tests_native` / pip-audit in CI; dev install now hash-locked |
| TS-11 | V | `diffusion.py:93-108` |
| GOV-01 | F | `7fa129f1`, overrides in `pnpm-workspace.yaml` |
| GOV-02 | P | Dependabot covers only actions; no scan job; `artifact-plan.json:517` has no producer |
| GOV-03 | P | `native-bindings.md` corrected; Go claim, `uniffi` in core, unpinned `cargo install` remain (`set -e` already present: audit error) |
| GOV-04 | F | `d4d4e0ff`, all actions SHA-pinned |
| GOV-05 | V | `build.yml:458-463` |
| GOV-06 | F | `docs/DEVELOPMENT.md:208-228` |
| GOV-07 | P | Index fixed; `docs/breif` name, untracked files, missing `capability-descriptor.schema.json` |
| GOV-08 | P | CI pip hash-locked; `python-version: '3.14'` at `build.yml:531,576`; `requirements.txt` `>=` only |
| GOV-09 | P | Command logs gone; 0.7.0 and 0.8.0-rc.1 attribution sets (~20MB) remain |
| GOV-10 | V | 6 of 18 checkouts |
| GOV-11 | V | `.flake8`, `.editorconfig:36`, `.pre-commit-config.yaml:18` |
| GOV-12 | V | CODE_OF_CONDUCT.md, LICENSE year |
| GOV-13 | P | scripts README fixed; validators tracked twice |
| GOV-14 | P | Branch fixed; `electron/package.json:133` `publisherName` |
| Dep currency | P | zip → 9 done; axum 0.7, rusqlite 0.32, sysinfo 0.32, notify 7, governor 0.7, dirs 5 |
