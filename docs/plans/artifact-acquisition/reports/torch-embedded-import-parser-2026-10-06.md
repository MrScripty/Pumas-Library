# Restored installed-runtime import qualification — 2026-10-06

Both installed-module import controls now execute their real isolated Python
subprocesses and pass. Full discovery passes316/316, with no errors or skips.
This closes the concrete parser gap introduced by lifecycle568a5f1; it does not
implement or qualify the proposed native uv mode.

## Source identity and scope

The separate source branch is `fix/torch-embedded-import-parser-9310c1d6`:

- Source: `6699ffc89423557f45a596295c3f89698d5d342b`.
- Tree: `6f69d66f38a7f466e1e03003b97e15e614f28d01`.
- Parent: `9310c1d6a0dbbef6e602a2a9c464643e21f2d341`, tree
  `93c3ac702f5f17694f0062b94df2216430a7bfda`.
- Only changed file: [test_embedded_runtime_imports.py](https://github.com/MrScripty/Pumas-Library/blob/6699ffc89423557f45a596295c3f89698d5d342b/torch-server/tests/test_embedded_runtime_imports.py).
- Test blob: `0b18c156cc95dd9797e58dcf13c723c61b463205`.

The tests now extract the `vec!` table inside `embedded_torch_runtime_files()`
and assert that the installer writer calls that function. They retain the
independent recognized-Python-row comparison and required/excluded-module checks.
All isolated interpreter, installed-module-path, speech-owner and capability
assertions are unchanged. The negative control still deliberately offers the
repository on PYTHONPATH, removes speech_binding.py from the installed tree and
requires its import to fail. No Rust/application/runtime source, dependency pin,
catalog receipt, native projection, adoption flag or public API changes exist.

## Actual probes and gates

[Actual subprocess evidence](torch-embedded-import-parser-2026-10-06/actual-import-probes.json)
records both original test IDs, staged files, return codes and child output:

- Positive:25 staged Python files; exit0;
  `EMBEDDED_IMAGE_TEXT_IMPORT_CLOSURE_OK`. The child's existing assertions check
  imported local modules are inside the temporary installed tree.
- Missing binding:24 staged Python files; exit1;
  `ModuleNotFoundError: No module named 'speech_binding'` despite the offered
  repository Python path. The positive sentinel is absent.

The recording wrapper calls the original subprocess method and returns its
unaltered result, then runs the original test assertions. It substitutes no
execution or import result. Full discovery independently runs the tests without
this wrapper, and the focused unwrapped two-test run also passes.

| Gate | Result |
| --- | --- |
| Two actual isolated import probes | 2 pass |
| Affected Q2 Python controls | 126 pass,0 skips |
| Full torch-server Python discovery | 316 pass,0 failures/errors/skips |
| Rust embedded-runtime materialization/tooling controls | 2 pass,0 ignored |
| Full default Rust workspace, excluding pumas_rustler linking | Pass; app-manager332/library1793/RPC298 unit controls plus integration/doc groups |
| Strict workspace Clippy, all targets/features | Pass, warnings denied |
| Full torch-server Ruff | Pass |
| Rustfmt | Pass |
| Full Cargo feature graph inspection | 33 pass; ORT dynamic-only, no build download features |
| Tested-source unchanged after gates | Pass |

Ordinary Rust workspace ignored native/model/qualification controls remain
ignored and are explicitly reported in the log. Prior selected-lifecycle and
reconciliation interruption qualification is retained; those long ignored
fixtures were not rerun for this test-parser-only change. Cargo ran locked,
offline, jobs2, debug0 and incremental0, with defensive ORT_SKIP_DOWNLOAD=1 and
task-local XDG configuration. No ORT, provider or model download occurred.

The [Python runner](torch-embedded-import-parser-2026-10-06/run-python-gates.sh)
and [Rust runner](torch-embedded-import-parser-2026-10-06/run-rust-gates.sh)
record exact commands. [Qualification bindings](torch-embedded-import-parser-2026-10-06/qualification.json)
bind source/tree/test bytes and every gate status; adjacent logs provide complete
outcomes. [Test environment](torch-embedded-import-parser-2026-10-06/test-environment.json)
records unchanged tool/interpreter hashes and existing dependency versions.
A task-local venv uses .pth entries for the previously qualified dependency target
and existing interpreter site-packages, so -I import children can find installed
third-party dependencies. Those entries contain no repository path. No package
installation, pin change, global Python/Git setting or credentials change was
needed.

## Preserved failures and boundaries

Original failures remain byte-identical in diagnostic commit
`c248d28cc4577861be9eba5c5da0e292def61a18`, tree
`4ab0eb403675fd289330f66d0aa1001ee64ce606`, and prior reconciliation evidence.
The [baseline/frozen identities and failing logs](torch-native-glibc241-diagnostic-2026-10-06/parser-error-identities.json)
record the exact test blob, installer blobs, causal lifecycle commit and tracebacks.
Earlier reports retain their historically accurate314-pass/two-error outcomes.

The [native diagnostic proposal](torch-native-glibc241-diagnostic-2026-10-06.md)
remains pending evidence review. Native mode would omit BOTH target overrides,
keep exact --python, require exact approved/native target AND executable equality
before/after resolution, preserve explicit mode/refusals, and bind caller-selected
mode/observations/argv without altering settled catalog receipts. No automatic
fallback or adoption is enabled. Real-provider/native-runtime/network-denial and
power-loss qualifications are unchanged. Main and the approved reconciliation
head remain untouched; no PR, CodeRabbit request, main merge, Q3 or old S3 work
is included.
