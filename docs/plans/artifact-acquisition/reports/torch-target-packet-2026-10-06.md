# Approved target packet/custody successor — 2026-10-06

This narrow successor repairs two native-observation admission defects, then
binds a separately approved complete target observation into the private accepted
resolution packet, local consumer provenance and opaque receipt payload. Missing,
mismatched, changed or unsupported explicit context cannot inherit acceptance or
fall back to absent-context behavior. Current automatic/preview producers still
pass no approved context; their existing resolver and the finite recipe are
unchanged. No experimental resolver is adopted. P1 and AQ gates remain open.

## Exact source and independent repair

Branch `feat/torch-target-packet-638bb7c4` starts at frozen
`638bb7c442be11af00f88017316fc4de04c15807`, tree
`6b13d305ffefe2cb55694e85477e9fd191b60f01`. Its separately reviewable native repair
is `1c40f5bcb671a3d650270615aeeba022a15c1f8e`, tree
`de0156bc9878aefeebabcb409cd8c3a51d978daf`. The first packet milestone is
`6f68f51e716f2c4dacdaa9517ba6a409cbe5eea3`, tree
`533582dcb82c4b47c830edeb1e7b928fb6fe5ece`. Final tested packet source, including
separate managed-provider/selected-consumer executable proofs:
`24a5b5c72243f707451b3d193145c2eac0a325d0`, tree
`be22f28f60ea4b57e6d3a57cb5d44b622ca0fb14`.

The independent review hypotheses reproduce against the exact frozen helper:

- With `platform.machine() == "AMD64"` but actual selected-interpreter tags
  restricted to `win32`, frozen observation constructed a `win_amd64` target.
- With Windows `Py_DEBUG` absent, public packaging tag generation detects debug
  through `sys.gettotalrefcount` or `_d.pyd` extension suffixes. Both fixtures
  construct debug and normal ABI tags, so a subset check alone still admits the
  normal target contrary to the contract's debug-interpreter rejection.

The repair compares all constructed compatibility tags against actual public
interpreter tags and separately refuses observed debug/free-threaded ABI tags.
The two regression methods fail in three subcontrols on the frozen helper and
pass on the repair. Their local-consumer boundary also asserts no pip execution,
package staging or output creation. This confirms preflight admission defects;
it does not claim an incompatible wheel was successfully installed. The fixtures
execute public packaging generators and its Windows debug probes, not private
packaging ABI helpers. Ordinary native Linux tag parity/local installation passes.

No frozen candidate was amended. Existing resolver/catalog experiment files,
finite recipe, dependency manifests/locks, native cleanup, importer/watcher/S3
write sets and main remain unchanged. No PR/merge, external reviewer contact,
provider/account credential, paid service or security/auth/network change. The
same executor was used throughout. No applicable filesystem AGENTS/skills exist.

## Accepted packet and consumer boundary

The [target contract](../../../contracts/wheel-target.md) extends the existing
accepted packet/provenance/custody owner, not acquisition's package semantics.

`wheel_target.py --observe` / `capture_observation()` runs only when explicitly
chosen by the trusted selected-interpreter owner. It returns the strict four-field
`pumas.wheel-target-observation.v1`: schema, complete validated target, exact
interpreter path and executable SHA-256. Executable hashing streams in bounded
chunks and does not require Python 3.11's `hashlib.file_digest`. Merely possessing
this JSON does not establish approval; the caller owns its observation provenance.

An explicit packet carries the complete `wheel_target` plus
`wheel_target_observation_sha256`. The digest covers the entire observation using
recursive sorted-key compact UTF-8 JSON with unescaped Unicode, preserved arrays,
nulls and booleans. A nested Unicode vector checks Python/Rust projection parity.
`bind_resolution` adds these fields; original artifacts/URLs/hashes remain intact.

The private Rust selection accepts expected observation **separately** from the
resolution/report. Its complete fixed-shape/identity checks require exact target,
whole pip-report runtime marker environment, observation digest, selected
interpreter path and independently supplied selected-executable hash. The existing
managed-provider hash stays separate in the packet/receipt. Standards-level
compatibility remains with the trusted Python target owner. The accepted packet holds an owned
observation snapshot; `#[serde(skip)]` prevents recreating acceptance from saved
resolution JSON. A regression keeps a different provider hash unchanged while
accepting/rechecking the selected consumer proof. A retained packet is checked
again before acquisition, including refusal when its binding fields have been removed after acceptance.

For a bound packet, the existing consumer writes those approved bytes exclusively
to its owned runtime, checks the existing provenance fence before acquisition and
again before pip, passes `--target-observation` to the local consumer, and rechecks
it after the runtime probe. Final output proof adds the approved observation
digest to the existing opaque receipt payload. Shared stage and verified-input
custody remain live through child effects, output validation, publication, cleanup
and settlement. Post-install approval mutation refuses publication and preserves
`Using` inputs without a receipt; it cannot issue acceptance from new current data.

The Python CLI validates both binding fields, complete observation/target,
independently supplied approval, actual native markers/ABI/tags and executable
bytes before staging or pip. A Linux provider-to-venv fixture succeeds when the
venv path resolves to the same executable bytes/native context. Different-byte
launchers/redirectors require independently approved actual consumer observation;
the test supports that case by explicitly observing the selected venv rather than
waiving the digest. Actual Windows redirector behavior is not qualified here.

Backward compatibility is explicit: absent binding fields **and** absent approval
retain existing local consumption. Bare `wheel_target`, a single field, explicit
null, supplied approval with removed fields, unknown schema or unsupported target
refuse. Low-level declared-target Python compatibility/preflight APIs remain
available but cannot mint accepted-packet authority. Current automatic and
retained-preview producers pass `None` as expected approval, so unsolicited
target-bearing packets refuse. Production observation-producer wiring is not
activated by this slice. Finite recipe inputs/selection stay in their existing
absent-context mode. No public Rust/RPC DTO or neutral durable schema changes.

## Source/test evidence

| Check | Measured result |
| --- | --- |
| Exact frozen native-observation reproduction | 3 expected failing subcontrols across WOW64/debug-refcount/debug-suffix; no incompatible installation attempted |
| Target suite | 13 pass, including both reviewed corrections and refusal before stage/pip |
| Approved observation/consumer suite | 8 pass: actual observer, canonical digest, preserved artifact identity, changed/rebound target, missing/null/downgraded context, unsupported schema/target, interpreter bytes/path and actual selected-venv/legacy local consumption |
| Existing consumer | 18 pass |
| Existing finite catalog | 13 pass |
| Combined Python final run | 52 pass |
| Rust packet/shared handoff | 25 pass: exact packet/report/interpreter binding, separate provider identity and downgrade refusal, Unicode digest, valid bound publication/receipt/settlement, three pre-stage refusal cases, changed approval/custody/no-receipt, plus existing valid/failed/cancelled/abandoned/cold-replay/finite controls |
| Feature graphs before builds | 33 pass, including Linux/macOS/Windows S3/headless and ONNX no-download contracts |
| Scoped checks | Strict offline/locked app-manager Clippy with test-support/tests, Ruff, Rust format, four Python syntax and source/document diff checks pass |

Rust tests run serially under the shared Cargo admission lock, one build job,
debug info disabled, incremental disabled and the existing isolated /tmp target.
Source was frozen for each active build. Successful Rust/Clippy runs were
repeated only after the Python-3.10-compatible digest refinement and the focused
provider/selected-consumer identity separation. Exact timings are in adjacent logs.
All commands joined. No runtime/GPU/provider/backend or package source code was
executed: real installs use tiny generated local wheels and RECORD/report checks;
venv bootstrap has its existing separately scoped standard-library ownership.
The shared fixture's HTTP server is authorized anonymous loopback. No network
denial, real Torch, Windows/macOS/musl runtime or hosted acceptance is claimed.

[Provenance/source hashes](torch-target-packet-2026-10-06/provenance.json),
[final Python log](torch-target-packet-2026-10-06/python-final.log),
[final Rust log](torch-target-packet-2026-10-06/rust-final.log),
[Clippy log](torch-target-packet-2026-10-06/clippy.log),
[frozen reproduction](torch-target-packet-2026-10-06/frozen-native-reproduction.log)
and adjacent logs retain the exact commands, source identities and results.
The adjacent actual Linux observation and canonical bytes are illustrative
selected-interpreter evidence, not transferable approval. Their digest is
`f367501f80061ded12531df1b061b3d87eb07f9ae3b5d9e33f42479cf90df239`.

## Remaining Q2 work and catalog authority

No catalog-authority decision prevents this bounded binding slice. Production
automatic/preview adoption still needs a separately admitted observation producer
under the actual selected-interpreter custody and a qualified accepted resolution
route. It must not infer approval from saved/resolver JSON or silently recapture a
changed target to preserve acceptance. The upstream decision remains with parent:
approved index/object universe and redirects, complete bounded enumeration and
candidate-byte/time budgets, and refusal for omissions/unavailability without
online/source fallback. These facts materially gate replacing the existing P1
resolver source-preparation route. A valid target/packet binding establishes none
of that catalog authority. Existing Q2 AC11/AC12 and AQ-PACKAGES, AQ-HTTP and actual
provider/platform/installed acceptance remain open; frozen UV experiments and the
finite qualified recipe retain their separate bounded claims.
