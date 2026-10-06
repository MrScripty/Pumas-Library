# Native glibc2.41 public-tool diagnostic — 2026-10-06

This evidence-only branch preserves reconciliation commit
`9310c1d6a0dbbef6e602a2a9c464643e21f2d341`, tree
`93c3ac702f5f17694f0062b94df2216430a7bfda`. No application/runtime source,
catalog receipt, version pin or caller adoption changes are included. The native
mode below is a proposal under independent review, not an implementation or
repository-path qualification. No package installation or model download occurred.

## Exact tool and blocker

The existing qualified public binary is uv0.12.23 Linux x86_64, SHA-256
`abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f`.
The selected interpreter is CPython3.12.14, SHA-256
`fa67443527ed9647f760d807e2a38f26340757123e643c4639cf273ed15d5ea7`.
Its unchanged native observation reports glibc2.41.

The pinned public CLI accepts explicit x86_64 glibc floors2.17,2.28 and2.31–2.40.
Neither `x86_64-manylinux_2_41` nor `manylinux_2_41_x86_64` parses.
The `linux` and `x86_64-unknown-linux-gnu` aliases select glibc2.28; they do not
mean native-host libc. The [official0.12.23 TargetTriple source](https://raw.githubusercontent.com/astral-sh/uv/0.12.23/crates/uv-configuration/src/target_triple.rs)
defines the finite enum and the alias mapping in `TargetTriple::platform()`.

[wheel-target contract](../../../contracts/wheel-target.md) requires exact
representability. `torch-server/offline_wheel_selection.py:resolver_platform`
admits only the qualified explicit floors and refuses2.41 before uv invocation.
The Rust private adapter validates an explicit platform string and always passes
`--python-platform`. Lowering the approved observation to2.40 or substituting
generic Linux would change the target and is prohibited.

The documented default [platform-specific pip resolution](https://docs.astral.sh/uv/concepts/resolution/#platform-specific-resolution)
and [`--python` interpreter selection](https://docs.astral.sh/uv/reference/cli/#uv-pip-compile--python)
provide a potential public-interface solution without changing this tool pin.
In [pinned `pip/mod.rs`](https://raw.githubusercontent.com/astral-sh/uv/0.12.23/crates/uv/src/commands/pip/mod.rs),
`resolution_tags` uses the selected interpreter platform when `--python-platform`
is absent. With neither target override it uses `interpreter.tags()` directly;
`resolution_markers` likewise uses interpreter markers. Retaining
`--python-version` reconstructs tags with `is_cross: true`, even though it keeps
the interpreter platform. The native proposal must therefore omit BOTH
`--python-platform` and `--python-version`, keeping exact `--python /absolute/path`.
The full-patch run below is a comparison control, not the proposed native mode.
[Pinned `pip/compile.rs`](https://raw.githubusercontent.com/astral-sh/uv/0.12.23/crates/uv/src/commands/pip/compile.rs)
calls those functions for its non-universal resolution.

The shell GitHub API tree lookup received a proxy403 and was stopped. No upstream
tag commit SHA was independently obtained. References are official version-tagged
source plus unversioned documentation, with that distinction recorded in
[source references](torch-native-glibc241-diagnostic-2026-10-06/source-references.json).

## Thirteen public CLI controls

All controls use new inert wheels generated with the existing local-fixture
builder. Platform-tagged wheels contain inert Python, not native binaries. They
qualify resolver tag decisions only. Each invocation uses the exact existing uv
and selected Python, no configuration/cache/index/source build, offline mode,
disabled keyring and disabled Python downloads/managed-Python selection. Five
allowlisted environment fields contain only local task paths and locale/PATH.

| Control | Exit | Observed result |
| --- | ---: | --- |
| Interpreter platform, full patch argument | 0 | Exact closure; reconstructed cross-target tags, comparison only |
| Native2.41, no target overrides | 0 | Same exact four-package closure |
| Explicit2.41 triple | 2 | CLI parsing refusal |
| Wheel-style2.41 triple | 2 | CLI parsing refusal |
| Explicit2.40 against2.41 wheel | 1 | No compatible solution |
| Generic Linux against2.41 wheel | 1 | No compatible solution |
| Generic GNU triple against2.41 wheel | 1 | No compatible solution |
| Native against2.42 wheel | 1 | No compatible solution |
| Native against CPython3.13 wheel | 1 | No compatible solution; Python/ABI mismatch |
| Native against2.40 wheel | 0 | Lower compatible floor |
| Explicit2.40 against2.40 wheel | 0 | Existing explicit projection positive control |
| Native against linux_x86_64 wheel | 0 | Native Linux tag admitted |
| Native with missing selected interpreter | 2 | No interpreter; no fallback/output |

[Summary](torch-native-glibc241-diagnostic-2026-10-06/summary.json), per-control
invocation/stdout/stderr/pylock/trace files, and seven exact wheel ZIPs are retained
in the adjacent evidence directory. Successful lock wheel hashes were checked
against local bytes. All thirteen traces observed zero IP socket calls. This is
observation, not enforced network-denial acceptance.

[Original manifest](torch-native-glibc241-diagnostic-2026-10-06/sha256-manifest.json)
SHA-256: `e34a189a233f53d2cabd14ebdab418aeb5c41b75434448090e2f2f4ba9d01963`.
[Publication bindings](torch-native-glibc241-diagnostic-2026-10-06/publication-bindings.json)
record byte-identical hashes for all83 original evidence files. No credentials,
headers, signed URLs, models, tool binaries or provider payloads are published.
Original paths are retained because they are non-secret evidence provenance.

The original `diagnose.py` generated twelve controls; the thirteenth was run
separately, with its exact invocation retained. `reproduce.py` combines all
thirteen without modifying original evidence. From the repository root, with
the original interpreter and existing qualified binary:

```bash
python -B docs/plans/artifact-acquisition/reports/torch-native-glibc241-diagnostic-2026-10-06/reproduce.py /tmp/pumas-native-reproduction /workspace/pumas-reconcile-tools/bin/uv
```

The output directory must not already exist. This runner only creates local
fixtures and invokes public compile; it cannot construct catalog custody or
install a selected runtime.

## Proposal and bounded qualification

1. Recommended for separate review: add a private native mode using exact
   `--python /absolute/path` and neither target override (`--python-platform`
   nor `--python-version`). Require fresh owned observations to equal the
   originally approved target AND executable before and after resolution.
   The existing subset-based `require_native_consumer()` is insufficient.
   A trusted caller chooses explicit/native mode before resolution; bind mode,
   observations and exact argv into selected provenance without rewriting settled
   catalog receipts. Retain every current catalog, executable, marker and selected
   wheel check. Do not accept a synthetic2.40 declaration as native2.41 or expand
   admitted marker policies. Preserve explicit mode and all of its refusals;
   never automatically fall back or retry from explicit into native mode.
   No public Rust API expansion is proposed. Runtime implementation waits for
   independent evidence review.
2. Keep the current explicit-only refusal until that implementation is reviewed
   and qualified. This remains safe with the existing pin.
3. Alternatively seek upstream explicit2.41 support, then review and qualify a
   future tool pin separately. No version change is part of this diagnostic.

Bound a future qualification to Linux x86_64, glibc2.41, ordinary CPython3.12.14.
Use fresh live CompleteCatalog fixtures, preserve exact request/catalog receipts,
and exercise supported/lower/higher floors, native Linux tags, Python/ABI mismatch,
extras/patch markers, missing or changed interpreter/observation/evidence and
interrupted ownership. Require the existing independent selected checker and
pre/post executable/runtime fences, then affected Q2 and full required gates with
the existing dynamic-only ORT feature constraints. No saved JSON becomes authority.
Automatic/preview adoption remains OFF. Real-provider acceptance, native runtime
binary execution, enforced network denial, other platforms and power-loss durability
remain outside this evidence. Q3 and old S3 work are unchanged.

## Two inherited parser errors

Both full test IDs are recorded in
[parser identities](torch-native-glibc241-diagnostic-2026-10-06/parser-error-identities.json).
They are the installed image/text startup import-closure control and the missing
speech-binding/no-repository-fallback control in `EmbeddedRuntimeImportTests`.
Both reproduce on base `04fde63b151fa019b73fcf71aca636b46a35c548`, tree
`910a9abef17f94af4a4f88ee14312ea0e84299dc`, and frozen reconciliation head9310c1d6.
Original failing logs remain byte-identical in this packet and in prior evidence.

The unchanged test blob is `b0b896b628be80f956acf919591347f319d62f67`.
Base installer blob is `98b5b03354015188a2d5c9bfa7686ec3e4925cb5`;
frozen installer blob is `31e22607ecbd7840f791a84308f67a352d1e2785`.
Lifecycle commit `568a5f1fca76b2253088d97f71b1d950e5beacc9` moved the inline
embedding table into `embedded_torch_runtime_files()`. The test parser at line63
still splits on `for (name, contents) in [`, raises IndexError during staging,
and prevents both actual isolated import probes from executing. The parser's last
change is `6320f938bbb27b387722c7b21f717f6d97bec3f1`. This is a concrete prior
qualification gap, now assigned a separate source correction; this diagnostic
commit intentionally retains the failing source/evidence.
