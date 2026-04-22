# Deferred Issues Register

This register captures problems discovered during standards analysis that are not fully solved by standards compliance alone. These should become separate bugs, test tasks, or design tasks.

| ID | Area | Location | Problem | Suggested Follow-Up |
| --- | --- | --- | --- | --- |
| D01 | Electron security | `electron/src/main.ts` | `api:call` forwards arbitrary method strings and params after trusting TypeScript annotations. | Add allowlist/schema validation and tests for malformed renderer payloads. |
| D02 | Electron security | `electron/src/main.ts` | `dialog:openFile` accepts raw dialog options from renderer. | Validate allowed properties or expose narrower dedicated dialog methods. |
| D03 | RPC security | `rust/crates/pumas-rpc/src/server.rs` | CORS allows any origin/method/header. | Restrict CORS or document loopback-only threat model with tests. |
| D04 | RPC lifecycle | `rust/crates/pumas-rpc/src/server.rs` | Spawned Axum server handle is discarded; shutdown/panic handling is not owned. | Return and own a server lifecycle handle. |
| D05 | Async lifecycle | `rust/crates/pumas-core/src/api/builder.rs` | Startup recovery tasks are spawned without tracked handles. | Add task supervisor and shutdown tests. |
| D06 | Python concurrency | `torch-server/model_manager.py` | Slot registry and config state can race across concurrent load/unload/configure calls. | Add manager-level async lock and concurrent request tests. |
| D07 | Python composition | `torch-server/serve.py` | `create_app()` mutates a module-global FastAPI app. | Build a fresh app per call and test duplicate creation. |
| D08 | LAN exposure | `torch-server/control_api.py` and `serve.py` | LAN mode can bind to `0.0.0.0` without visible auth policy. | Add threat model and token/trusted-network policy. |
| D09 | Test isolation | `rust/crates/pumas-core/tests/api_tests.rs`, `rust/crates/pumas-core/src/tests.rs` | Process-global env/path overrides require serialized guards; broader suite parallel safety needs audit. | Add isolation documentation and run repeated parallel test checks. |
| D10 | Generated/runtime state | workspace tree | Runtime directories under `launcher-data/` and `rust/target/` appear locally and can pollute audits. | Add audit scripts that explicitly exclude ignored generated paths. |
| D11 | UX/a11y | `frontend/src/components/InstallDialog.tsx`, `MigrationReportsPanel.tsx` | `window.confirm` bypasses app focus and style system. | Replace with accessible app confirmation dialog. |
| D12 | Frontend tests | custom role-button/backdrop components | Keyboard/focus tests may be incomplete for custom interactive generic elements. | Add targeted a11y interaction tests before refactoring visuals. |
| D13 | Dependency ownership | `electron/package.json`, `frontend/package.json`, root `package.json` | Workspace-local scripts rely on root TypeScript/ESLint tooling. | Move command-owned deps to each workspace and verify package-local commands. |
| D14 | Release integrity | release docs/scripts | No visible checksum generation workflow for release artifacts. | Add release artifact contract and checksum generation step. |
| D15 | Performance | Rust model index/library/download/conversion paths | No Criterion benchmarks found for performance-sensitive claims. | Add representative benchmarks after decomposition stabilizes APIs. |
