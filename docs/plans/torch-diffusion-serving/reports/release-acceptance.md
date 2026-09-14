# Release acceptance evidence

A6 and A7 passed within the recorded artifact scope. The sections below retain
chronological evidence; early limitations are superseded by later acceptance
where explicitly recorded. Distribution and remaining fault acceptance are
still pending in the plan.

## Historical M1 inference-disabled release

Built successfully on 2026-09-13 using:

```bash
CARGO_BUILD_JOBS=4 cargo build --manifest-path rust/Cargo.toml -p pumas-rpc --release --no-default-features --offline --target-dir launcher-data/cache/torch-qualification/disabled-target
```

Artifact SHA-256: `a1a33e7d1577f191a9a11380f0272611e70970b8272b70cad39a1ef0f5c50edc`.
Started with an isolated `disabled-root` on loopback port 18766. `/health`
returned 200; `/v1/models`, `/v1/chat/completions`, and
`/v1/images/generations` returned 404. RPC `torch_get_status`, `launch_torch`,
`launch_llama_cpp`, `get_available_versions` and `install_version` returned
JSON-RPC -32601 (method not supported). No Torch/llama runtime files were created
in that root. Raw route responses and build/startup logs are retained under
`launcher-data/cache/torch-qualification/`.

The inference-disabled frontend build succeeded. Interactive control absence has
not yet been verified. This evidence is partial A6, not full acceptance.

## Historical M1 inference-enabled release

The release build succeeded with default features. Artifact SHA-256 before the
subsequent dependency-repair correction:
`12c5c1211f8d1755e18a8a52035a969828a4614e662c04b16a45671e468dc355`.
No real image or Tuldok browser acceptance has run on it.

## M2 release artifacts and disabled desktop

Fresh enabled RPC SHA-256:
`822354ce36b3b44ef514be405fd9ebb92fe08513cf1188a492756b42cb9145fe`.
Fresh disabled RPC SHA-256:
`4e22caf1dd2a6a304374f239b2e40ceb012774c02754fe0aa71001a1eaaf276c`.

The disabled artifact ran against `disabled-m2-root` on port 18769. Health was
200, models/chat/images routes were 404, and the five inference management calls
returned -32601. Neither Torch nor llama.cpp installation directories appeared.
Raw evidence: `disabled-m2-checks.json` and `disabled-m2-release.log`.

Built `pnpm --dir frontend build:library-only` and `pnpm --dir electron build`,
then launched the actual Electron shell with `PUMAS_RPC_BINARY` pointing at this
disabled release and an isolated launcher root/user-data directory. The backend
started on port 40519. Inspected the rendered desktop and its button catalogue:
no Torch/llama tabs, runtime install/start controls or serving controls appeared.
The model-library import/search controls remained present. Evidence:
`disabled-desktop.png`, `disabled-desktop-controls.json`, `disabled-desktop.log`.
This completes the disabled portion of A6 for these artifacts. Enabled real-model
acceptance remains pending, so A6 as a whole is still pending.

## Real llama.cpp/Tuldok regression

A7 passed against the enabled artifact above. See [Tuldok evidence](tuldok.md):
real Qwen VLM, gateway, browser display, unsaved suggestion and saved annotation
provenance. The test-owned model was unloaded afterward; user profiles and source
dataset were preserved. Shared installer regression suite: 90 passed.

## Final enabled FP8 release

Release SHA-256 `b11742c95e7dc68d82abd790618a6cba9e012dbc95e7c8ad8cb60f9165b45b74`
is running in the actual desktop on the main library root, gateway
`http://127.0.0.1:38035/v1`, with managed Torch runtime 0.1.4. Both image models
load. Real Tuldok FLUX/FP8 generation/display/save/import passed in 8.843 seconds;
Nunchaku's real Tuldok pass is retained in its report. A6 is passed using this
enabled workflow and the previously qualified disabled artifact above; the
disabled artifact was not rebuilt for the subsequent conversion-only changes.
The new general FP8 conversion backend uses the existing explicit setup owner
and CPU dependencies; it adds no Torch serving/management routes outside the gate.

The real desktop FP8 conversion completed in 43.132 seconds. Focused frontend
checks: 21 existing conversion tests and the added FP8 selection case passed;
TypeScript and Electron builds passed. General CPU FP8 output reloaded in the
managed GPU runtime with finite inference, including a zero-weight-block case.
Core test targets compile. No expanded fault matrix was performed after the
user requested prioritizing the usable flow. Public runtime discovery/publication
(A1/T7) and unperformed A5 GPU fault cases remain open; full plan acceptance is
not claimed.
