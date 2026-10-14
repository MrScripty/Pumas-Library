# Checkpoint execution ledger

## 2026-10-14 — preparation

- Original worktree was clean on `work` at `0dd38c70`. Fetched `origin/main`
  and created task branch at `ef9eead9`; old refs and local inference work remain
  untouched. No model, native runtime or user library was accessed.
- Connected GitHub search found no open Pumas PRs. PR #69 is merged; its head
  `036bed9c` records successful Build 38247599905 and CodeQL 38247594558.
  Those are historical author/CI records, not this executor's validation.
- Local `gh` read API requests returned Forbidden. Connected GitHub reads work;
  Git fetch succeeds. Push/PR outcomes will be recorded separately.
- Plan admitted before implementation; existing headless and release-script
  checks started against current source. Results pending.

## 2026-10-14 — current baseline and launcher fixture repair

- Complete no-default core run passed: 2,070 unit tests, 218 integration tests,
  6 doctests; 29 ignored entries, including top-level subprocess helpers and
  examples. Catalogue recovery passed 7 tests with one child helper ignored at
  top level. No earlier catalogue timing failure reproduced. S3 feature-gated
  integration targets have zero tests in this configuration.
- Release Node suite passed 33/33; version alignment at rc.1, dependency ownership,
  default/S3 attribution and Rust formatting passed. Node 24.19.0 and Python
  3.12.14 differ from repository patch pins; Rust/Cargo is the pinned 1.92.0.
- Launcher initially passed 47 and failed 2: immediate Node startup took 146 ms
  against a 100 ms minimum; terminated PID 6421 remained state Z under PID 1.
  Amended the plan before editing the fixture, retained actual process-group
  termination and force/close checks, gave the early-exit fixture startup margin,
  and observed Linux process state. Corrected complete suite passed 49/49.
- RPC first attempt stopped before tests because CMake was unavailable. CMake
  3.31.6 was installed under `/tmp/pumas-v080-cmake`; the retry uses that binary
  and task-owned XDG configuration. Outcome pending.
- To stay within executor disk capacity, removed 47 task-touched disposable
  incremental-cache directories after core compilation completed. Source files,
  old refs and saved binaries were retained. Preserved the pre-existing RPC at
  `/workspace/.pumas-tools/pre-v080-20261014-pumas-rpc`, 633,094,624 bytes,
  SHA-256 `336690da08cd505c6bb86c8007cd8ddc2976c4f02ec82f089a15fb0bb3d751fa`.
- Current main and PR #69's passing head have the identical Git tree
  `7301cb8e86f9e7a498f5dd64b663a4ea5e174b75`. Historical CI remains labeled
  separately from current local execution. No model/native runtime was downloaded.
- Parent clarified that only actual demo dependency edges need qualification;
  unrelated consumers do not need simultaneous releases. Read-only inventories
  and remaining source/package gates will accompany an immutable receipt.
