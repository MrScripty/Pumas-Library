# AC10 acquisition resource measurement — 2026-10-05

## Scope and acceptance contract

This independently authorized slice adds an opt-in measurement through the
existing public `AcquisitionService::with_capacity` and consumer `acquire_http`
APIs. It does not change production acquisition, persistence, retries, buffers,
credentials, runtime provisioning, or Cargo features. Its base is ONNX formatter
successor `26a84e323cae566a46a8f76bef48fa1010aed48b`, whose sole change over frozen
portability checkpoint `a94fd92021f27fdeedb6e2de6e01c41c250ef576` is Ruff formatting
of the feature guard with an identical Python AST. Frozen S3 head
`493b935c6a41d4d4aeca8e8a66f10b4aba114365` is preserved independently.

| Candidate identity | Exact value |
| --- | --- |
| Branch | `qualification/acquisition-resources-26a84e32` |
| Base commit | `26a84e323cae566a46a8f76bef48fa1010aed48b` |
| Base tree | `3ee66988eb1668188011b2124890b10031403ebd` |
| Qualified implementation commit | `7535927163ef84bba33e9edd5e2a150e1b7a1ef4` |
| Qualified implementation tree | `1b17f662df88c49098a5ab6ca0e4ad7ee28152c8` |

Existing finite admission limits and small saturation tests do not measure a
large streaming workload. This probe supplies that missing evidence without a
new production observer/downloader or a costly default CI workload. Retain it
while AC10 needs this public-owner measurement; revise or replace it when an
authoritative production workload/measurement subsumes this evidence.

Each fresh process streams synthetic, independently SHA-selected ordinary files
from its controlled loopback source, verifies them through the real acquisition
owner, reads the verified descriptors in its consumer callback, and settles
canonical receipts. The fixture holds one response per configured worker after
64 KiB so an excess demand can test admission before source release. After
drainage, partial-to-final inode identity and the final selected file footprint
prove the ordinary-file representation in this acquisition-owned workspace.
There is no model importer, extraction, GPU, real provider or native SDK in this
probe. It uses credential-free HTTP to its explicit loopback source only.

The fixed matrix compares 8 MiB and 128 MiB per artifact at one and two workers.
A fifth case deliberately retains both 128 MiB payloads in memory to check that
the RSS measurement can see a full-payload allocation. Default three repeats
give within-host min/median/max; this is not a statistical production guarantee.
Per-process inputs are bounded to 1–256 MiB, 1–4 workers and 512 MiB total.
Only temporary children of the explicitly selected measurement root are used.

Acceptance checks are defined before qualification:

1. Every source payload byte is accounted for, selected size/SHA matches, and
   canonical adopted records retain the exact verified completion receipts.
2. With all workers held, an excess demand returns the existing typed `workers`
   capacity refusal, preserves inventory, opens no extra source request and
   writes no excess-demand payload. Installed work then completes and drains.
3. Each final payload has the same inode as its held staging file, and the
   workspace contains exactly one payload inode per selected artifact. This
   does not qualify a downstream importer copying/extracting fallback.
4. All Linux counters and exactly one versioned measurement result are present.
   A zero-test execution is rejected. The deliberately resident allocation
   appears in process peak RSS at least at its known allocated payload size.
5. Samples and configurations remain distinct. No RAM/throughput budget is
   invented from worker counts, source chunk size or one successful sample.

## What the numbers mean

| Quantity | Evidence and limits |
| --- | --- |
| `VmHWM` | Kernel process peak RSS in bytes; includes test harness, controlled source, acquisition and consumer. Baseline is captured after setup; growth from baseline is derived, not a system/cgroup memory measurement. |
| `VmRSS` | Resident process snapshot at the named observation point; not a peak. |
| Process `rchar`/`wchar` | Linux character-I/O accounting, including verification and instrumentation; not network wire traffic or payload-only I/O. |
| Process `read_bytes`/`write_bytes` | Linux storage-I/O accounting; filesystem/page-cache dependent, not physical device traffic or a universal write-amplification bound. |
| Payload logical/allocated bytes | Inode metadata (`len`, `st_blocks * 512`) sampled every 25 ms and at completion. Renamed aliases are deduplicated by device/inode. This is sampled peak payload footprint, not exact peak disk use; store metadata and filesystem-wide usage are excluded. |
| Source bytes written | Successful source application writes, with payload and complete HTTP response bytes separated. TCP/IP overhead/retransmission/wire bytes are unmeasured. |
| Progress step | Largest observed increment reported to the host callback; not an internal buffer allocation limit. |
| Elapsed time | Instrumented operation duration including SHA/consumer work and control allocation. Debug/test builds are not release performance evidence. |
| Capacity/chunk/deadline | Configured worker/blocking budgets, 64 KiB source chunks and a 300-second fixture deadline. Hash concurrency, queued stream bytes and internal buffer peaks remain unmeasured. |

The Rust fixture has owned connection tasks and a joined blocking filesystem
sampler. The runner uses a fresh process for every row; on timeout it kills and
waits for that owned child. The source and acquisition live in that child,
without external service processes. No ambient credential/configuration dump
or authorization headers enter the result.

## Reproduction and qualification status

Compile the integration executable explicitly, then use its exact path:

```bash
cargo test --locked --offline --manifest-path rust/Cargo.toml \
  -p pumas-library --no-default-features --test acquisition_resources \
  --no-run --message-format=json
python scripts/release/measure-acquisition-resources.py \
  --binary /absolute/path/to/acquisition_resources-TEST_HASH \
  --root /existing/measurement/filesystem/directory \
  --output /existing/evidence/directory/resources.json --repeats 3
```

The runner does not build, download or provision anything. No-default compilation
is sufficient for this common acquisition owner; it does not weaken the inherited
no-build-download invariant for ONNX-enabled graphs.

All 15 fresh-process cases passed on Linux x86_64, kernel 6.18.44, 4096-byte pages,
with an explicit workspace filesystem root (device 27). Rust/Cargo 1.92.0 built
the no-default integration executable in the test profile, with debug symbols
and incremental compilation disabled; optimization remained the default test
setting. These are debug/test measurements, not shipping release performance.

| Workload | Payload total | Peak RSS minimum / median / maximum (MiB) |
| --- | --- | --- |
| 8 MiB, one worker | 8 MiB | 20.37 / 20.59 / 20.61 |
| 8 MiB, two workers | 16 MiB | 21.20 / 21.37 / 21.67 |
| 128 MiB, one worker | 128 MiB | 20.48 / 20.60 / 20.64 |
| 128 MiB, two workers | 256 MiB | 22.11 / 22.26 / 22.39 |
| 128 MiB, two workers, retained allocation control | 256 MiB | 277.74 / 278.14 / 278.30 |

All normal two-worker large cases wrote exactly 268,435,456 source payload bytes
and 268,435,632 HTTP response application bytes. They retained two final payload
inodes with exactly 268,435,456 logical/allocated bytes. Sampled allocated peaks
were 268,439,552–268,443,648 bytes; sampling and filesystem allocation are not
exact peak disk guarantees. Median Linux `write_bytes` growth was 268,505,088
bytes, including the process's metadata/fixture work, not physical device traffic.
Across the matrix the largest progress increment was 679,936 bytes, despite
64 KiB producer chunks; chunk configuration must not be advertised as a
measured internal transport-buffer bound.

The data supports streaming without a resident full payload for these exact
operations, and ordinary same-inode workspace publication. It supplies no
universal memory cap, throughput budget, importer-copy fallback qualification,
hash/queue-buffer envelope or native/platform/provider acceptance.

Strict scoped no-default Clippy (`-D warnings`), Cargo formatting, both exact
Ruff 0.15.2 commands over `torch-server scripts/release` (54 files), 10 guard tests
and 21 cross-target dependency graphs passed. The runner rejects a zero-test
executable. Zero-size input fails before fixture creation. Every case's
parent-owned temporary directory was removed, leaving the matrix root empty.
Early observer/runner failures during development are diagnosis only; the final
matrix and controls qualify the corrected fixture/runner.

The evidence directory is `/workspace/scratch/acquisition-resources/`.
`resources.json` retains all samples and configured/measured/unmeasured fields;
`verification.json` binds candidate identity, commands, profile, binary/source
hashes and logs. `runner-controls.json` records the negative controls. The runner
uses no downloader/build operation; Cargo features/dependencies are unchanged,
and the inherited 21-graph no-ONNX-download guard remains passing.

The only implementation additions are an ignored Linux integration probe, its
explicit runner and an on-demand verification-inventory row. There is no public
production API, DTO/schema, Cargo dependency, feature or persistence change.
Parent coordination owns PRs, external review, merge and downstream pins; this
branch is independent of PR41 and includes its formatter fix only as ancestry.

AC10 and acquisition gates remain pending until the remaining representative
model/filesystem/platform and full resource-envelope claims are satisfied.
The next AC10 acceptance input is a named production-scale workload and
authoritative resource budget on its intended filesystem/build/host, including
the real consumer's copying fallback where used. Q3 real-provider acceptance
still needs the separately authorized environments recorded in the frozen S3
qualification; the inspected pinned SDK's missing truncation evidence still
blocks a defensible prefix-completeness API. Runtime R1 remains AQ-HTTP-gated.
