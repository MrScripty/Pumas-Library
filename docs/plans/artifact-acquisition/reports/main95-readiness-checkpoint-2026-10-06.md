# Main95 integrated readiness checkpoint

Execution was available after the disconnect notification: normal shell `pwd`,
Git status/object lookup and commands succeeded in `/workspace`. This bounded
checkpoint stops before prefix enumeration and does not merge or update a PR.
No further feature or optional Content-Length hardening was added.

## Exact ancestry

Qualification branch: `qualification/s3-hf-ac08-main95`.

| Input | Head | Tree |
| --- | --- | --- |
| Approved main | `95a0baad2d0aea4650fc36ad4afd969ac9391bf5` | `3ee66988eb1668188011b2124890b10031403ebd` |
| SDK reader | `9077ec90e02a75fc1bf789d361e423187ddeec86` | `1b45dabe2fe8a941a921de8a795a674ae94300e4` |
| AC08 repair | `10f39431c1c4a447d21d34b42670cd26b4b3efff` | `d1ae65f9bc67cb8066f62d2b5ad9a6a87580da59` |
| HF target repair | `4eb273b329631efcd82022206aba7636b7a4ac20` | `483d6a2aa52abbec4dff779962f2abd40152e79c` |

Normal merge chain, with ordered parents:

1. `0e021b0003ffd4b7de66fab1eb7fedcae87050c5`, tree
   `ee41da489d766abb521c7e2d966b669fc92cd1bd`: main95, then SDK9077.
2. `7c8aac72b04478f6d07fa5c36b156457f312ec31`, tree
   `9b04a393e64c6978eb15f39f404ae2b9ae5ccd6a`: 0e021b00, then AC0810f39431.
3. `9a1466f5b127c6bcd815059ca0d2780a0508abe3`, tree
   `cc7e9cbc05c706427b27800834663f84fe49b0f3`: 7c8aac72, then HF4eb273b3.

All four inputs are ancestors. Original refs are unchanged; no rebase, squash,
force push or PR merge occurs. SDK9077 descends from composed S3 production
`493b935c6a41d4d4aeca8e8a66f10b4aba114365`, bringing its earlier Q3 prerequisites.
AC08 and HF both depend on package completeness `aca0ef2a5dcffecfb98c2ba42fc87886b2210ebe`,
which descends from mixed-file `04b0459eedfea1336f200355c37f881e7d2b4f88` and
approved-main second parent `26a84e323cae566a46a8f76bef48fa1010aed48b`.
Thus the main-relative composition includes these prerequisites, beyond the
isolated three recent review deltas.

SDK/main conflicts were Cargo.lock and generated attribution inventory. The
resolved manifest retains main's dynamic ORT configuration and SDK's sole owner;
Cargo resolved the existing SDK lock against that manifest offline. Final lock
has 502 versions. Attribution was regenerated after all merges. Ledger conflicts
retain both histories and unique HF/AC08 sections. A helper assertion about a
shared section's trailing blank line failed, and the shell inadvertently committed
ledger conflict markers in intermediate 7c8aac72. The next merge 9a1466f5 corrects
the ledger from the good 0e021b00 plus both immutable incoming ledgers; shared
sections compare equal after trailing whitespace. The final ledger contains no
markers. This intermediate documentation defect is retained honestly in history.

## Isolated review scopes and regression evidence

- **SDK:** compare 493b935c to 9077ec90: ten files, core Cargo/lock, private
  reader/SDK transport, synthetic auth tests, two reader/acquisition integration
  files, README, ledger and qualification report. Public constructors, receipts,
  immutable/range/retry policy remain compatible. Parent independent review found
  no blocking source defect; runtime results remain author evidence. SDK reader,
  manifest and frozen native source bytes match 9077 exactly in composition.
- **AC08:** compare aca0ef2a to 10f39431: four hook/test files plus ledger.
  Code commit `b64332e07cb6a27aac36febf41cdbdecfed00ea6` is followed by a
  documentation-only child. Valid full pushed snapshots supersede delayed
  unversioned startup lists; late startup cannot resurrect completed/empty state
  or erase a newer error. Unchanged code failed five regressions; paused merge
  remained a control. Header presentation stays downloading at 100% bytes until
  backend Completed. Hook source/tests match this milestone exactly here.
- **HF:** compare aca0ef2a to 4eb273b3: private package selection helper, API
  regressions, README and ledger. Targets must be selected weight payloads in
  the index's SafeTensors/PyTorch format; config/index-self/wrong-format targets
  are refused while valid bin/pt/pth and known part naming remain controls.
  Actual unchanged Rust ran 45 passing index cases and two failures: the public
  config target reached Completed. The first failure stopped that red public
  loop before self-reference; final passing tests exercise both. Failures retain
  partial marker/stub and lack an Adopted receipt. Parent accepted this repair
  independently. The helper and public API source match 4eb exactly here.

## ONNX feature and runtime selection boundary

Approved-main Cargo configuration has `load-dynamic` and no `download-binaries`,
`copy-dylibs` or ONNX TLS features. Resolved ort 2.0.0-rc.12 enables dynamic
loading/preload-dylibs, std, ndarray, tracing, half and APIs 17 through 24;
ort-sys enables disable-linking/std/APIs 17 through 24. Both core and RPC default
plus S3 graphs validate on Linux, macOS and Windows without build-download/copy/
TLS features, object_store, aws-config or an AWS default HTTP client.
The repository's 12 feature contracts also pass. Graphs are not platform builds.

The approved runtime loader/real backend remain byte-identical to main95:
absolute explicit `ORT_DYLIB_PATH` has priority; invalid explicit selection fails
without fallback. With no explicit path, select only the OS library beside the
executable, then canonicalize that exact file. The loader validates OrtGetApiBase,
version compatibility and C API 24, and passes the same canonical path to ort.
There is no build-time runtime acquisition. Actual separately provisioned runtime,
model inference and installed/platform acceptance remain outside this checkpoint.

## Composed checks and attribution

Rust 1.92, locked/offline, defaults plus `s3,test-support`, one worker, debug zero,
no incremental compilation and writable task-local XDG configuration:

| Actual check | Result |
| --- | --- |
| Package filter, including HF target regressions | 52 passed |
| Public HF API | 39 passed |
| S3 units | 13 passed; global-trace and ambient child cases executed by parents |
| S3 acquisition / native workflow / reader | 44 / 7 / 14 passed; cold child executed |
| ONNX library path selection | 2 passed |
| Renderer hooks/completion | 121 passed in three files |
| Header/local actions/remote menu | 32 passed in three files |
| Frontend types / zero-warning lint | Passed |
| Feature contracts / six explicit ORT+SDK graphs | 12 / 6 passed |
| Dependency ownership / attribution tests | Passed / 6 passed |
| Canonical attribution generator/check | 365 default-release entries, validated |

Internal logs live in `/workspace/scratch/readiness-main95/`: `package.log`,
`hf-api.log`, `s3-unit.log`, `s3-integration.log`, `ort-selection.log`,
`ort-features.log`, `feature-contracts.log`, `ort-sdk-platform-graphs.log`,
`renderer.log`, `renderer-presentation.log`, `frontend-types.log`,
`frontend-lint.log`, `attribution-generate.log`, `attribution-check.log`,
`attribution-tests.log` and `dependency-ownership.log`. Cached Node dependencies
were linked after manifest/lock byte comparison and every exact link was retired.
An initial renderer invocation used three nonexistent presentation path filters;
its 121 valid tests passed, then the three actual repository presentation files
ran separately for 32 passes. No browser access workaround was attempted.

Canonical default-release notices/inventory are refreshed using the unchanged
generator and actual composed inputs. They describe the default release profile,
which does not enable optional S3. The prior SDK report's authoritative optional
S3 license evidence covers its separate closure; an S3-enabled distribution must
include those additional terms. Frozen earlier evidence/inventories are retained.
No S3-enabled release attribution or production package is falsely certified by
this default-only regeneration.

The composed targeted source tests pass; individual milestone lint/evidence
remains preserved. A new full RPC/default suite, broad strict RPC test-target
lint, hosted checks, live providers, desktop browser/focus/accessibility and
hard-kill/platform qualification were not added to this bounded checkpoint.
The earlier 11 inference-disabled RPC test-target dead-code warnings remain a
scoped limitation, with no suppression. No acquisition/runtime gate is advanced.
Workspace disk was about 440 MiB free after affected Rust checks; no shared
cache/evidence cleanup or additional build was undertaken.

Parent owns remaining review/PR/integration decisions. No real credentials,
account provisioning, paid services, runtime downloads or new feature work occur.
Execution stops at this checkpoint as requested for the weekly usage ceiling.
