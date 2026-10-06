# AC10 public HF importer measurement — 2026-10-06

The Linux-only ignored `public_hf_large_transfer_preserves_file_identity_and_records_resource_usage` regression streams a synthetic 512 MiB GGUF-shaped object through public core HF admission, the existing acquisition owner, the real model importer and receipt-backed settlement. It adds no production hook, dependency or public API. Default and no-default runs use separate fresh processes, a workspace `overlay` filesystem and a fixed 64 KiB source/hash buffer.

## Provenance and commands

Base: `722245514b3bae42511aa5ea188570292ce276d4`, tree `14e8a1b36b580f2397264dd38b18329ec07bc75e`. Admission: `d3d93f8b5da09f7d71c96631aab141c6bae9fb6c`. Source: `8a147160c2b3743c549e00a89b1e48f54d7a2390`, tree `9544e16dde7802baa2033bf7ceea7d44c2ba3767`; `api/hf.rs` blob `855fe4272889d7518d572461160b35a02a299d23`, SHA-256 `6d6f4bf94d43a3095e81cb853dd9738687c0ccc7d2f35f4871911d9fe3299bce`. Branch: `qualification/ac10-public-hf-72224551`. Independent specification and standards reviews accepted this exact blob; their source-only ACKs do not accept a dependency gate.

Rust/Cargo 1.92.0, Linux 6.18.44 x86_64. All Cargo commands use the retained serialized wrapper, `--offline --locked`, one job, incremental disabled, shared target `/workspace/Pumas-Library/rust/target`, and explicit `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`. These settings are part of the measurements, including process RSS. The repository-local author/committer identity is the owner-selected MrScripty identity, verified before and after each commit.

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  /tmp/pumas-cargo-admission/run-cargo-serialized test --offline \
  --manifest-path rust/Cargo.toml --locked -p pumas-library --lib \
  public_hf_large_transfer_preserves_file_identity_and_records_resource_usage \
  -- --ignored --nocapture --test-threads=1
# Repeat in a separate process with --no-default-features before the test filter.
```

The checked-in [evidence](ac10-public-hf-2026-10-06/evidence.json) binds exact commands, exits, versions, source, JSON metrics and SHA-256 hashes of retained raw logs. The archive preserves diagnostic bytes, including failed attempts. It contains no collected environment values or credential material. Original scratch evidence remains under `/workspace/scratch/ac10-public-hf/`.

## Supporting validation

Both opt-in measurements and the existing public mixed-size status/import-settlement control pass 1/1 in each feature mode. Strict all-target `pumas-library` Clippy passes with warnings denied in default and no-default configurations. Scoped rustfmt and Git diff checks pass. Exact argv, exits and raw logs are bound in the evidence JSON. This scoped check set does not replace hosted current-head qualification.

## Measurements

| Observation | Default | No default features |
| --- | ---: | ---: |
| Wall time (ms) | 79,265 | 81,782 |
| Stream release to importer hold (ms) | 60,268 | 62,240 |
| Other new logical bytes | 10,385 | 10,385 |
| Other new allocated bytes | 20,480 | 20,480 |
| Before VmRSS (kB) | 56,140 | 54,872 |
| Before VmHWM (kB) | 56,140 | 54,872 |
| Importer hold VmRSS (kB) | 99,004 | 96,536 |
| Importer hold VmHWM (kB) | 99,864 | 96,868 |
| First chunk rchar (bytes) | 2,377,968 | 2,377,968 |
| First chunk wchar (bytes) | 993,072 | 993,072 |
| First chunk read_bytes (bytes) | 0 | 0 |
| First chunk write_bytes (bytes) | 1,314,816 | 1,310,720 |
| After promotion rchar (bytes) | 2,687,343,488 | 2,687,360,174 |
| After promotion wchar (bytes) | 537,960,551 | 537,936,687 |
| After promotion read_bytes (bytes) | 0 | 0 |
| After promotion write_bytes (bytes) | 538,148,864 | 538,173,440 |


Each run observed exactly four expected pinned HF metadata/tree/object requests and 536,870,912 payload bytes. The first public progress hold corresponded to a physical 65,536-byte regular `.part` file. That device/inode remained identical at the real importer metadata-write hold and after settlement. The final 536,870,912-byte file occupied 536,875,008 allocated bytes. At the importer hold, public status remained `Downloading` with full bytes, exact selected/model identity, `Using` custody, retained queue admission and no completion receipt. After release, repeated public `Completed`, metadata/index publication, `Adopted` custody, settled admission and a receipt bound to the exact acquisition/lease/demand/manifest/workspace/files were required. Independent final hashing matched source and receipt SHA-256 `cf4ca2aa6340e8946b852787296e6e2555ba9675d64ebedd795c14ad71996be3`.

Full-root regular-file inventories before start and after settlement deduplicate device/inode without following symlinks, report changed baseline files and reject any other artifact-sized file, including a grown baseline file. The model has exactly its expected path and no retained alias; its final no-follow regular-file identity is rechecked. Other new files and the changed DB WAL are separately recorded. The authored sentinel remains intact. Source task drainage and owner shutdown succeeded.

## Interpretation and limitations

`VmRSS` and `VmHWM` are raw Linux kB samples before start and at the importer hold. `VmHWM` is a process-lifetime maximum, not a resettable transfer peak; its difference is not transfer-only peak RAM. `/proc/self/io` records raw `rchar`, `wchar`, `read_bytes` and `write_bytes` at the first-chunk hold and after promotion before independent final hashing. Counters include both source server and downloader in this process. Kernel block accounting does not establish physical-media writes; zero `read_bytes` does not mean no logical reads. The inventory establishes retained files at its snapshots; it cannot exclude transient removed copies, page-cache memory or reflink/shared extents. Counts are configured capacity, not a measured concurrency envelope.

This fixture uses the existing public-core recovery fixture without watcher, IPC, global registry or background connectivity. It exercises the real managed HF transfer/importer workflow, not the installed backend resource envelope, Electron, inference or an actual provider. One synthetic Linux workload does not qualify concurrent limits, copying fallback, other filesystems/platforms, deployed durability, HTTP fault behavior or full AC10/AQ-HTTP.

Start is bounded to 60 s, source accepts to 15 s, initial progress to 30 s, transfer/verification/handoff to 300 s, importer entry/release to 30 s, cancellation to 15 s, terminal settlement to 60 s, owner shutdown to at most two 30 s attempts and source join to 15 s. Finite getters use 30 s and final hashing cooperates between chunks under 60 s. Individual synchronous filesystem syscalls cannot be preempted by these async deadlines. Every post-start failure releases both gates, attempts cancellation and owner drainage, then signals/joins or aborts/awaits the source before assertions; undrained roots are retained.

## Failed attempts and resource guards

The first compiled fixture (`c092b47d7088f54a62da53c3774578c437f53b05`) failed at the 30-second importer-entry wait because that wait began when the server finished writing, before the existing owner finished verification/handoff. The run served the full body, preserved partial/final inode and drained via cancellation/shutdown, but did not qualify importer settlement. Its missing later artifact-path observation also caused a secondary inventory assertion; the recorded inventory contains one model file. The narrow repair now includes marker removal after `files_ready` and package validation in the same 300-second transfer phase before the unchanged 30-second importer wait. Independent `Using`, no-receipt, inode and final settlement assertions remain. Both reviewers accepted the exact repair. The earlier UUID Serialize compile error and interrupted builds remain archived, without passing claims.

Builds were serialized with disk checks. Authorized inactive reproducible Cargo cache files were hashed before removal; manifests are retained. Protected source, retained custody/evidence roots, release archives, production owners and frozen S3/native write sets remain unchanged. Own temporary PR42 dependency copies were parked under `/tmp/pr42-build-cache` to recover workspace headroom; locked dependency sources/manifests/versions were not changed. After all Cargo checks completed, those temporary PR42 dependency copies were restored to their original worktree locations under a headroom guard; the restoration manifest is archived. The payload fixture itself remained on `/workspace`, not tmpfs.

## Gate disposition and next existing-plan feature

AC10 and AQ-HTTP remain pending. Q2 wheel-file-set acquisition/local-only Torch consumption is the next existing-plan implementation feature, gated by AQ-HTTP. Its required boundary remains an accepted exact wheel closure, shared verified local handoff, installation under denied network, a lease held through child exit/cleanup, installed-output proof, and separate resolver/bootstrap traffic accounting. Current `resolve_runtime.py --install` resolves and installs before emitting its report/lock, and `installer/torch.rs` consumes that combined post-install result; it does not provide this boundary. No Q2 source work is included. Parent owns PRs, hosted qualification, reviews and integration. Real-provider and supported-platform S3 acceptance remain separate.
