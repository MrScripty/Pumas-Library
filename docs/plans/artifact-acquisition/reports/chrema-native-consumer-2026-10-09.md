# Actual Chrema consumer / ec03 native qualification

The unmodified published Chrema generic consumer passed bounded native
authentication, retention, catalog/lookup and caller cleanup against the already
qualified ec03 executable. Independent execution repeated the same three cases
on fresh disposable roots. No Pumas production defect or blocking wire mismatch
was exposed. The deliverable is the actual-consumer qualification harness and
its frozen receipts; no replacement consumer or Python SDK is introduced.

## Exact cohort

| Input | Identity |
| --- | --- |
| [Chrema consumer source](https://github.com/MrScripty/Chrema/tree/d9f660beb8273aac16e62fae3902b9f5e4d1559b) | head `d9f660beb8273aac16e62fae3902b9f5e4d1559b`, tree `cc755424881f3c53b20b626ca7b44f14fb5f2cb8`; clean before/after execution |
| Chrema published branch | `feature/pumas-supported-session-20261009` |
| Pumas contract/source base | `fe3c9f04c243f436a393ae47e16f616244a85cee`, tree `7ff1f50411824eefdb0481792d99584bdeb6ae36` |
| Actual normal executable compiled source | `ec03d0a6d4d0572674b4cb4269da5cabbe4f9d7d`, tree `9a897c4bfbd87362fe96de1327c5f9f9d8b575f9` |
| Linux x86_64 executable | 203722168 bytes, SHA256 `61c20bd24bd179fbc14538183c6898c14e677d84209413de69e1d797584a0a47` |
| Existing Pumas fixture session helper | 27998 bytes, SHA256 `ee267a62f2a688edfb3e9bbe79c704c7f69ed754be32bd66fa6f1e8e7b8ea6b9`; unchanged |
| [Operator runner](../../../../scripts/consumers/qualify_chrema_native.py) | 13898 bytes, SHA256 `1dbf226fab291d50858f97b0811cbc078e8acf3f59c0a8eaf6740b5af70177ab` |
| [Actual consumer probe](../../../../scripts/consumers/chrema_native_probe.mjs) | 9422 bytes, SHA256 `3013f08cea338460fb9558c946339fe7f395b20590ae21fec8ac90a14c25e4bd` |
| Observed interpreter tuple | Linux, CPython 3.12, Node `v24.19.0`; Node executable hash and five exact imported Chrema module hashes in receipts |

The executable retains package version 0.7.0 and shared `PumasBuildInfo` identity.
No Cargo build, ONNX download, new embedded build identity or version bump was
performed. New qualification source does not relabel the compiled executable.
The inspected lifecycle/catalog/lookup producer implementations are identical
between ec03 and fe3; the consumer's declared fe3 contract provenance is preserved.

## Actual native behavior

The operator runner creates only fresh authorized disposable roots, isolated
config/cache/registry/HF paths, empty credential files and sentinels. It starts
and supervises its own pinned Pumas owner through the unchanged existing helper.
Chrema then borrows that live owner; its adapter never starts or stops an owner.
The owner's executable cohort is known from this supervised launch, rather than
inferred from the borrowed observer hash.

The probe dynamically imports Chrema's exact `openPumasLocalConsumer` with its
real local session, retention bridge and model catalog modules. Passive observers
log actual Node spawn/request/end/data/close events, always forwarding the original
calls, objects, arguments, bytes and outcomes. Observation parsing failures are
contained and cause qualification failure later. No fake CLI or HTTP peer,
generated binding, substitute consumer, inference adapter or synthetic model is
used. Imported finite-provider/capability modules are not dispatched.

| Case | Result |
| --- | --- |
| All three borrowed contexts | Two real `--describe-local-http --launcher-root ROOT` children exit zero; both native descriptions match the supervised owner. Root, endpoint and paired generations agree before and after actual retention. |
| Actual retention | Native `pumas-owner-retention` SSE acknowledgment observed, with exact `retained` state and both generation fences; no reconnect or elapsed hold timer. |
| Catalog and lookup | Native empty `{success:true,models:{}}` catalog projects to `{modelIds:[]}`; canonical absent lookup preserves v1 requested ID and `missing` resolution. |
| Host authority | Separate callbacks run before dispatch and before disclosure. Deliberate catalog denial sends no transport request; fresh grant admits another actual catalog read. These are authorized fixture grants, not Nearwork signed-user authorization evidence. |
| Invalid ID | Chrema rejects `unknown//invalid` locally with `TypeError`, before authorization or transport. This does not claim native `-32602` acceptance through Chrema. |
| Normal release | Sticky release settles, consumer/retention signals invalidate, all local transports close, owner remains healthy. |
| Caller cancellation | Actual caller signal triggers cleanup after completed reads; release settles, borrowed owner remains healthy. No in-flight reconciliation or inference cancellation is tested. |
| Native owner revocation | Operator gracefully exits only its own Pumas context, sending SIGTERM to its supervised child through the existing helper. Native SSE emits `revoked`; Chrema invalidates and releases. This is not a shutdown RPC, crash or historical-owner recovery. |
| After every closed context | Catalog and lookup refuse before creating any new transport or observer child; no fallback start, successor attachment or retry. |
| Orderly fixture close | Own Pumas children exit zero and are reaped; disposable registry instance rows are empty and sentinels unchanged. |

Each attempt has three cases, nine native Chrema RPC responses, three native
Chrema retention streams and ten additional fixture-control RPC responses.
It observes twelve native children (six Chrema authenticated CLI observers and
six operator-owned bootstrap/retention children) plus three Node callers, all
zero-code exits and observed PID absence after reaping. Three temporary Python
helper description processes are internally awaited but excluded from that
recorded child count. No forced shutdown occurred. The unchanged helper can
force only its own children on failed grace and reports unconfirmed custody;
this cohort does not qualify that failure path.

The two successful attempts total 38 native RPC responses, six actual Chrema
retention streams, 24 recorded native child cleanups and six Node caller cleanups.
The earlier Pumas sample runs are excluded and their draft source was preserved
outside Git without publication.

## Frozen receipts and recovery

| Receipt | Bytes | SHA256 |
| --- | ---: | --- |
| [Root actual execution](chrema-native-consumer-2026-10-09/native-qualification.json) | 70249 | `2852f623dab7907be2153ce1df666fb351c3c17f8bfec90c07f56acd18299fd3` |
| [Independent actual execution](chrema-native-consumer-2026-10-09/independent-native-qualification.json) | 70585 | `58cc5a31c748141e55dc8aaf8aa0f33956c052f77667af28880896a4baf21f8a` |
| [Independent source clearance](chrema-native-consumer-2026-10-09/independent-source-review.json) | 3428 | `e3217db6c6c90fb94d0535be03d072cf7db19c822cb94ecf445e572b731ac7cf` |
| [Independent runtime clearance](chrema-native-consumer-2026-10-09/independent-runtime-review.json) | 7715 | `2f3e772b1bde0a79a7fb1cdefdc9285b0abd9de70b8dfb5def4bed974106df72` |
| [Independent qualifier process](chrema-native-consumer-2026-10-09/independent-process-receipt.json) | 2856 | `14cee645a45d7384d3f25efeaf85c36b954abcc7802caf4c0793864ba4a09667` |
| [Executor recovery](chrema-native-consumer-2026-10-09/executor-recovery.json) | 341 | `dc5931ea7bb4a830cb8410d2c331e7a95c969e35eadb5711bb2b7f67137b293a` |

The reported 11:10:56 UTC environment disconnection preceded any actual Chrema
dispatch. An actual executor/process inspection recovered successfully and found
no Pumas or Node process outstanding; no uncertain operation was retried. Root
execution ran 11:17:18–11:17:25 UTC; independent execution ran 11:19:21–11:19:27 UTC.
Both preserve their exact source/binary hashes, native frames, RPC bodies and
process results. Finished-attempt descriptions do not grant current availability
or reusable owner authority.

Each attempt uses offline flags, empty credential files, metadata disabled and
`ORT_SKIP_DOWNLOAD=1`, plus an owned deny proxy. Both proxies observed zero
requests, forwarded zero and joined their threads. This is configured proxy
behavior, not OS-wide network enforcement. No persistent credential was changed.

## Reproduce the bounded cohort

Use the exact clean Chrema source pin, an existing fe3 contract-source checkout,
the separately qualified executable and a new disposable attempt directory:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/consumers/qualify_chrema_native.py \
  /absolute/qualified/pumas-rpc \
  --binary-sha256 61c20bd24bd179fbc14538183c6898c14e677d84209413de69e1d797584a0a47 \
  --chrema-source /absolute/Chrema-at-d9f660be \
  --pumas-source /absolute/Pumas-contracts-at-fe3c9f04 \
  --work-dir /absolute/NEW_DISPOSABLE_ATTEMPT
```

The runner refuses an existing attempt directory, validates clean Chrema identity
and pinned helper/binary bytes, and durably checkpoints PIDs and completed reads
in `qualification.json`. It changes no consumer source or private production
root. A failed or unresolved attempt must be inspected using its existing receipt
and owned processes; never rerun it to manufacture certainty.

This qualifies actual Chrema generic-module control-plane integration. It does
not qualify Nearwork UI/domain workflows, signed application authority, native
Found/Reclassified/busy-error campaigns through Chrema, same-URL successor
behavior, real model inference, downloads, live providers, selected ONNX SDK
trust/load, crash/cold recovery, physical-store recovery or other platforms.
Catalog presence is not readiness; a replacement requires deliberate fresh
selection. There is no lookup advertisement marker, and arbitrary borrowed-owner
binary qualification remains a host responsibility. Existing provider, ONNX trust,
inference and retained-root custody holds remain in place. No main merge, release,
tag, public Chrema mutation or large Library archive accompanies this cohort.
