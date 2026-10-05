# Desktop review fixes — 2026-10-05

Separate successor `fix/s3-desktop-review-106a6ca4` starts exactly at frozen
anonymous milestone `106a6ca40ac41717854d847214ee3a1cf1e673b4`, tree
`ff61e5b4df248fa31731abcaf2dbdccb17430860`. It addresses two coordinator-supplied
source review findings before authenticated UI work. The frozen branch and
reader/native/PR40/main refs remain unchanged.

Shared ModalDialog now excludes effectively disabled controls using `:disabled`,
including controls disabled by a fieldset and controls selected through explicit
tabindex. A disabled requested initial target also falls back to an enabled
control. The DOM regression reproduces the S3 fieldset/Cancel/Observe/Close
layout and checks forward Tab from Close reaches Cancel and reverse Tab wraps
back. This is DOM/source support only. Actual browser/Electron focus and keyboard
qualification remain parent-owned; Chromium's installed sandbox helper still
prevents a supported local browser launch and no bypass is used.

Rust's S3 request preflight and its generated desktop schema reject literal
VersionId `null` and object keys with empty, dot or parent segments, including
leading/trailing delimiters. These are the frozen reader's exact-preservation
constraints from `validated_object` and maintained object_store Path parsing;
the reader's pure helper is private, so the receiving wire mirrors that grammar
without changing the reader or importing SDK types into the RPC contract.
A differential test observes the actual reader's Configuration refusal for the
same structural inputs before source I/O. Unsupported keys are refused, never
normalized to a different object.

The actual controlled HTTPS/RPC regression submits `null` and `../weights.gguf`
before a corrected request in the same process. Both return invalid parameters;
observer remains Idle, no workspace/acquisition exists, and the corrected request
then imports through the existing Ready/receipt/public lookup path. A direct
admission regression also proves bad pins cannot poison later admission. The
renderer/generated IPC tests reject these exact inputs before sending a start.
No retained-work semantics, native publication policy or reconciliation owner
changes.

Local checks: six focused Rust tests plus one isolated HTTPS child pass; 14
modal/dialog/hook tests pass; 67 Electron IPC/preload tests pass with one existing
native sandbox smoke skipped. Frontend types/lint, bundled preload build,
canonical contract generation/check, rustfmt and diff checks pass. RPC Clippy
passes with all warnings denied except the inherited headless dead-code lint;
the fully strict/default limitations from the frozen milestone remain open.
Evidence is under `/workspace/scratch/s3-desktop-auth/` in `fix-*` logs and
`focus-tests.log`; exact commit/ref evidence is saved after publication.

Production writes are limited to ModalDialog and the RPC S3 wire's preflight/
schema; tests and the six canonical generated contract files support them.
No core/app-manager, reader/manifest/signing, watcher/importer/discovery/native
repair, acquisition schema, dependency or lockfile byte changes. Parent owns
independent review, hosted/browser/default/platform qualification and merges.
The authenticated successor remains a separate feature slice.
