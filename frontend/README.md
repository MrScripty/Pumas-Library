# Frontend

The frontend is the React renderer for the Pumas desktop app. It presents
backend-owned library and runtime state; it does not access the filesystem,
spawn processes, or decide durable model status.

## Stack and Boundary

- React 19 and TypeScript
- Vite 6
- Tailwind CSS 4 and semantic CSS tokens
- Framer Motion
- React Aria interaction primitives
- Vitest, Testing Library, and jsdom

The renderer calls the typed adapter under `src/api/`, which delegates to the
sandboxed Electron preload API. Values received from Electron, the backend, or
browser persistence are untrusted at runtime even when a TypeScript interface
describes their expected shape. Decode them before they enter application
state.

```text
components and hooks
  -> frontend API adapter
    -> window.electronAPI
      -> Electron IPC and local RPC
```

## State Ownership

- Hooks own async work, cancellation, cleanup, stale-invocation protection, and
  renderer subscriptions.
- Components own presentation and short-lived interaction drafts.
- The Rust backend owns model availability, download state, repair outcomes,
  runtime profiles, serving instances, and status telemetry.
- Cached startup data is a projection. Its age, provenance, refresh state, and
  degraded outcome must remain visible to the caller and, where relevant, the
  user.

Do not infer that a model is complete from its name, repository, or a partial
set of files. Repository identity and artifact identity are distinct: one
Hugging Face repository may contain multiple files or quantizations, while the
same model published in another repository remains a separate model.

Hugging Face download-details responses are decoded in preload from the
backend-owned contract before hydration. Unknown sizes remain null, and ordered
quant/file-group identities are preserved. A response for a different repository
or an invalid nested payload cannot replace the current row's details; failures
leave the existing projection intact and allow retry. This does not validate
all search responses or prove remote files are downloadable.

Download-details requests accept omitted/null quant selection or an array of
strings. Malformed selections are rejected before IPC and at RPC admission,
not replaced with an empty selection. RPC accepts either `repo_id` or `repoId`,
but not both or extra fields; exact string contents and ordering are preserved.

Inference-settings reads use backend-generated decoding, including nullable
constraints and structured JSON defaults. The modal keeps drafts and read
results scoped to one model and rejects mismatched identities. Failed settings
reads show an unavailable state, not an editable empty list; close and reopen
to retry. Structured defaults display as read-only JSON. Settings updates require
an explicit array; malformed or missing settings reject before IPC and RPC
persistence. An explicit empty array clears stored overrides and restores the
backend's lazy defaults. Valid strings, ordering and nested JSON remain intact.

Notes updates accept text or an intentional clear (omitted, null, or blank).
Malformed values, duplicate aliases and unknown fields reject before IPC and
RPC persistence, rather than accidentally clearing notes. Nonblank Markdown,
Unicode and whitespace are preserved exactly.

Notes/settings save confirmations use generated response decoding and must
match the requested model. Notes clears omit the returned text; missing-model
failures are distinct from successful saves. Malformed responses or transport
errors leave drafts intact and display an unconfirmed-save warning, without
automatic retries. Reopen the model to check persisted values before retrying:
an unconfirmed response does not prove that the backend write failed.

Available runtime-version reads decode the backend's camelCase release and
asset fields, including nullable sizes and descriptions. Rate-limit responses
have no version list; the UI keeps its last releases and records the known or
unknown retry delay. Malformed responses reject rather than dropping individual
rows or accepting legacy field spellings. Existing refresh timing is unchanged.

Runtime GitHub cache snapshots are decoded before entering hook state. Full
snapshots preserve nullable age, timestamp and release count; the existing
no-manager response omits those fields and contains only three false flags.
Malformed snapshots leave the last valid state intact. This does not establish
runtime availability or change cache polling timing.

Installed runtime-version lists are decoded before entering state. Exact tags,
ordering and duplicates are preserved; malformed lists report an error and keep
the last valid list. Runtime lookup and installation behavior are unchanged.

Active/default version reads share a validated string response. Empty strings
retain the existing wire meaning of no selection and display as null; other
tags remain exact. Invalid replies leave the previous selection intact.

Comprehensive runtime status validates required nullable selections, the installed
count and each version's dependency lists before entering state. Invalid replies
report an error and retain the previous status and default selection. Existing
no-manager empty snapshots and failed dependency checks represented as empty
lists remain limitations; validation does not establish runtime or dependency
availability.

The dedicated `check_version_dependencies` bridge method uses the generated
request/outcome contract and requires both a runtime tag and app id. It reports
the backend's installed/missing names and optional `requirementsFile` after a
subprocess-backed path/requirements/`pip list` observation. Rust makes no
explicit dependency-install, cache-write or network call, but inherited
Python/pip subprocess effects are not excluded or proved absent. Unknown app
identities remain missing-manager errors, disabled inference-plugin builds are
unsupported, and missing versions remain errors, while an absent venv or
requirements file retains the
backend's explicit report semantics. A valid response means the check returned a
typed report, not that dependencies are ready. There is currently no hook or UI
state consumer for this route, so the direct exposed bridge is the renderer
boundary; malformed responses and transport failures reject without becoming
empty data or triggering retries. FE-I42 and FE-I43 track tag-derived path
reachability and flattened package-list failures.

The dedicated `get_release_dependencies` bridge method uses the generated
request/outcome contract and requires both a runtime tag and app id. It reads
the selected release's `requirements.txt` and preserves the backend's extracted
strings, ordering and duplicates exactly; the wire also preserves all admitted
strings, including empty values. The earlier preload
signature accepted an optional numeric `topN`, omitted the required app identity
and sent an unused `top_n` field; the coordinated internal bridge now sends the
actual Rust request instead. Missing versions or requirements files remain
successful empty lists, so success proves only that the current textual
extraction completed or the file was absent. It does not establish dependency
completeness, installation or runtime readiness. There is no current hook or UI
state consumer: malformed responses and transport failures reject at the direct
bridge without retries, state replacement or dependency installation. FE-I42
tracks tag-derived path reachability and FE-I44 tracks ambiguous empty results
and the approximate parser.

The dedicated `install_version_dependencies` bridge method requires a runtime
tag and app id through the generated request contract, then validates the exact
`{success:boolean}` confirmation. The awaited backend operation may create a
virtual environment, run `ensurepip`, upgrade pip, install requirements, read or
write constraint/cache data, create a pip-cache directory and contact package
services. A true response means either no requirements file was found after any
virtual-environment setup or the final pip install exited successfully; it does
not prove dependency readiness, prerequisite command success or that constraints
were applied. There is no current hook or UI state consumer, so the direct bridge
is the renderer boundary. False confirmations pass through; malformed responses
and transport errors reject after one mutation request without an automatic
retry. FE-I42 tracks tag-derived process and mutation reachability, FE-I45 tracks
the unbounded/cancellation-free subprocess lifecycle and unsafe uncertain retry,
and FE-I46 tracks ignored prerequisite and constraint/cache failures.

Runtime-version info preserves the backend's exact tag and installed flag with
its current required null size. It does not expose installation paths, dates,
release metadata or a computed size. The generated preload decoder rejects
missing fields, non-null size and invented metadata before the hook receives the
record. A no-manager installed-false result does not prove runtime availability.

Runtime installation validation returns the backend's raw removed-tag list,
orphan-directory path list and valid count without a success wrapper. Validation
may remove stale installation metadata, but does not delete reported orphaned
directories. The preload rejects malformed results without retrying the
operation. Empty no-manager results do not establish runtime availability.

Runtime installation progress is validated as the backend's raw nullable
camelCase snapshot before one explicit projection into the UI's established
snake_case state. Terminal success/failure facts, nullable fields and completed
items are preserved; malformed reads keep the last progress, mark network state
failed and continue scheduled polling without repeating the installation
mutation. A null no-manager/no-progress result does not establish availability.

Runtime installation start validates one generated request and the backend's
exact discriminated started/failed records. A successful start means release
lookup completed and a detached worker was spawned; installation completion is
still established only by progress polling. False, malformed and transport
failures do not start polling, refresh versions or retry the mutation. An
uncertain response is not proof that no worker started. The backend's shared
installation state and detached-worker lifecycle remain documented limitations.

Runtime installation cancellation validates the backend's exact success boolean.
Success means a cooperative cancellation request was accepted, not that the
worker has stopped. False confirmations remain visible failures; malformed
confirmations reject after one request and are never retried. A false no-manager
result does not establish runtime availability.

Runtime-version removal validates its own generated success-boolean outcome.
Success is returned only after the backend removal method completes and the hook
awaits its version refresh. False or malformed confirmations do not refresh, and
a rejecting refresh callback after accepted removal never retries the destructive
request. Current refresh readers can retain prior values and expose their own
error state without rejecting, so success does not prove every read refreshed.
Backend removal is sequential rather than transactional, so an error must not be
interpreted as proof that no removal effect occurred.

Runtime-version switching validates its own generated success-boolean outcome in
both the direct bridge method and the composed launch path. False or malformed
confirmations cannot refresh or launch a runtime, and later refresh or launch
failures never repeat selection. Success means the backend selection writes
completed; it does not establish that a runtime process is running or ready.

Runtime launch validates an exact generated outcome with `success` and optional
non-null `error`, `log_path`, and `ready` fields. Direct Ollama/Torch launch and
the composed version-selection adapter share that decoder. A successful launch
means process creation and the producer's bounded readiness observation
completed; `ready: false` remains a successful launch, and readiness is not
proof of process or version identity or continuing health. The active process
hook uses its later status observation, rather than `ready`, to clear starting
state. A decoded false outcome updates or clears the log from that failure;
malformed and transport outcomes preserve the prior log. All surface failure and
never retry launch. Composed selection is not transactional:
a successful switch remains applied if the later launch fails or is uncertain.

Default runtime-version selection shares one generated request contract across
preload, Electron main and standalone Rust RPC. Omitted/null tags clear the
selection; exact strings are preserved and malformed values reject before any
mutation. Its exact success boolean is decoded before hook state changes. True
applies the confirmed selection and performs one status refresh; false or
malformed replies do neither, and refresh failure never repeats the mutation.
Backend persistence follows the in-memory change, so an error is not proof that
no selection effect occurred.

Library metadata reads preserve omitted optional payloads and validate present
metadata as objects, including nested JSON and component-manifest states.
Malformed responses cannot enter the modal; nested values display without
object coercion, and non-string link fields remain plain text. This does not
change metadata extraction or its existing handling of unavailable metadata.

Complete current GGUF and safetensors rows offer format conversion to the other
format (F16). The dialog requires explicit tool-installation consent, observes
backend progress and requests cancellation without assuming it has completed.
Closing stops UI polling, not backend conversion; reopening reads existing work.
Unconfirmed start/setup outcomes block repeated requests in that dialog. Setup
readiness requires verified Python imports, not merely an existing interpreter.
The backend and desktop APIs remain usable without the GUI. Quantization-specific
configuration remains [FE-I23](../docs/plans/current-standards-remediation-2026-09-03/frontend-and-ui/issues.md)
follow-up; the format-conversion control does not advertise quantization.

## Source Guide

| Path | Responsibility |
| --- | --- |
| `src/api/` | Electron/backend adapter and response normalization |
| `src/components/` | Screens, panels, and reusable UI |
| `src/hooks/` | Async workflows and subscriptions |
| `src/types/` | Consumer-side TypeScript projections |
| `src/config/` | Feature and theme mappings |
| `src/utils/` | Pure presentation and provider utilities |
| `src/test/` | Shared test setup and helpers |

Prefer grouping code by owned capability over adding another cross-cutting
utility or an explanatory README in every directory.

## Themes and Accessibility

Theme tokens are defined in `src/index.css`; programmatic theme mappings live
in `src/config/theme.ts`. Use semantic tokens instead of hard-coded colors.

Prefer native semantic controls. Composite widgets must provide complete
keyboard, focus, naming, dismissal, and state behavior. The lint policy
requires React Aria `useHover` instead of raw mouse hover handlers. jsdom tests
are useful for component contracts but do not prove real focus, layout,
contrast, reduced motion, or assistive-technology behavior.

Installation details expose named, determinate progress. Routine progress
updates are not live announcements; one atomic terminal region announces
failure assertively and cancellation or success politely. This installation
surface exists only in the default build.

The renderer follows the operating-system reduced-motion preference. Reduced
mode removes nonessential Popover entry and dismissal translation while
retaining opacity feedback, and bounds CSS animations and transitions to a
near-zero duration and one iteration.

## Commands

From the repository root:

```bash
npm run -w frontend dev
npm run -w frontend lint
npm run -w frontend check:types
npm run -w frontend test:run
npm run -w frontend build
npm run -w frontend build:library-only
```

The default build includes inference integrations. `build:library-only`
removes their UI.

For representative desktop behavior, build and run through the root launcher.
See [Development](../docs/DEVELOPMENT.md) and the
[current standards audit](../docs/audits/current-standards-2026-09-03/README.md).
