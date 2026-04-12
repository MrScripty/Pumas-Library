# bindings/csharp

## Purpose
Checked-in smoke-only assets for validating generated Pumas C# bindings without
treating generated C# or native binaries as source-controlled artifacts.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `Pumas.NativeSmoke/Program.cs` | Minimal console harness that compiles against generated bindings and exercises a small stable Pumas call path. |
| `README.md` | Contract and usage notes for the C# smoke workflow. |

## Problem
Pumas previously generated bindings without proving that the generated C# surface
still compiled or loaded the native library. That left namespace/class drift and
binding regressions undetected until a foreign-language consumer tried to use
the library.

## Constraints
- Generated C# must remain generated-only and must not be hand-edited.
- The default smoke must be model-free and offline.
- The harness must compile without NuGet restore so it is usable in constrained
  CI or local environments.
- The checked-in files in this directory must be stable harness/docs assets, not
  generated output.

## Decision
Keep only a tiny C# harness and documentation in source control. Generate the
real binding files into `rust/target/uniffi/csharp/`, compile the harness
against those generated files, and run the result against the native Pumas
library.

## Alternatives Rejected
- Check generated `.cs` files into `bindings/csharp/`: rejected because they
  are derived artifacts and would drift.
- Rely on documentation examples only: rejected because docs do not prove the
  generated binding still compiles or loads correctly.

## Invariants
- Generated C# stays under ignored build output, not in this directory.
- The smoke harness validates real generated names from the compiled library.
- The default smoke must not depend on network access or model assets.

## Revisit Triggers
- A richer host-language acceptance workflow is needed beyond constructor and
  simple API calls.
- Pumas adds packaged quickstarts or shipping C# examples that should live next
  to this smoke harness.
- Product-facing native naming changes and the generated namespace changes with
  it.

## Dependencies
**Internal:** `scripts/check-uniffi-csharp-smoke.sh`, `pumas-uniffi`, and the
generated files under `rust/target/uniffi/csharp/`.
**External:** `.NET SDK` and `uniffi-bindgen-cs`.

## Related ADRs
- None identified as of 2026-04-12.
- Reason: This directory exists to support binding verification, not a distinct
  runtime architecture decision.
- Revisit trigger: The repo ships a maintained C# SDK or multiple host-language
  quickstarts.

## Usage Examples
```bash
./scripts/check-uniffi-csharp-smoke.sh
```

## API Consumer Contract
- Primary consumers are repo developers and CI jobs validating the generated
  Pumas C# surface.
- The harness expects generated C# files and the matching native library from
  the same build.
- The smoke fails fast if generated names, compile surface, or runtime loading
  no longer match expectations.

## Structured Producer Contract
- This directory does not publish generated bindings itself; it provides a
  stable harness for validating generated outputs produced elsewhere.
- Generated files consumed by this harness live under `rust/target/uniffi/csharp/`.
- If generated namespace/type names change, the smoke harness and root binding
  docs must be updated in the same change.
