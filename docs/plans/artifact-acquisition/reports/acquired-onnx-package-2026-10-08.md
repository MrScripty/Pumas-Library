# Acquired ONNX structural package registration

The acquired-model bridge separates verified assets, structural registration,
runtime compatibility and loaded execution. Existing local identification
recognizes ONNX as a Safe data representation, but filename/protobuf magic alone
cannot prove a complete graph or external-data package. This report specifies
the bounded source-neutral structural qualifier and its recorded tests.

## Acceptance class

The source-neutral acquired-model qualifier admits one immutable static FLOAT
tensor graph. It does not create an S3 parser or change object transport. Field
numbers follow the official [ONNX v1.16.2 schema](https://github.com/onnx/onnx/blob/v1.16.2/onnx/onnx.proto3);
reference semantics follow [ONNX external data](https://github.com/onnx/onnx/blob/v1.16.2/docs/ExternalData.md).
There is no existing structural ONNX reader to extract in this checkout. The new
small bounded reader uses existing dependencies and fails closed on fields
outside this class; it is not a replacement for the general ONNX checker.

| Concern | Accepted contract |
| --- | --- |
| Model | IR 7–10, exactly one standard opset import at version 13, one named graph. Standard domain is empty or absent. Producer/version/domain/doc strings are inert metadata. |
| Values | FLOAT only, statically known positive dimensions, rank at most 16; scalars are permitted. At least one typed input and output. Value information must agree with the inferred graph shapes. |
| Nodes | One-output Identity, Relu, equal-shape Add and compatible rank-two MatMul. No attributes, overloads or custom domains. Names/outputs are unique and inputs refer to topologically available values. |
| Weights | Dense immutable initializers with raw F32 bytes or EXTERNAL data. Initializers cannot overlap graph input names. Dtype, dimensions, byte size and storage state must agree. No type-specific packed tensor-data fields. |
| Single file | Root-level logical `.onnx` primary with all weights embedded. Nested single embedded selections are refused because the existing single-file copier normalizes their basename. |
| External package | Primary graph plus exactly the external files it references. Nested layouts are preserved by the existing multi-file copier. Location resolves relative to the selected graph's logical parent, never the host filesystem. |
| External ranges | Required location; optional unsigned decimal offset (default zero) and length (default remainder). Checked bounds and exact shape-derived length. Multiple tensors may share a blob, with padding and individually valid ranges. |
| Bounds | Existing 256 selected-file limit; 16 MiB protobuf graph; 100,000 fields shared across inspected nested messages; 4096 entries per repeated field; rank 16 and checked u64 byte arithmetic. External blob bytes retain acquisition/copy owners' existing resource controls. |

The same manifest lexical path validator rejects absolute, traversal, drive/URL,
backslash, empty-component, reserved-device and nonportable paths. References
must match exact selected logical names, including case, and cannot alias the
graph. A neighboring file, an unselected root-level blob or a parser-supplied
basepath cannot satisfy a nested graph's reference. The importer still preflights
the complete selected destination mapping and reserved names before publication.

Unknown/future semantic fields, dynamic/zero shapes, non-FLOAT types, other opsets
or operators, broadcasting/batched MatMul, sparse/training graphs, local functions,
attributes/subgraphs, custom code and unreferenced companions are outside this
class. External `checksum`/`basepath` and unknown/duplicate external-data keys
are explicitly refused rather than ignored. Integrity comes from the issued
receipt's whole-file SHA-256 and the copied-byte verifier. Pure numeric graphs
need no tokenizer, processor or Transformers config; text/image/audio package
semantics requiring those assets are not inferred here. This is a deliberately
small structural package class, not broad Transformers ONNX export support.

## Custody and publication

Qualification reads the held primary descriptor and uses receipt file sizes and
selected logical paths for external closure. It does not reopen paths, read
ambient config/tokenizers, load libraries, import model code or create sessions.
Copying verifies every graph/blob against the acquired SHA-256. The existing
receipt-bound intent, atomic stage/publication, confirmed output, reconciliation,
finalization/cancellation and recovery owners are unchanged.

ONNX task classification stays Unknown unless the existing explicit import hint
policy supplies a type. The existing metadata projector can recommend
`onnx-runtime` from the file extension; that format hint is preserved and is not
an admission or compatibility decision. Focused tests check Unknown task/type,
pending metadata review and no compatible-app claim. Safe here means a data representation under the existing
unsafe-format policy. Ready means confirmed library assets, not backend readiness.
No provider compatibility, embeddings capability, numerical execution or inference
claim is made. Runtime admission and native library provisioning remain
separate requirements. ONNX build/download behavior is unchanged; all Cargo commands use
`ORT_SKIP_DOWNLOAD=1`.

## Evidence and integration

Focused tests exercise genuine owned ModelProto/F32 bytes through generic HTTP
acquisition, issued receipt, shared importer, copied output, Ready metadata and
read-only output reconciliation. They also prove successful arbitrary-byte
acquisition with refused publication for malformed/missing/unsafe/custom packages;
those operations retain Using custody rather than being mislabeled as models.
Cancellation before finalization produces no consumer receipt or model. A valid
graph with changed tensor payload fails verified descriptor handoff; wrong receipt
intent and replaced primary paths also fail without publication. Positive cases
compare every copied graph/blob byte with its receipt-backed selected fixture;
copy-time SHA verification itself remains the existing unchanged owner. Parser boundary
tests exercise shared budgeting, field/item/rank limits, malformed wire encodings
and singular-field ambiguity. The official Python ONNX checker validates the
entire exported successful fixture corpus without inference, using isolated test
dependencies; no Cargo manifest or lockfile changes are made.

Recorded structural qualification used `ORT_SKIP_DOWNLOAD=1`, `CARGO_INCREMENTAL=0`, the locked
workspace and an owned target. Successful runs:

| Command scope | Result |
| --- | --- |
| `cargo test -p pumas-library --features s3 --lib model_library` | 886 passed, 6 ignored; includes four new parser boundary tests. |
| `cargo test -p pumas-library --features s3 --test acquired_onnx_package --test acquired_model_bridge --test s3_model_bridge` | 16 ONNX, 12 shared bridge and 4 native S3 regressions passed. |
| `cargo test -p pumas-library --no-default-features --test acquired_onnx_package` | 16 passed. |
| ONNX 1.16.2 `checker.check_model(path, full_check=True)` | All six exported successful fixtures passed; no inference session. |
| `cargo clippy -p pumas-library --features s3 --all-targets -- -D warnings` | Passed. |
| Changed Rust files `rustfmt --check`; `git diff --check` | Passed. |

The recorded broader default-feature/S3 library run passed 2,008 tests with
14 ignored and two environment baseline failures while creating the default
user registry on a read-only filesystem:

- `api::models::tests::get_inference_settings_batch_reports_per_model_errors`
- `api::s3_inspection::tests::persisted_inspection_changed_atomic_image_is_unavailable_and_noncreating`

Both failures reproduced in the earlier shared-bridge test binary; its core
sources were unchanged by the intervening public S3 contract/UI changes. No
baseline fixture was changed to mask these failures. Focused owned-registry
tests passed. This is not an all-green broader-suite claim.

No live provider, installed desktop, backend/model admission or inference
qualification is established by these structural tests.

The extension uses the existing acquired-package dispatch and atomic copied
importer. It does not widen the S3 transport contract or add runtime sessions.
The public S3 negative ONNX fixture lacks graph inputs/nodes and remains
outside the admitted class. Rerun the focused importer suite and generated
contract checks when changing these interfaces. Unknown semantic fields must
remain refused; successful transfer never establishes runnable output.
