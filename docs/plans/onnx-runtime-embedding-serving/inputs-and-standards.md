# Inputs And Standards

## Inputs

### Problem

Pumas can already expose served llama.cpp GGUF embedding models through the
OpenAI-compatible `/v1/embeddings` gateway, but ONNX embedding models are only
recognized as model-library artifacts. There is no first-class ONNX Runtime
provider, no ONNX sidecar endpoint, and no serving workflow that records ONNX
models as backend-owned served instances.

The specific user need is to serve an ONNX `nomic-embed-text-v1.5` model so
Emily and other external apps can use it through a stable local endpoint.

### Standards Reviewed

- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/PLAN-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/CODING-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/ARCHITECTURE-PATTERNS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/SECURITY-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/TESTING-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/TOOLING-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/FRONTEND-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/ACCESSIBILITY-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/DEPENDENCY-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/DOCUMENTATION-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/CONCURRENCY-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/CROSS-PLATFORM-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/LAUNCHER-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/INTEROP-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/RELEASE-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/languages/rust/RUST-API-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/languages/rust/RUST-ASYNC-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/languages/rust/RUST-SECURITY-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/languages/rust/RUST-DEPENDENCY-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/languages/rust/RUST-INTEROP-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/languages/rust/RUST-CROSS-PLATFORM-STANDARDS.md`
- `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/languages/rust/RUST-TOOLING-STANDARDS.md`

### Constraints

- Backend-owned served state remains authoritative. Frontend may hold form
  drafts only.
- Boundary validation must happen at sidecar and RPC boundaries before file,
  network, or process operations.
- The sidecar must default to loopback binding and keep LAN exposure behind
  explicit policy and auth.
- The Pumas `/v1` gateway remains the external application facade.
- Public external access remains facade-first: external apps use the Pumas
  `/v1` gateway, not raw provider endpoints, unless a separate plan explicitly
  makes a raw provider endpoint a supported product contract.
- ONNX Runtime provider additions may replace existing provider dispatch and
  route contracts when those contracts encode two-provider assumptions. The
  implementation must remove obsolete fallbacks and route shapes instead of
  carrying legacy compatibility paths.
- Runtime profile lifecycle must identify who starts work, who stops it, how
  health is checked, and how stale PID/status state is handled.
- Dependency additions must be owned by the ONNX sidecar, not leaked into
  unrelated Python sidecars.
- Gateway routing must check provider endpoint capabilities before proxying an
  OpenAI-compatible path.
- Gateway alias and provider-side loaded model id must be modeled separately
  enough that ONNX does not inherit Ollama or llama.cpp naming assumptions.
- Tests must verify behavior through public contracts where practical, not
  implementation details.
- Before implementation begins, dirty implementation files must be resolved,
  committed, stashed, or explicitly allowed. Plan/docs edits may remain dirty
  while this plan is being refined.
- Contract changes must be defined and reviewed before implementation slices
  depend on them. Rust DTOs, TypeScript bridge types, plugin metadata, persisted
  JSON shape, and sidecar HTTP payloads must be updated in the same logical
  slice when they describe one boundary.
- Blocking filesystem, process, dependency-install, and model-load work must
  stay out of async request paths unless isolated through the existing
  blocking-work pattern.
- Runtime wiring follows the composition-root pattern. Concrete provider
  behavior, serving adapters, gateway clients, and launch strategies are wired
  at application/runtime boundaries; domain policy code must depend on narrow
  contracts rather than constructing infrastructure clients ad hoc.

### Standards Compliance Guardrails

- **Architecture:** Keep provider policy in core/domain provider behavior or
  adapter modules. Transport handlers and React components may consume provider
  capabilities, but must not duplicate provider rules.
- **Contracts:** Treat runtime-profile JSON, plugin manifests, served-model
  snapshots, RPC payloads, OpenAI-compatible HTTP payloads, and sidecar control
  payloads as executable boundary contracts with runtime validation and
  serialization tests.
- **Security:** Validate request payload shape, model ids, aliases, endpoint
  URLs, paths, ports, batch sizes, token counts, vector dimensions, and bind
  hosts at the process/API boundary. Canonicalize model paths against approved
  roots before any sidecar load.
- **Concurrency:** Every background task, process lifecycle operation,
  inference queue, and polling/subscription owner must have explicit ownership,
  bounded capacity, cancellation/shutdown behavior, and test coverage for
  cleanup or stale-response handling.
- **Dependencies:** ONNX Runtime, tokenizer, and numerical dependencies belong
  only to `onnx-server/` unless another owner demonstrably executes them.
  Dependency additions must document why in-house code is not appropriate,
  license/maintenance status, transitive cost, and CPU/GPU package strategy.
- **Frontend:** Frontend state remains transient; served state and runtime
  profile state remain backend-owned. New controls must use semantic elements,
  accessible labels, event-driven updates where available, and deterministic
  cleanup for any unavoidable polling.
- **Cross-platform:** Use path APIs and platform abstractions for executable,
  venv, script, PID/log, and model paths. Do not hardcode separators or scatter
  platform checks through handlers or UI code.
- **Release:** Release validation must cover packaged sidecar dependencies,
  managed state isolation, launcher-compatible build/smoke paths, dependency
  audit evidence, and user-visible changelog/release notes when the feature
  ships.
- **Documentation:** New source directories with non-obvious purpose, especially
  `onnx-server/`, must include README contract sections for API consumers,
  structured producers, lifecycle, errors, and compatibility.

### Standards Compliance Gates

These gates are implementation blockers. A milestone is not complete until the
applicable gates are satisfied and evidence is recorded in Execution Notes or
PR notes.

| Standard Area | Gate |
| ------------- | ---- |
| Plan execution | Confirm dirty implementation files are resolved, committed, stashed, or explicitly allowed before code implementation begins. Commit each verified logical slice before starting the next slice. |
| Architecture | Provider rules live in core/domain provider behavior or narrow adapters. RPC handlers, gateway handlers, React components, and plugin metadata consume capabilities only; they must not encode provider policy. |
| Composition roots | Concrete provider registry entries, serving adapters, gateway HTTP clients, process launch strategies, and sidecar model-manager implementations are wired by lifecycle/composition owners, not created inside business-policy functions or UI components. |
| Contracts | Rust DTOs, TypeScript bridge types, persisted JSON, plugin manifests, gateway payloads, and sidecar HTTP payloads change together for each boundary slice. Runtime validation and serialization/round-trip tests are required before consumers depend on new shapes. |
| Executable contracts | Multi-producer or multi-consumer boundary schemas live in a dedicated contract/schema owner or documented facade module. Defaults, enum semantics, normalization, ordering, and compatibility rules are tested, not inferred from TypeScript interfaces alone. |
| Security | Raw IPC/HTTP/config payloads parse once into validated types at boundaries. Internal provider code must not accept unchecked raw paths, URLs, ports, aliases, dimensions, batch sizes, or model ids when a validated domain type can carry the invariant. |
| Rust API | Use `Result` with structured errors for recoverable production failures. Do not add `unwrap()` or `expect()` to production request, lifecycle, or background-service paths. Use newtypes/enums for expensive route, provider, endpoint, lifecycle, and placement invariants. |
| Rust async | Keep pure validation, policy, and routing logic synchronous. Async code is limited to I/O shells. Every spawned task has an owner, tracked handle, cancellation path, shutdown behavior, and panic/cancellation observation. |
| Concurrency | Queues, inference work, lifecycle restarts, gateway requests, and frontend polling/subscriptions have bounded capacity, stale-response handling, and deterministic cleanup tests. |
| Dependencies | ONNX dependencies are declared at the narrowest owner (`onnx-server/`) and pinned through that owner’s lock strategy. Root/workspace manifests must not carry ONNX-only runtime dependencies for convenience. Package-local lint/test/install commands must succeed from the sidecar owner without relying on unrelated root dependencies. |
| Testing | The first implementation slice includes a failing-first vertical acceptance test through the real gateway path before broad horizontal expansion. Integration tests isolate temp roots, ports, persisted profile files, process state, and environment variables, and affected integration suites are run with normal parallelism plus at least one repeat pass before merge. |
| Frontend | Backend-owned served state and runtime profile state are never optimistically mutated. Interactive controls use semantic elements, accessible names, keyboard coverage, and role/name selectors in tests. |
| Documentation | New or materially changed source directories update README contract sections. Provider capability and provider-scoped route architecture gets an ADR before merge. Placeholder README content is not acceptable. |
| Cross-platform | Path, executable, venv, PID/log, and native-library handling use platform abstractions and path APIs. Tests or smoke notes cover paths with spaces and canonical path identity where containment is security-sensitive. |
| Launcher/release | `launcher.sh --install`, build, and release-smoke paths account for ONNX sidecar dependencies with idempotent checks and isolated verification state. Release notes/changelog, dependency audit, license review, and package-size impact are recorded for the user-visible feature. |

### Assumptions

- The first ONNX serving use case is text embeddings.
- ONNX embedding packages include, or can be imported with, tokenizer files
  needed by `transformers` tokenization.
- `nomic-embed-text-v1.5` uses 768-dimensional embeddings by default and may
  support Matryoshka truncation through caller-selected `dimensions`.
- Callers provide task-prefixed input when required by the model family.
- Pumas can treat ONNX Runtime profile status like other managed/external
  runtime profiles.
- The Pumas gateway can continue to be the public `/v1` facade after its routing
  internals are replaced with provider capability checks and provider-scoped
  model routing.
- The implementation targets a managed ONNX Runtime profile lifecycle so the
  GUI can set up ONNX, save profiles, assign model routes, and serve models
  without requiring a separately started external server. External ONNX profiles
  are optional only if they fit the same clean provider contract.

### Dependencies

- New `onnx-server/` Python sidecar.
- `launcher-data/plugins/onnx-runtime.json`.
- `rust/crates/pumas-core/src/models/runtime_profile.rs`.
- `rust/crates/pumas-core/src/runtime_profiles.rs`.
- `rust/crates/pumas-core/src/api/state_runtime_profiles.rs`.
- `rust/crates/pumas-core/src/process/launcher.rs`.
- `rust/crates/pumas-core/src/serving/`.
- `rust/crates/pumas-core/src/api/serving.rs`.
- `rust/crates/pumas-rpc/src/handlers/serving.rs`.
- `rust/crates/pumas-rpc/src/handlers/mod.rs`.
- `rust/crates/pumas-rpc/src/server.rs`.
- `frontend/src/types/api-runtime-profiles.ts`.
- `frontend/src/types/api-serving.ts`.
- `frontend/src/types/plugins.ts`.
- `frontend/src/types/apps.ts`.
- `frontend/src/config/apps.ts`.
- `frontend/src/hooks/useManagedApps.ts`.
- `frontend/src/hooks/useSelectedAppVersions.ts`.
- `frontend/src/components/AppShellPanels.ts`.
- `frontend/src/components/app-panels/AppPanelRenderer.tsx`.
- New or updated ONNX Runtime app panel component.
- `frontend/src/components/ModelServeDialog.tsx`.
- `frontend/src/components/app-panels/sections/RuntimeProfileSettingsShared.ts`.
- `frontend/src/components/app-panels/sections/RuntimeProfileSettingsFields.tsx`.
- `frontend/src/components/app-panels/sections/RuntimeProfileSettingsSection.tsx`.
- `frontend/src/components/app-panels/sections/RuntimeProfileSettingsList.tsx`.
- New ONNX-compatible model library section and view-model helpers, modeled
  after the llama.cpp route-selection workflow but scoped to ONNX Runtime.
- `docs/contracts/desktop-rpc-methods.md`.
- Existing serving plans:
  - `docs/plans/local-runtime-profiles-and-ollama-version-manager/plan.md`
  - `docs/plans/llamacpp-compatible-library-profile-serving/plan.md`

### Affected Structured Contracts

- `RuntimeProviderId` gains `onnx_runtime`.
- `RuntimeProviderMode` gains `onnx_serve`.
- Runtime profile snapshots, statuses, route mutations, and frontend bridge
  types use provider-scoped route records and include ONNX Runtime provider
  values.
- Plugin metadata gains `onnx-runtime` with `.onnx` compatibility.
- Frontend model metadata types recognize ONNX as a local executable format,
  including primary format detection for imported/downloaded ONNX artifacts.
- `ServeModelRequest` validation accepts ONNX Runtime profiles for embedding
  ONNX artifacts.
- `ServedModelStatus.provider` may be `onnx_runtime`.
- `ModelRuntimeRoute` is replaced by a provider-scoped route contract keyed by
  `(provider, model_id)`. The old global route shape must not remain as a
  parallel code path after the migration/cleanup step.
- Pumas gateway provider routing maps served ONNX models to provider-side model
  names and proxies `/v1/embeddings`.
- Provider capabilities declare which OpenAI-compatible paths each provider can
  serve. ONNX Runtime starts with embeddings only.
- Served model state must retain enough information to map a public gateway
  alias to the provider-side model id used by the ONNX sidecar.
- ONNX sidecar exposes OpenAI-compatible embedding responses.

### Affected Persisted Artifacts

- `launcher-data/metadata/runtime-profiles.json` moves to a new schema version
  with provider-scoped routes and may contain ONNX Runtime profiles/routes after
  users configure them. Old global routes are migrated or discarded in a
  one-way cleanup step; the persisted file is rewritten in the new shape.
- ONNX sidecar profile runtime directories may contain PID files, logs, and
  sidecar-local config.
- Model-library records must classify `.onnx` as a first-class local executable
  format. Any existing metadata paths that only recognize `gguf` or
  `safetensors` are replaced in the same slice instead of leaving ONNX as an
  ad hoc special case.

### Process And Lifecycle Ownership

- Pumas runtime profile service owns starting, stopping, health checking, and
  status publication for managed ONNX Runtime sidecars.
- The ONNX sidecar owns loaded model sessions and unload cleanup inside its
  process.
- Pumas serving owns the durable route/default profile config and in-memory
  served-model status.
- Pumas gateway owns external request routing and error shaping.
- Frontend owns only transient form drafts and displays backend-confirmed
  snapshots/events.
- Managed ONNX profiles must use unique PID/log paths and ports. Restart must
  stop the old sidecar or mark stale state before starting a replacement.
