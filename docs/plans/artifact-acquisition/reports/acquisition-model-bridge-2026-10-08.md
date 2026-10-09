# Shared acquisition-to-model import bridge

This report describes the shared GGUF and safetensors acquisition-to-import
bridge. The [S3 brief](../../../breif/s3-model-fetch.md) separates generic byte
transfer from model semantics. The subsequent [bounded ONNX extension](acquired-onnx-package-2026-10-08.md)
and [public S3 flow](s3-public-model-bridge-2026-10-08.md) build on this contract.

S3 is an arbitrary byte store. Its resolver, manifest and transfer engine remain
format independent. A successful transfer establishes the exact selected bytes;
it does not establish a complete model or authorize loading/executing those bytes.
This change puts model qualification in the shared importer and leaves backend
compatibility and real inference to the existing runtime policies and adapters.

## Shared entry and acceptance contract

`ModelImporter::import_acquired_model` accepts the current issued consumer receipt
with the exact serialized `ModelImportSpec`, a nonempty fully SHA-256-bound selected
set and an exact selected primary weight logical path. It qualifies held input
files and uses the existing copied-import owner and atomic publication pipeline.
`reconcile_acquired_model` is its read-only receipt/output proof callback; the
acquisition consumer still owns settlement. Existing GGUF-specific methods and
receipt schemas remain available and compatible.

The bounded acceptance set is:

| Representation | Qualification |
| --- | --- |
| Single GGUF or existing GGUF with inert auxiliaries | Existing primary magic/identification and auxiliary policy. Auxiliary JSON/index names remain inert; their presence does not change the package type. |
| Single safetensors | Genuine little-endian header, unique JSON members, tensor dtype/shape/offset validation, exact contained data coverage without holes, overlap, truncation or trailing bytes. No tensor payload is allocated for inspection. |
| Transformers safetensors directory | Root config and safe tensors; complete shard sets and one component-local index when multiple weight files are selected; exact index tensor-to-shard and tensor inventory closure; required tokenizer/config and processor documents. |
| Standard Diffusers safe tensor directory | Exact `StableDiffusionPipeline` or `StableDiffusionXLPipeline` index with required unet, VAE, text encoder, tokenizer and scheduler components (plus the second encoder/tokenizer for XL). Every declared nonoptional component requires its selected configuration/assets. Existing staged component validation and Diffusers metadata/publication are reused. |

Transformers package config families admitted by this bounded contract are
`llama`, `mistral`, `qwen2`, `qwen3`, `bert`, `roberta`, `vit` and `whisper`.
Text families require `tokenizer_config.json` and `tokenizer.json`; ViT requires
processor configuration instead of a text tokenizer. Whisper and a config with
`processor_class` require a nonempty `preprocessor_config.json` or
`processor_config.json`. All selected package JSON is bounded, duplicate-free
and object-shaped; required configurations must be nonempty. Admitted config
provides coarse type classification without unselected neighboring-file reads.
Classification remains subject to existing metadata review.

The tokenizer contract covers JSON serialization version 1.0 with WordLevel,
WordPiece or BPE vocabulary encodings, unique integer vocabulary IDs, required
unknown tokens and complete BPE merge vocabulary references. This is structural
asset qualification, not a tokenizer execution or architecture/weight-shape
compatibility test. Backend deserialization, class support, numerical behavior
and complete model-specific semantic validation remain separate requirements.

Safe tensor dtypes admitted here are BOOL, U8/I8, U16/I16, U32/I32, U64/I64,
F16/BF16/F32/F64 and F8_E4M3/F8_E5M2. Tensor dimensions and byte arithmetic must not
overflow. The new non-GGUF contract is bounded to 256 selected files, 16 MiB of
aggregate selected JSON documents, and 16 MiB of aggregate safetensors headers.
GGUF retains its existing bounds. The wire validation follows the
[official safetensors format](https://github.com/huggingface/safetensors#format)
using existing dependencies; no new package or model download is required.

The [ONNX structural extension](acquired-onnx-package-2026-10-08.md) adds a
bounded class of static FLOAT graphs with exact external-tensor closure. Other
ONNX graphs, pickle/PyTorch/GGML, adapters, SentencePiece binary/tiktoken
tokenizers, additional Diffusers classes and arbitrary unknown formats remain
outside this publication set. Accepted safetensors data is entirely contained
in its validated offsets; the ONNX extension separately validates selected
external files. Generic acquisition still permits unsupported representation
bytes. Local/HF broader import paths and unsafe-format acknowledgments are
unchanged. Packages selecting executable-format members, custom Diffusers
libraries or JSON `auto_map` are refused here; acquisition grants no custom-code
consent.

## Custody, lifecycle and integration

Inputs are opened through `AcquiredArtifactUse::open_file`, held throughout
qualification and copied publication, rewound after inspection, and copied from
those descriptors. Identification explicitly disables ambient filesystem
context. The shared HF selection/index validators were extracted into
`model_library/package_selection.rs`; HF retains a small forwarding module.
Indexes never expand the selection or authorize filesystem access.

The existing copy-time SHA-256 comparison binds qualified inputs to the receipt
bytes. Safe logical paths and reserved destination admission, capability-held
stage/root custody, index collision refusal, payload/metadata proofs, durable
Ready publication and read-only reconciliation remain with their original owners.
No second publisher, mutable registry, acquisition owner or receipt issuer exists.

The native S3 facade now delegates to the generic importer after existing
selection, transfer, verification and finalization admission. Its request shape,
source pins, credential lifetime, progress and cancellation gate are unchanged.
Cancellation can win before finalization/receipt issuance; once finalization
starts its owned pipeline settles. A failed qualification can leave verified
acquisition `Using` custody and an issued receipt without a published model.
Use the existing retained-work/reconciliation policy; do not replay with a new ID.
Read-only reconciliation proves the exact existing output and never reruns import.

The [public S3 flow](s3-public-model-bridge-2026-10-08.md) exposes explicit
selected-primary paths through the same importer. Desktop source selection
validates paths and source pins; it delegates model-content qualification and
publication to the shared bridge. It must retain the existing error and
cancellation behavior without adding source-specific model parsers.

HF and S3 callers share `model_library/package_selection.rs`, importer
registration and the existing atomic publication pipeline. When composing
changes to importer/identifier call sites, HF forwarding imports or native S3
wiring, rerun the acquired-model and S3 integration targets and the shared
validator regressions. Publication, runtime admission and recovery remain
separate authorities.

## Verification and limits

Owned tiny format fixtures contain actual F32 safetensors data, serialized
WordLevel tokenizer JSON and package documents. Tests use local loopback byte
sources and explicit temporary registries. They exercise byte transfer, receipt
binding, copied publication, index visibility/Ready state and same-owner read-only
reconciliation. They establish import qualification, not meaningful model
inference, live-provider behavior, process-loss recovery or cross-platform durability.

Recorded bridge qualification used locked official dependencies and
`ORT_SKIP_DOWNLOAD=1`. The acquired-model and native S3 suites passed 12 and
four tests; existing S3 acquisition/workflow suites passed 44 and nine tests.
The model-library suite passed 870 tests with six ignored. The acquired-model
suite also passed its 12 tests without S3, and the default-feature/S3 run passed
12 acquired-model plus four native S3 tests. Strict Clippy and formatting passed.

The pre-bridge baseline passed 122 importer tests with one ignored, 868
model-library tests with six ignored, and the 44/9 S3 acquisition/workflow
tests. No baseline code failure occurred in those executed scopes. Tests used
explicit owned registries; the default user registry was read-only in the test
environment. Ignored tests remained ignored. These results do not establish
full-workspace, real-provider or inference acceptance.
