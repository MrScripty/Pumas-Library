# Local Cohere transcription

Status: loader and private Python operation owner (milestone A in `local-asr-operation-contract.md`) are implemented and synthetically tested. Next slice: production shutdown/model custody, Rust ownership and truthful capability discovery. No available end-to-end transcription capability is claimed.

## Objective and ownership

Support the requested Lanternwake voice-to-text feature through Pumas Library, using an installed local Cohere Transcribe model. Pumas owns model identity, managed runtime, operation admission, inference, cancellation and terminal cleanup. Lanternwake owns consent-triggered bounded capture and editable transcript review before explicit submission.

The isolated Python slice owns `torch-server/loaders/cohere_asr_loader.py`, loader dispatch in `loaders/__init__.py`, `ModelManager.speech_lease`, focused tests and this plan. The private Python operation owner is implemented in `speech_operations.py`; its ownership and verification are recorded in `local-asr-operation-contract.md`. Remaining integration covers public Rust contracts, RPC handlers, serving capability projection, runtime recipes and production shutdown composition. Acquisition/import/custody implementations are unchanged.

## Internal adapter contract

- Installed directory with native `cohere_asr` config and `CohereAsrForConditionalGeneration` architecture, local safetensors weights. Pumas library resolution must independently bind repository/revision or content digest and retain asset custody. Configuration validation alone is not identity attestation.
- Native `AutoProcessor` and `CohereAsrForConditionalGeneration`, both `local_files_only=True` and `trust_remote_code=False`; model load also requires safetensors. Imports are lazy. An older runtime raises `SpeechRuntimeUnsupported` without breaking unrelated imports.
- `transcribe(model, processor, pcm16le, language, cancel)` accepts bytes containing one to 480,000 signed little-endian mono samples at 16 kHz (up to 30 seconds), and an explicit language from the model's 14-language set. It accepts no URL or caller-supplied file path.
- Generate at most 512 new tokens. Return one UTF-8 transcript bounded to 16,000 bytes; reject invalid/oversized output, do not silently truncate. Empty text is permitted for silence and must remain text, never a gameplay command.
- No thread is created by the adapter. The operation wrapper must hold `speech_lease` until the synchronous function returns. Cooperative cancellation is checked before work, after preprocessing/generation/decoding and through a stopping criterion. Preprocessing and the encoder may not respond immediately. A cancellation acknowledgement is not proof of worker cessation. Initial devices are CPU and CUDA only; MPS/other accelerators fail unsupported before loading. CUDA is synchronized before clearing the owned float audio conversion. If synchronization fails, `SpeechCleanupUnconfirmed` retains the original error, cleanup error and buffer; the operation wrapper must fence/retain the device inside its lease until runtime/process cleanup proves cessation, then release the buffer. Ordinary context-manager exit is not sufficient in that outcome. No universal secure-memory-erasure claim is made.
- The shared device lock prevents unload and cross-model concurrent inference while the lease is held. The operation wrapper must not cancel an executor future and release that lease while its native thread continues.

## Public operation design, pending integration

The producer must implement truthful capability discovery tied to the installed model and qualified runtime, then an operation-ID-based admission/status/cancel contract. Request metadata must explicitly identify encoding, sample rate/count and language; response must bind the model/operation identity to bounded text or a typed terminal error. The Rust and Python operation owners must agree exact routes, fields and protocol version together. No consumer should infer these names from this proposal.

Missing model, missing runtime, wrong architecture, unsupported language, busy runtime, malformed audio, cancellation, shutdown and invalid backend output need distinct outcomes. A disconnected client does not authorize replay or release of worker custody. No hosted fallback is permitted.

## Runtime dependencies and acquisition

The [official model card](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026) recommends native Transformers >=5.4.0 and reports testing with torch 2.10.0. It lists torch, huggingface_hub, sentencepiece, protobuf and accelerate alongside the processor dependencies. This PCM-only adapter also uses NumPy; it requires no compressed-audio decoder. Existing FastAPI/Uvicorn remain the sidecar transport. A compatible exact dependency lock must be qualified separately before production capability is advertised.

The current managed runtime pins Transformers 4.57.6 and cannot supply this native model class. This slice deliberately does not upgrade the shared image/text runtime recipe or lock. The ASR runtime selection and qualified dependency set remain the runtime owner's work.

[Official files](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026/tree/main) list 4.13 GB weights under [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0). The page currently requires contact-sharing acceptance; exact fields/conditions are not visible while logged out. No gate acceptance, download, credential creation, paid API call or private voice transmission occurred. Local model inference remains blocked until acquisition and runtime setup are authorized and available.

## Acceptance evidence

Loader tests use synthetic classes and PCM to prove local-only loading arguments, native class requirements, config rejection, input/language/output bounds, cooperative cancellation points, conversion cleanup and device lease behavior. They do not demonstrate model accuracy or actual Transformers execution.

The developer test requirements pin NumPy 2.3.5 for actual PCM conversion assertions.

Run `python3 -m unittest discover -s torch-server/tests -p test_cohere_asr_loader.py -v` and the existing model manager/load lifecycle suites. Milestone A evidence in `local-asr-operation-contract.md` covers private admitted-work cancellation, repeated cancellation, observer loss, deduplication, bounded drain and retained unconfirmed custody. Public transport/model identity and production shutdown composition remain separate integration gates. Finally run a real approved installed-model synthetic speech fixture through Pumas and the game. Physical microphone and speech quality require separate evidence.

### Loader slice checkpoint, 2026-10-03

- 15 focused adapter/lease tests passed, including unsupported runtime/device, local loading arguments, cancellation points, and unconfirmed CUDA cleanup.
- Existing model manager (2 tests) and model load lifecycle (7 tests) passed.
- Whole Torch Ruff 0.15.2 checks and formatting passed.
- The global Python broad suite executed 135 tests with 7 errors in existing FastAPI route enumeration (`_IncludedRouter.path`). The unchanged PR22-equivalent baseline executed 120 tests with the same 7 errors in the same environment. This is an inherited environment mismatch, not a full-suite pass. No route test was weakened.
- Real model loading, CPU/CUDA inference, managed ASR runtime, public operation lifecycle and game transcription remain unqualified or unimplemented. Loader source acceptance does not satisfy those claims.
