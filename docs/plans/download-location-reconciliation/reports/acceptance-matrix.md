# Required acceptance claims

Authority: [plan](../plan.md). All claims are **pending** at plan creation;
no implementation or new acceptance test has run. Evidence owner for every
claim: the implementing agent; manual desktop observations identify the operator.
Detailed procedures are in [verification](verification.md).

| ID | Observable criterion | Evidence kind | Required environment | Mode | Status / evidence |
| --- | --- | --- | --- | --- | --- |
| C1 | No reachable shared-workspace mutator moves/deletes/rebinds an admitted transfer destination. Real multi-file transfer survives watcher-triggered classification; deferred classification converges after custody release. Every bounded sibling has a tested or sufficient construction-proof disposition. | integration | representative: actual core, stores, loopback producer and filesystem watcher on supported desktop targets | automated | pending |
| C2 | Each of the three historical shapes, including all 33 selected LLaDA files, resolves through explicit local acceptance. New-path files remain; original Error snapshot survives in history; exact queue admission is durably released; no HF completion receipt or fabricated immutable source is created. Unrelated paused admissions remain unchanged. | integration | representative: real core/SQLite/JSON, tiny authored payloads, absent old paths, renamed catalog paths | automated | pending |
| C3 | Missing/partial selected files, ambiguous candidates, wrong selection/root/attempt, stale preview, invalid metadata, unsupported history/version and escaping paths reach their intended boundary and return the specified typed result without authoritative mutation. | focused and contract | representative: valid positive fixture altered one condition at a time; actual decoder and capability boundary | automated | pending |
| C4 | Active/hidden admissions, modern acquisition effects, deletion/intent claims and unresolved quarantine cannot be bypassed by legacy resolution or sibling mutation. Own retained admission is settled only by exact resolution authority after verified quiescence. Cancel/shutdown retain ownership through required effects. | integration | representative: real task/custody owners and stores with deterministic synchronization | automated | pending |
| C5 | Envelope-7/HF-5 converts losslessly to envelope-7/HF-6 in the admitted transaction. Old HF reader rejects new state without writes; generic acquisition writer preserves the opaque HF partition. Previously supported outer migration inputs and existing modern receipt semantics still pass; malformed/unknown versions fail explicitly. | contract | representative: retained old-reader fixture plus actual current decoders/publisher and existing migration fixtures | automated | pending |
| C6 | Interruption before/after publication, uncertain parent sync, lost acknowledgement and notification loss converge to either retained original failure or the exact durable resolution/release pair. Duplicate requests are idempotent; conflicting operation reuse fails; no mixed history/release state or unsupported automatic retry. Normal reopen is healthy. | system | representative: disposable process-isolated library, real publisher, injected publication faults and controlled process termination | automated | pending |
| C7 | User sees that local files exist separately from an old failure, can inspect and choose Keep existing files, receives honest unverified-provenance wording, and sees the attempt resolved in the same window. Reopen preserves the result. Missing/ambiguous/busy/uncertain cases remain actionable; no direct metadata edit or manual offline procedure is required for schema-7 resolution. Keyboard and screen-reader semantics work. | user-workflow | required-real: supported desktop session, normal Electron sandbox, actual renderer/preload/IPC/RPC/core | either | pending |
| C8 | Recovery performs bounded metadata/stat work only: no model-byte reads/hashes, copying, backup, move/delete, destination recreation or network transfer. Filesystem/device identities and selected files remain unchanged, metadata IO is bounded and unrelated state is preserved. | integration | representative: real capability boundary and tiny fixture; large logical library represented without large allocations | automated | pending |
| C9 | All applicable required repository suites and static gates pass on the candidate source. All scoped MUST findings have concrete closed dispositions; composed-design and generated-contract semantic reviews agree with the implemented artifact. Exclusions are only documented unsupported consumers, not suppressed failures. | focused, contract and integration; static/design supporting gates | representative: pinned toolchains, repo `.venv`, subprocess/loopback access; native CI targets where platform-dependent | automated checks plus recorded review | pending |
| C10 | Newly rebuilt Linux amd64 `.deb` contains the candidate backend/frontend, installs/extracts correctly, performs C7 on the authored legacy fixture, remains healthy through close/reopen and shuts down cleanly. Final package hash, source revision, build inputs, attribution and environment are recorded; startup smoke alone is insufficient. | release-artifact and user-workflow | required-real: supported Linux desktop and native package verification environment, isolated fixture/config | either | pending |

For C1, cross-platform claims cover the supported Linux/macOS/Windows path and
custody implementations; retain existing native CI qualification and run affected
new cases on those targets. Linux Debian delivery is C10; it makes no claim that
new Windows/macOS installers have been released. If a required native target
cannot run, retain its claim as blocked; local Linux agreement does not replace it.

For C8, a scoped test-only observer at the owning effect boundary may record IO
operations and network requests. Keep it private, compare actual operations with
the contract, and pair it with independent stat/device observations. Hash tiny
authored fixtures only when useful to prove unchanged contents; do not hash or
read the user's weights. A metadata projection agreeing with another projection
does not prove payload integrity or historical HF success.

Each satisfied entry must link a report recording tested source, command/procedure,
test names/counts, environment, result and limitations. Record `blocked` when an
environment/oracle is unavailable and `invalid` evidence when a failure never
reaches the intended boundary. Do not promote a generic exception, a zero-test
filter, generated freshness, or a copied implementation expectation into proof.
