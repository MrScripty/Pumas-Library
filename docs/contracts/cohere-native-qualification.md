# Cohere native audio qualification requirements

This records the acceptance requirements originally inspected after private
candidates `9da3f906aaebe2bfc8e790abaf1e600903b9531b` and
`0f778688ee12a274e42fe8391b08c51208f49cf0`, with the current evidence below.
Production audio remains unavailable. Dependency and controlled protocol evidence
do not establish a real model load, transcription quality, or model-specific
native disposal.

## Verified progress, 2026-10-10

The source-pinned managed CPython 3.12.14, Torch 2.10.0+cpu and Transformers 5.4.0
cohort has been installed through the existing owner and compared across two
independent installation roots. The retained native loader/library cohort and
selected Python bytes are bound by the source recipe.

At commit `444b668ec52844f6f800d3203d8fcd8934db13e1`, the
[hosted confined dependency probe](https://github.com/MrScripty/Pumas-Library/actions/runs/38008799725/job/114083904836)
passed Dynamo/Cohere class imports and a CPU operation under the required
Landlock/seccomp boundary. Its report at 00:36:11Z records
`child_tree_drained=true` and `production_available=false`; read/write, socket,
thread and cache-write refusal assertions passed. This proves that bounded
probe's dependency read closure, not every late read during real model inference.

At commit `3696fb8b3b57bed939d2febc4c3c62cba1da34a9`, the
[managed-interpreter lifecycle job](https://github.com/MrScripty/Pumas-Library/actions/runs/38010737841/job/114090075943)
passed the dependency probe again and executed three private-session tests:
hello cancellation, held-load source retention through uncertain-drain retries,
and endpoint-clone closure before drainage. The session tests use a controlled
backend without claiming native ASR or fixture-child confinement.

The conditional RPC/profile producer is published. It reserves the original
profile/generation before runtime/model preparation and joins the original
preparation and child owners through stop/shutdown. Shipping qualification
catalogs remain empty. Real pinned-model transcription and disposal are still
required before production admission.

## Selected model and access

The selected base model is `CohereLabs/cohere-transcribe-03-2026`, with native
architecture `CohereAsrForConditionalGeneration` and model type `cohere_asr`.
The public [repository metadata](https://huggingface.co/api/models/CohereLabs/cohere-transcribe-03-2026)
observed on 2026-10-08 identifies full revision
`d552dadc2922ba6f6c1f994f8db904d52dcaf942` (modified 2026-10-07). This is
the proposed immutable acquisition pin, not a claim that gated bytes were read.
Do not substitute mutable `main`, `refs/pr/6`, or the older short revision in
the model card's bibliography. The Arabic-specific later variant is a different
model selection.

The [official repository](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026/tree/main)
requires agreeing to contact-information sharing before file access. Its
advertised license is Apache-2.0. The exact access form must be reviewed by the
user in their own account; this audit neither accepted terms nor inspected
credentials. Public visibility does not grant gated file access.

No verified account access or accepted access form is recorded by this audit.
Development authorization does not establish gated-file access or acceptance of
contact-information sharing. The remaining external prerequisite is a supported
authenticated owning HF connection with the required access, or an already
authorized, Pumas-indexed local package. No credential should be placed in a
chat, fixture, or handoff. Acquisition approval does not clear the engineering
requirements below.

## Artifact verification

No official model-file content digest was available from the public metadata
read in this environment. Do not treat a filename, repository revision, aggregate
storage count, or our synthetic fixture hash as a model-file digest.

After authorized acquisition, use the existing artifact acquisition and model
index owners, not a new download or installation path:

1. Resolve repository metadata and every chosen member at the full revision
   above. Refuse a missing or different returned commit identity.
2. Retain the official per-member size and content identity. For an LFS member,
   compare streamed file SHA-256 with its published LFS SHA-256; a Git blob ID
   uses the Git `blob <length>\0<bytes>` object hash and is not a raw SHA-256.
   If metadata supplies another identity, document its verification semantics
   rather than interpreting it as a content digest.
3. Hash the actual acquired bytes, validate sizes, and record a SHA-256 manifest
   bound to the repository, revision and exact selected member set. Preserve the
   original descriptors; do not strip declarations to evade preflight.
4. Prepare through `ModelLibrary::prepare_cohere_artifact_use` and retain the
   root execution grant and held copied read source through native lifetime.
   File observation or a manifest cannot manufacture loaded-slot authority.

The current supported required members are `config.json`, `model.safetensors`,
`preprocessor_config.json`, `tokenizer.json`, and `tokenizer_config.json`.
At the selected public revision, also select `generation_config.json`,
`processor_config.json`, and `special_tokens_map.json`: eight members in total.
The generic optional `added_tokens.json` member is not listed at this revision
and must not be invented. The exact plan, including intentionally unknown
per-file digests and sizes, is [cohere-asr-acquisition-plan.json](cohere-asr-acquisition-plan.json).

Rust preparation and Python preflight accept only known original top-level
`auto_map` bindings, limited to each descriptor's role, and the original
`CohereAsrTokenizer` alias as inert metadata. Unknown, nested, repository-qualified
or list bindings, custom classes and secondary path redirects remain refused.
No descriptor is rewritten. Explicit installed `CohereAsrFeatureExtractor`,
`TokenizersBackend` and `CohereAsrProcessor` construction avoids Auto dispatch;
the model class is installed `CohereAsrForConditionalGeneration`, with
`local_files_only=True` and `trust_remote_code=False`.
Controlled fixtures and an executable branch check of the pinned Transformers
source establish that this explicit tokenizer reads selected `tokenizer.json`
without consuming `tokenizer.model`. Repository Python and `tokenizer.model`
remain unselected. This proves the controlled compatibility seam, not the
contents of gated descriptors or a complete native execution read closure.

## Runtime baseline and existing installer

The [official model card](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026)
documents native Transformers support from 5.4.0 and testing with Torch 2.10.0.
The `cohere-asr` adapter uses the existing resolver, managed Python provider,
installer and version manager. Its compatibility profile pins Transformers
exactly 5.4.0 and retains the exact selected Torch version, build and Python
variant, requiring Torch >=2.4.0 for this native Transformers implementation.
The floor comes from [the pinned native import gate](https://github.com/huggingface/transformers/blob/v5.4.0/src/transformers/utils/import_utils.py).
That floor does not claim model qualification for every allowed Torch version.
The upstream tested Torch 2.10 cohort remains a separate acceptance candidate.
The bundled image recipe
couples Torch 2.9.1+cu130, Transformers 4.57.6 and image/Nunchaku dependencies;
it cannot attest this ASR baseline. Do not overwrite that image preset.

A minimal initial Linux CPU candidate avoids a CUDA driver qualification:
managed CPython 3.12, Torch 2.10.0+cpu and Transformers 5.4.0. This is a proposed
cohort from the original inspection; the exact CPU cohort is now covered by the
verified dependency evidence above. Official CPU binaries are listed in
[PyTorch's version instructions](https://pytorch.org/get-started/previous-versions/).
Public distribution metadata records these two exact wheel identities:

| Distribution | Published SHA-256 |
| --- | --- |
| `torch-2.10.0+cpu-cp312-cp312-manylinux_2_28_x86_64.whl` | `ee40b8a4b4b2cf0670c6fd4f35a7ef23871af956fecb238fbf5da15a72650b1d` |
| `transformers-5.4.0-py3-none-any.whl` | `9fbe50602d2a4e6d0aa8a35a605433dfac72d595ee2192eae192590a6cc2df86` |

Sources: [official CPU wheel index](https://download.pytorch.org/whl/cpu/torch/)
and [PyPI release metadata](https://pypi.org/pypi/transformers/5.4.0/json).
No wheels were downloaded during the original metadata inspection. Subsequent
installation and retained transitive-byte evidence is recorded above; these two
wheel identities alone do not pin that closure.
The ordinary Linux Torch wheel on PyPI is a different build with CUDA
dependencies; its metadata must not be substituted for the CPU index recipe.
Resolve and retain exact wheel versions, hashes, licenses and installed-file
identities for the full platform-specific closure through the existing installer.
The direct ASR install path reuses the existing trusted-source report checks, hash-locked
requirements, staged RECORD validation and installed-file publication. Its native
imports must succeed before installation publishes; successful imports are
recorded as inconclusive because no model was loaded. Its runtime recipe grants
no image-generation capability and cannot mint audio execution admission.
The profile is available through native preview/install and existing RPC selection
using adapter `cohere-asr`; the desktop adapter chooser has not been expanded.
The older retained resolved-plan installer does not produce this installed-file
custody manifest and refuses ASR before filesystem or process effects. ASR callers
must consume the public Ready selection through the direct staged installer.

The earlier inspected host was Linux x86_64, glibc 2.41, Python 3.12.14, with no exposed
CUDA/ROCm GPU device. At inspection it had 4,056,281,088 bytes free in the
workspace, less than the public repository's 4,132,801,897-byte storage count.
That count was not an exact download budget. Those capacity observations are
historical and do not describe the current executor. Recheck capacity for the
full package, installed runtime and retained copied model bytes before acquisition.
Provision adequate space or use an authorized local installation elsewhere
before acquisition. No minimum execution-memory claim is made without loading.

The native feature extractor requires Torch and librosa and constructs a librosa
mel filter even for already normalized PCM. The pinned audio utilities also
import soxr. Qualify the resolved NumPy/SciPy/Numba/LLVM, soxr and any libsndfile
dependencies actually installed/imported. Accelerate >=1.1 is required by the
current `device_map` load, including CPU. Transformers also requires HF Hub
>=1.5,<2, tokenizers >=0.22,<=0.23 and safetensors >=0.4.3, plus its other base
requirements. The model card additionally lists soundfile, sentencepiece and
protobuf; audit actual native/tokenizer consumption before reducing this recipe.
Do not add datasets or inference-server extras solely because optional examples
use them. See [pinned feature extraction](https://github.com/huggingface/transformers/blob/v5.4.0/src/transformers/models/cohere_asr/feature_extraction_cohere_asr.py)
and [pinned package requirements](https://github.com/huggingface/transformers/blob/v5.4.0/setup.py).

## Conditional source implementation

The [installed audio constructor](installed-audio-constructor.md) retains the
original prepared allocation, conditionally transfers runtime custody, and uses
sealed Linux memfd snapshots for selected model descriptor and weights reads.
Its private loader constructs explicit classes from those bytes without model
package/cache lookup. Shipping policy remains empty: immutable model snapshots
alone do not establish interpreter/dependency trust, native filesystem read
containment, real transcription, or native disposal. The positive policies and
native constructors in unit tests are controlled fixtures.

## Remaining execution and acceptance gates

Selected installed Python files and the managed-depot lease establish byte
custody but do not contain native loader reads. ELF interpreter paths, dependent
shared objects, search paths, late `dlopen`, locale and auxiliary native data
must be owned and fenced. Ambient `LD_PRELOAD`, `LD_AUDIT`, loader search variables
and profile overrides cannot enter a qualified child. Python `-I -B -S` does
not establish these ELF properties. Static dependency inspection, a clean probe,
or `/proc` maps cannot by themselves prove the complete late-load closure.
The controlled ELF inspection added by this slice cannot enable the shipping
runtime factory or authorize native execution.

The native processor read set includes descriptor-controlled secondary reads;
`local_files_only=True` prevents network lookup but does not fence local/cache
reads. Reject secondary audio-tokenizer and tokenizer-path redirects before
loading, and qualify actual config/processor/tokenizer classes and generation
settings against the immutable files. Preserve integer decoder tokens and mask
dtypes when moving features. Only a single original sample and single chunk
can enter the current bounded adapter: processor chunk metadata must not reach
generation as an unused model argument, and multiple chunks cannot be silently
dropped. Terminal evidence must include the effective decoder-start prefix.

Official feature normalization needs a usable frame count. Real acceptance must
exercise very short input, finite features, silence and ordinary speech; a valid
generic PCM declaration does not imply the native feature extractor supports
every frame count. Keep generic input semantics stable and refuse unsupported
native input before effects rather than inventing an undocumented duration floor.

Before production admission, observe an actual pinned model load and a finite
single-chunk transcription through the owning generic endpoint, selected-byte
read containment, truthful stop/length evidence, queued and admitted cancellation,
caller loss, exact device/object disposal, quarantine and a clean next operation.
CPU execution does not establish GPU behavior. Classification, long-form chunk
assembly, diarization, timestamps and auto language detection are not qualified
by this bounded transcription path.

The pinned processor's ordinary single-sequence `decode` is the current output
shape. Device conversion preserves integer decoder IDs and mask dtypes. The
current loader omits an explicit checkpoint dtype selection; qualify its actual
load dtype and memory use instead of assuming the advertised BF16 storage
implies BF16 execution.

Hung native work can retain custody indefinitely. No new timeout is implied by
this report. Recovery requires observed exact-child drain; failed or poisoned
cleanup retains custody as documented in [the lifecycle contract](owned-audio-lifecycle.md).
Source increments and qualification evidence are published in the draft
integration PR. No main merge, release or real-model acceptance is established
by this document.
