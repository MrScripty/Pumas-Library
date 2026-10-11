# Playground verification — 2026-10-11

Prototype based on published Pumas `main` at
`ad31e391dcd94c204b3fcd748cc98d27b3281e65` (source package v0.8.0-rc.1).
Latest published tag observed was v0.7.0. Only `tools/model-playground` and the
development guide link are in this feature's write set. No runtime implementation,
dependency manifest, model library or desktop workflow was modified.

Standards: repository CONTRIBUTING/DEVELOPMENT/ARCHITECTURE/SECURITY and
MrScripty/Coding-Standards at `188beda1fa477d21d576c233dd8c7f4c4c267d23`,
Core/Router with implementation, verification, commit, documentation, frontend,
IPC, contracts, concurrency, security, diagnostics and relevant detail guidance.
No repository AGENTS.md or task-specific local skill was present.

| Evidence | Actual claim | Boundary |
| --- | --- | --- |
| Node 24.19.0 syntax checks | All playground modules parse | Repository pin is 24.15.0; exact patch was not available |
| 14 Node tests | Six adapter shapes, decoding, 8 MiB attachment boundary, limits, capability state, fencing, redaction, actual loopback HTTP, errors, no replay, cancel, disconnect, shutdown and listener admission | Inference replies are controlled simulations, not models |
| Playwright 1.57.0 / Chromium 151.0.7922.173 | Keyboard discovery, native controls, JSON editing, file attachments, finite results, valid/corrupt PNG decoding with preserved receipts, export, cancel, empty models and 390px layout | Linux representative browser only; no screen-reader/platform conformance claim |
| Existing Pumas binary with disposable empty root/registry | Real health response; real discovery 404 becomes unavailable and disables inference | Older local binary, not a build of current `main`; no model execution |
| `git diff --check` | Whitespace check for the committed change | No core/runtime regression claim |

Screenshots:

- [Desktop workflow, simulated inference](simulated-browser-desktop.png)
- [Mobile cancellation, simulated inference](simulated-browser-mobile.png)
- [Real older-runtime unavailable state](real-runtime-browser.png)
- [Real observation receipt with binary SHA-256](real-runtime-observation.json)

The normal launcher contains no canned inference output. Simulation is exclusively
under `test/`; screenshot runtime identity and response text explicitly label it.
Real HTTP transport to a fixture does not qualify inference, audio conversion,
provider cancellation, GPU memory, image quality or embedding quality.

No model weights or suitable inference fixtures were found in this workspace.
The one indexed-looking diffusion directory contains metadata only. An offline
build of current source was attempted and stopped at missing `aws-sdk-s3` registry
dependency data before compilation. No dependencies/model weights were downloaded
to work around that gap, and no paid service or user's machine was contacted.

Real model coverage remains **untested** for chat, text, embeddings, transcription,
image generation and image-to-text. Audio generation, image editing, 3D and audio
classification have no executable reviewed playground adapter at this revision.
The interface reports each runtime's unavailable reason independently.

No unpublished bugfix was read or consumed. Adoption depends on the user's eventual
running Pumas build exposing compatible discovery and typed operations and reporting
its real readiness. Unpublished audio/model/runtime fixes may be necessary for their
models to become available or succeed; this prototype neither assumes those fixes
nor duplicates them. Refresh discovery and collect real receipts on that authorized
runtime once it is ready. A changed public DTO requires a focused adapter update and
contract evidence, not a backend rewrite.
