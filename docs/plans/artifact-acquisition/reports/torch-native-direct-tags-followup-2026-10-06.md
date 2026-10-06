# Four direct-native-tags diagnostic controls — 2026-10-06

The four requested controls now exercise public uv's direct selected-interpreter
tags and native markers: BOTH `--python-platform` and `--python-version` are
absent, while `--python` retains the exact absolute interpreter path. They produce
the expected two refusals and two acceptances with exact local wheel hashes.
This is a diagnostic successor, not a native-mode implementation or acceptance.

## Prior evidence and corrected labels

Parent evidence commit: `d5317a2e218b79b102b1a55db5b9053724be7e43`, tree
`138111c43300d6a68170c06efe6d7d81708d97f9`.
The original thirteen-control packet and all parser qualification/failing evidence
are retained byte-identically. The original files and scripts have not been relabeled
or rewritten. [Label corrections and pair bindings](torch-native-direct-tags-followup-2026-10-06/prior-control-labels.json)
supersede the native-path interpretation of these four original identifiers:

| Original identifier | Precise original meaning | Direct-tags successor | Expected/actual exit |
| --- | --- | --- | ---: |
| native-floor42-refusal | Interpreter platform, full patch override; reconstructed tags; higher-floor refusal | direct-native-higher-floor-refusal | 1/1 |
| native-wrongabi-refusal | Interpreter platform, full patch override; reconstructed tags; Python/ABI refusal | direct-native-python-abi-refusal | 1/1 |
| native-floor40 | Interpreter platform, full patch override; reconstructed tags; lower-floor acceptance | direct-native-lower-floor | 0/0 |
| native-linux-tag | Interpreter platform, full patch override; reconstructed tags; Linux-tag acceptance | direct-native-linux-tag | 0/0 |

Each original supplied `--python-version3.12.14`. In the
[official pinned0.12.23 `pip/mod.rs`](https://raw.githubusercontent.com/astral-sh/uv/0.12.23/crates/uv/src/commands/pip/mod.rs),
`resolution_tags(None, Some(version))` reconstructs tags with `is_cross: true`.
Only `(None, None)` returns `interpreter.tags()` directly and uses native markers.
The original comparison outcomes remain valid, but did not qualify that direct
path. This report corrects that interpretation without changing historical bytes.

The already retained `native-floor41-no-overrides` positive closure and
`native-missing-interpreter` refusal genuinely omit both flags and are not repeated.
The overall packet now has thirteen original controls plus exactly four new runs;
six controls in total cover the no-overrides interface. No unrelated gate or
runtime fixture was rerun for this correction.

## Exact identities, inputs and results

- uv:0.12.23 Linux x86_64; SHA-256
  `abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f`.
- Exact selected interpreter:
  `/opt/codex/runtimes/codex-primary-runtime/dependencies/python/bin/python`;
  SHA-256 `fa67443527ed9647f760d807e2a38f26340757123e643c4639cf273ed15d5ea7`;
  ordinary CPython3.12.14, native glibc2.41.
- Frozen reconciliation commit:
  `9310c1d6a0dbbef6e602a2a9c464643e21f2d341`, tree
  `93c3ac702f5f17694f0062b94df2216430a7bfda`.
- The seven existing inert wheel ZIPs and each roots.in are copied byte-for-byte
  from the original committed packet after verification against both its manifest
  and exact Git blobs. No wheel regeneration, dependency/pin change or installation
  occurs. Tagged fixtures contain inert Python, not native binaries.

[Successor summary and bindings](torch-native-direct-tags-followup-2026-10-06/run/summary.json)
record original invocation digests, new argv/roots digests, all seven wheel byte
identities, output package identities and per-case results. Each case directory
retains invocation, roots, stdout, stderr, trace, native observations before/after,
and successful genuine public pylock output.

The higher-floor fixture is `cp312-cp312-manylinux_2_42_x86_64`; uv refuses with
no solution and no pylock. The Python/ABI mismatch fixture is
`cp313-cp313-manylinux_2_41_x86_64`; uv likewise refuses. The latter is a combined
Python/ABI mismatch control, not an independently isolated ABI-only claim.
Lower-floor selection returns only native-floor40, with wheel SHA-256
`a8ccbdec4aaa9880ef6f69ad7849db8ab1ddd72f7106d65781f9938a5ce4eca8`.
Linux-tag selection returns only native-linux, with wheel SHA-256
`21ba538fbe3bf23324c418e579a9d7586e2468523759fb4a12bc4e18b2502522`.
Both successful pylocks identify local file URLs and matching original wheel hashes.

Each run preserves no-config/no-cache/offline/no-index/no-build/no-sources,
disabled keyring and disabled Python-download/managed-Python selection flags.
Only five non-secret environment fields are supplied: PATH, LANG, task-local
XDG_CONFIG_HOME, UV_CACHE_DIR and TMPDIR. No HOME, network or credential setting
is changed. All four syscall traces observe zero AF_INET/AF_INET6 operations;
this remains observation, not enforced network-denial qualification.

Fresh observations are captured through the exact selected interpreter before
and after each compile. Each equals the original diagnostic observation in full,
including target and executable identity; executable and uv hashes are rechecked.
These data comparisons do not create live owner capabilities or reconstruct
catalog authority from saved evidence.

## Reproduction and remaining boundary

[rerun-four.py](torch-native-direct-tags-followup-2026-10-06/rerun-four.py) performs
only these four compiles. In the original environment, with a fresh output directory:

```bash
python -B docs/plans/artifact-acquisition/reports/torch-native-direct-tags-followup-2026-10-06/rerun-four.py /workspace/Pumas-Library /tmp/pumas-direct-tags-four
```

The [manifest](torch-native-direct-tags-followup-2026-10-06/sha256-manifest.json)
binds this report and all new retained evidence. The prior manifests remain valid.
The successor contains only documentation/evidence additions; the approved
reconciliation and separate parser-fix source branches remain unchanged.

Native runtime implementation still waits for independent evidence review. Its
proposal must omit both overrides, retain exact --python, require fresh owned
observations equal to the originally approved target AND executable before/after
resolution, and bind caller-selected mode, observations and argv without rewriting
settled catalog receipts. Preserve explicit mode/refusals with no automatic fallback
or retry. Automatic/preview adoption remains OFF. This diagnostic qualifies no
installation, native-binary execution, real provider, power-loss behavior or
enforced network denial. No model, new credential, Q3, old S3, Library transfer,
main merge, PR or CodeRabbit work is included.
