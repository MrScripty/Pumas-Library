# ADR 0009: ONNX Runtime is a downloaded, version-managed runtime

- Status: Accepted
- Date: 2026-11-12
- Supersedes: the packaging and provisioning parts of
  [ADR 0001](0001-onnx-runtime-provider-model.md) ("ONNX Runtime packaging
  remains part of the Rust native dependency closure" and "does not enter
  app-manager's version installation flows").

## Context

ADR 0001 kept ONNX Runtime execution in-process and provisioned its native
library outside Pumas: desktop packages shipped `libonnxruntime.so`,
`onnxruntime.dll` or `libonnxruntime.dylib` beside `pumas-rpc`, pinned to one
release, and developers set `ORT_DYLIB_PATH`. llama.cpp and Torch, by contrast,
are external runtimes the user downloads and selects through the version
manager.

The owner decided that ONNX should not be shipped inside the app: it is an
external runtime that is downloaded like llama.cpp and Torch.

## Decision

- ONNX Runtime (app id `onnx-runtime`) is a version-managed app. Its releases
  are the official `microsoft/onnxruntime` GitHub releases.
- No release is pinned. The listing keeps stable releases whose version exports
  the C API the `ort` binding requests (1.x with x at least the binding's C API
  version, 24 today) and that publish a CPU archive for the host
  (`linux-x64`, `linux-aarch64`, `osx-arm64`, `osx-x86_64` or `osx-universal2`,
  `win-x64`, `win-arm64`). Each is offered as a `vX.Y.Z+cpu` variant. 1.24.2 is
  the combination verified end to end: evidence, not a requirement.
- Installation reuses the llama.cpp native-archive path: the shared acquisition
  service (resumable transfer, publisher SHA-256), attempt custody, completion
  receipts and restart reconciliation. ONNX Runtime has its own acquisition
  consumer (`runtime.onnx-runtime`), versions directory
  (`onnx-runtime-versions/`) and active-version marker. Extraction removes the
  upstream wrapper directory and makes the loaded library a regular file at
  `lib/<platform library name>`, which the receipt hashes.
- The provider loads the library of the active installed version. The host
  (pumas-rpc) resolves it from the version manager and passes it to the session
  manager; it is never part of a request. Resolution order is `ORT_DYLIB_PATH`
  (explicit developer override), the active installed version, then a library
  beside the executable (headless inference archives).
- Without a usable library the refusal says that no version is installed and
  active and how to install a compatible one. Serving returns that reason, and
  an ONNX runtime profile refuses to launch without it.
- Desktop packages stage only `pumas-rpc`; package smoke checks refuse a
  packaged ONNX Runtime library.

Execution stays in-process, as ADR 0001 decided.

## Consequences

- ORT adopts one native library per process and Pumas refuses a different one
  after adoption. Changing the active version therefore takes effect after
  Pumas restarts; until then a load reports that a restart is needed. Switching
  versions without a restart would require an out-of-process ONNX host.
- A fresh desktop install cannot run ONNX embeddings until the user installs an
  ONNX Runtime version. Consumers that require ONNX (for example embedding
  models) must surface the refusal reason.
- Only CPU archives are offered, because the provider registers only the CPU
  execution provider. GPU builds would need a provider change first.
- Headless inference archives still package the pinned runtime beside the
  executable; their release pins, probes and notices are unchanged.

## Revisit When

- the provider gains a GPU execution provider (offer GPU archives as variants);
- ONNX Runtime changes its archive layout or C API compatibility policy; or
- version switching without a restart is required.
