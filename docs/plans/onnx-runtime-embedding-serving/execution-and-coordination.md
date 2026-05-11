# Execution And Coordination

## Execution Notes

Update during implementation:
- 2026-05-11: Split the monolithic ONNX Runtime embedding serving plan into a
  standards-compliant plan directory. `plan.md` is now the execution index;
  detailed inputs/standards, impact review, provider contracts, risks,
  milestones, and coordination notes live in separate linked Markdown files.
- 2026-05-11: Plan created from user request to add ONNX Runtime embedding
  serving for external apps, following the local coding standards plan
  structure.
- 2026-05-11: Plan reviewed against runtime profile, serving, gateway, process,
  plugin, and frontend blast radius. Added Milestone 0 for provider capability
  boundaries, provider-aware validation/unload/gateway dispatch, shared gateway
  client, and clearer frontend integration constraints.
- 2026-05-11: Plan iterated against local Coding Standards for planning,
  architecture, security, concurrency, dependencies, frontend/accessibility,
  interop, documentation, launcher, release, cross-platform, and Rust-specific
  API/async/security/tooling requirements. Added a standards compliance
  guardrail section and milestone-level verification requirements.
- 2026-05-11: Plan re-reviewed with the constraint that legacy code and
  backwards-support paths must not remain. The plan now requires provider-scoped
  routes, clean provider behavior replacement, managed ONNX lifecycle as the
  target slice, one-way runtime-profile route cleanup, and removal of old
  global-route/two-provider fallback paths.
- 2026-05-11: Plan iterated again against the local Coding Standards directory.
  Added implementation-blocking compliance gates, code-area findings, vertical
  acceptance-test requirements, validated boundary type requirements,
  lifecycle/cancellation checks, dependency ownership evidence, README/ADR
  traceability, release artifact/SBOM expectations, and standards-compliant
  parallel worker coordination rules.
- 2026-05-11: Plan reviewed against current code blast radius. Added concrete
  findings for runtime profile DTOs/service, route resolution, serving core,
  RPC serving handlers, gateway proxy, runtime launch lifecycle, model-library
  metadata, frontend app registry, runtime profile controls, model route UI,
  state synchronization, and oversized files. Added simplification and
  performance requirements so ONNX is implemented through provider behavior,
  provider-scoped identity, launch strategy extraction, shared gateway client,
  and frontend provider compatibility helpers instead of new special cases.
- 2026-05-11: Plan updated to make the cleaner provider model an explicit
  architecture deliverable. Milestone 0 is now the Provider Model Refactor and
  requires documenting shared systems, separating app/plugin identity from
  runtime provider behavior, adding a provider registry, migrating Ollama and
  llama.cpp through provider behavior/adapters first, adding provider-scoped
  routes/served identity, endpoint capability checks, managed launch strategies,
  model compatibility types, and frontend provider descriptors before ONNX
  serving is wired.
- 2026-05-11: Plan iterated again for standards compliance after the provider
  model update. Added explicit public-facade and composition-root constraints,
  executable contract ownership matrix, provider-registry lifecycle ownership,
  package-local sidecar dependency/format checks, README traceability gates,
  integration-test isolation/repeat requirements, frontend declarative
  rendering and semantic selector requirements, and additional re-plan triggers
  for standards gate failures or contract ownership ambiguity.
- 2026-05-11: Plan re-reviewed against the current code after the cleaner
  provider-model changes. Added additional blast-radius findings for hard-coded
  Rust/frontend/plugin app identity, absence of a real provider registry
  composition root, per-request/per-operation HTTP client construction, Torch
  sidecar limits as a reference pattern, gateway endpoint-specific body policy,
  the very large model-library implementation surface, and frontend
  serve-dialog state drift. Updated milestones to require app/runtime descriptor
  strategy, reusable provider clients, model-library compatibility extraction,
  endpoint-specific gateway limits, app identity drift tests, and serving-status
  subscription use before ONNX is wired.
- 2026-05-11: Implementation start hygiene check ran before code edits.
  `git status --short --untracked-files=all` found dirty implementation files
  under `rust/crates/pumas-core/src/model_library/`: `artifact_identity.rs`,
  `library.rs`, `library/migration.rs`, and `mod.rs`. The first confirmed
  slice is Milestone 0 worktree hygiene plus provider-model documentation setup.
  Per the plan gate, code implementation is paused until those dirty
  implementation files are resolved, committed, stashed, or explicitly allowed
  for this plan.
- 2026-05-11: Dirty model-library implementation files were committed in
  `2c6dea94` before ONNX implementation resumed. Focused verification for that
  pre-existing dirty slice passed: model-library migration dry-run tests and
  Rust workspace formatting via `rust/Cargo.toml`.

## Commit Cadence Notes

- Commit the sidecar skeleton and tests as the first verified slice.
- Commit Rust provider/profile contracts separately from frontend UI when
  feasible.
- Commit gateway routing with Rust tests before release validation.
- Keep code, tests, and documentation together when they describe one completed
  behavior.
- Follow `COMMIT-STANDARDS.md`.

## Optional Parallel Worker Plan

Use only if implementation is parallelized.

Parallel work is allowed only after Milestone 0 freezes the shared contracts and
the integration branch is clean. Shared contracts, persisted schemas, plugin
metadata, route DTOs, lockfiles, launcher behavior, and ADRs are serial
integration files unless one explicit owner is assigned for the current wave.

| Owner/Agent | Primary Write Set | Allowed Adjacent Write Set | Forbidden/Shared Files | Output Contract | Handoff Checkpoint |
| ----------- | ----------------- | -------------------------- | ---------------------- | --------------- | ------------------ |
| Sidecar worker | `onnx-server/` | Sidecar README and sidecar-local dependency manifest/lock files | Rust DTOs, frontend types, root/workspace dependency manifests unless explicitly assigned | Python sidecar, validation, fake and real-session tests, README | Sidecar tests pass, dependency ownership evidence recorded, endpoint contract documented. |
| Rust worker | `rust/crates/pumas-core/`, `rust/crates/pumas-rpc/` | `launcher-data/plugins/onnx-runtime.json`, Rust docs/README updates when assigned | Frontend components, Python sidecar internals, lockfiles not owned by Rust slice | Provider contracts, route migration/cleanup, serving, gateway tests | Rust focused tests pass, serialization/migration evidence recorded, no old route shape active. |
| Frontend worker | `frontend/src/` | Electron bridge/types only when required by the frozen contract | Rust DTOs, sidecar internals, plugin metadata unless explicitly assigned | ONNX app icon/panel/profile/model-route UI and tests | Typecheck/build/focused frontend tests pass, no optimistic backend-owned state introduced. |
| Integration owner | Plan, ADR, cross-layer docs, release notes, shared schema/manifest files | Coordination reports and final verification notes | None; this owner serializes cross-cutting edits | Contract sync, docs, release evidence, final verification | Full vertical acceptance path passes and worker outputs match assigned write sets. |

Worker reports must be written under this plan directory if workers are used:
`docs/plans/onnx-runtime-embedding-serving/reports/<worker>-<date>.md`.
Each report must list changed files, tests run, skipped checks, contract
assumptions, and any needed out-of-scope edits. Integrate one worker branch at a
time, verify after each integration, and clean up worker workspaces only after
their commits are reachable from the integration branch and no uncommitted
changes remain.

## Re-Plan Triggers

- The available ONNX model package does not include enough tokenizer/config
  files for local tokenization.
- `nomic-embed-text-v1.5` ONNX exports require model-specific custom ops or
  output handling that cannot be represented by a generic embedding sidecar.
- ONNX Runtime GPU packaging differs enough by platform to require separate CPU
  and GPU plugin/runtime profiles.
- The Pumas gateway cannot safely route embedding-only providers without the new
  provider capability model fully replacing path/provider dispatch.
- The provider capability boundary grows enough to require a separate runtime
  provider registry refactor.
- Any standards compliance gate cannot be satisfied in the current architecture
  without expanding the blast radius beyond the affected systems named in this
  plan.
- A boundary contract cannot name one owner, runtime validator/decoder,
  producer test, consumer test, and persisted compatibility policy.
- New extracted modules, source directories, or provider descriptors cannot
  stay within standards file-size/responsibility thresholds without a broader
  decomposition plan.
- The frontend generic app panel cannot support runtime profiles without
  duplicating provider-specific panel behavior.
- The hard-coded app registry, managed-app decoration, selected-version state,
  or app-panel renderer cannot represent ONNX Runtime cleanly without replacing
  the app registry approach.
- Rust `AppId`, plugin metadata, version-manager registration, and frontend app
  descriptors cannot be kept in sync with focused drift tests.
- Reusable provider clients cannot be injected through the provider registry or
  gateway composition root without a broader RPC server state refactor.
- Provider-scoped model routes reveal a broader route/default-profile redesign
  is required before ONNX route assignment can be implemented cleanly.
- Dependency evaluation finds ONNX Runtime packaging, transitive dependency
  cost, license, or CPU/GPU split is not acceptable for sidecar-local ownership.
- Required lifecycle/concurrency guarantees require a broader process manager
  refactor than this feature can safely include.
- Cross-platform launch or path handling cannot be expressed through existing
  launcher/process abstractions without scattering platform checks.
- Required frontend accessibility or event-driven state constraints conflict
  with the current app-panel architecture.
- Runtime-profile schema migration/cleanup cannot remove the old global route
  shape without unacceptable data loss.
- External apps require LAN access or authentication behavior beyond the
  existing loopback-first gateway policy.
- Emily needs a different embedding dimension than the served model provides,
  implying a memory schema migration.

## Recommendations

- Recommendation 1: Keep ONNX Runtime separate from llama.cpp. Both can expose
  OpenAI-compatible endpoints, but they own different artifact formats,
  lifecycle behavior, and dependency footprints.
- Recommendation 2: Prefer pointing external apps at the Pumas gateway instead
  of raw provider endpoints. This keeps aliases, served state, and future auth
  policy in one place.
- Recommendation 3: Keep the first slice embedding-only. Add reranking or other
  ONNX tasks later behind explicit provider capability flags.
- Recommendation 4: Do Milestone 0 before sidecar integration. It reduces the
  risk that ONNX support cements current Ollama-vs-llama.cpp assumptions.
- Recommendation 5: Keep the first complete vertical slice managed-sidecar
  first because the expected UX is setup, profile save, model route assignment,
  and serving from the ONNX app panel.
- Recommendation 6: Treat the ONNX model library panel as a provider-specific
  sibling of `LlamaCppModelLibrarySection`, not as a generic `ModelManager`
  variant. The user workflow is route/profile assignment plus serving, which is
  closer to llama.cpp than to the generic model download/library surface.

## Completion Summary

### Completed

- Initial implementation hygiene check completed. Pre-existing dirty
  model-library implementation files were committed separately before ONNX
  implementation resumed.

### Deviations

- None.

### Follow-Ups

- Any implementation-time standards deviation must be recorded here with an
  owner, mitigation, and revisit trigger.

### Verification Summary

- Pre-existing model-library dirty slice verified before commit with focused
  migration dry-run tests and Rust workspace formatting. ONNX plan verification
  has not started yet.

### Traceability Links

- Module README updated: pending.
- ADR added/updated: pending. Provider-scoped routes and provider behavior
  replace existing runtime-provider architecture enough to warrant a durable
  decision record during implementation.
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: pending.
