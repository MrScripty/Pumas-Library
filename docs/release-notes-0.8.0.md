# Pumas Library v0.8.0

Pumas Library 0.8 makes Pumas easier to build on. Applications that use Pumas
for their models get one typed API to discover what is served and run it, and
the desktop library now shows models by what they are used for.

The largest changes are:

- a typed model-operations API with model discovery (`GET /v1/served-models`)
- new `decision`, `rerank` and `multimodal_embedding` capabilities, and
  bounded image-to-text
- text embedders and rerankers served by llama.cpp, ONNX Runtime and Torch,
  computed the way each model's sentence-transformers declarations say
- ONNX Runtime as a downloaded, version-managed runtime with CUDA support, and
  runtime profiles that can each pin a version and run side by side
- model cards, license files and bundled code kept with every download
- third-party telemetry turned off in every runtime Pumas starts
- models grouped by use, with new **Decision and evaluation** and **Video
  generation** categories, input/output badges and adapter marking
- a model's category is metadata: changing it never moves a model or changes
  its ID
- local owner discovery for applications that embed Pumas
- digest-checked local imports and S3 imports
- inference-enabled headless builds for Linux, Windows and macOS
- an explicit, backed-up upgrade for older libraries
- a locked-down desktop app and hash-checked runtime installs

## For desktop users

### Models grouped by use

The library groups models by what you would use them for: LLM,
vision-language, **Decision and evaluation** (new), embedding, reranking, image
generation, **Video generation** (new), audio and vision. Each row shows the
inputs and outputs the model handles (for example *image → video* or
*audio → text*), and LoRA/PEFT adapters are marked **Adapter**.

Detection now trusts what the files say over a repository's task tag: a GGUF
with a decision head is a decision model, a `…ForASR` architecture is audio,
and video tasks are video. When a task tag disagrees, the model is flagged for
review instead of being overruled by the tag.

If Pumas gets a category wrong, choose **Used as** in the model's metadata
window. Your choice is kept by later re-scans; clearing it re-detects the
category.

### Categories never move your models

A model's category is now metadata. Re-detection or your own choice changes
how the model is grouped, never its folder or ID, so served-model IDs and
runtime profiles keep working. (A change of model family can still move it.)

The first time 0.8.0 opens a library it updates the library's classification
rules in place; the next refresh re-detects categories without moving anything.

### Torch runtimes resolve current versions

Pumas no longer installs Torch from a fixed package lock. The fixed v2.9.1
preset is replaced by **Verified combinations**: the install dialog offers each
one as a shortcut (for example *cu130 · Python 3.12 · Nunchaku*), and
installing it resolves current package versions. Verified combinations are
evidence of what was tested, not pins. Runtimes installed from the old preset
keep working.

Every Torch install carries all general modalities: text (with Jinja), image
and audio packages resolve together, so image generation, Cohere ASR,
embedders and rerankers work in any install. Nunchaku is the only optional
extra. A modality with no compatible wheel is left out with its reason instead
of failing the install. Torch wheels download through Pumas, so a dropped
connection resumes from the bytes already received, and each wheel is checked
against its published SHA-256.

### Runtimes run side by side

A runtime profile can pin an installed version, so llama.cpp, Torch and ONNX
Runtime profiles on different versions serve at the same time. Changing a
runtime's active version no longer stops running profiles; it applies to new
launches.

ONNX Runtime is now downloaded and version-managed like llama.cpp and Torch,
and no Pumas package bundles it. Its panel lists official releases as CPU,
CUDA 12 and CUDA 13 builds. Installing a CUDA build also installs the CUDA and
cuDNN libraries it needs, into that version. Each ONNX Runtime profile runs in
its own host process on the device you choose (CPU, CUDA, or CoreML on
macOS), so switching its version restarts only that host, never Pumas. The
NVIDIA driver stays the operating system's: when it is missing or too old,
Pumas says so and points at NVIDIA's download.

### Model cards, licenses and code

Downloads now keep the model card (`README.md`), license files and any code
the repository ships, next to the weights. The model's metadata window has a
**Card & License** tab. Code is kept as a record only; Pumas never runs it.
Models downloaded earlier can fetch what they are missing with
`backfill_library_companion_files`, which is a dry run unless you turn that
off.

### Privacy

Every runtime and conversion process Pumas starts runs with third-party
telemetry off (`HF_HUB_DISABLE_TELEMETRY`, `DO_NOT_TRACK` and ONNX Runtime's
telemetry), and with `HF_HUB_OFFLINE=1`, so model files arrive only through
Pumas's own downloads.

### Clearer startup and recovery

- If the backend cannot start, the recovery screen shows the actual reason.
- Download state written by a newer Pumas no longer stops the backend: the
  library, models and inference start, downloads are paused without rewriting
  that state, and a banner explains why.
- A failed Torch startup trial says why the managed launch failed.
- Desktop views that receive an invalid update report it and reload instead of
  going stale.
- A failed download whose files an older version moved can be kept without
  downloading it again: see
  [Recover a failed download whose files moved](library-upgrade.md#recover-a-failed-download-whose-files-moved).

### Safer desktop and runtimes

- The desktop app stays on its own bundled pages. Links open in your browser;
  other sites, pop-up windows and browser permission requests are refused.
  Packaged builds have no DevTools and ignore `--dev`, `--debug` and
  `PUMAS_RPC_BINARY`.
- Ollama installs only from archives that match the publisher's SHA-256, and
  Torch installs only from a hash-checked lock made for that install.
- Runtimes start with a minimal environment: `LD_PRELOAD`, `PYTHONPATH` and
  credentials are not passed to them. Stopping a runtime signals only processes
  that Pumas started and recorded.

## For applications that use Pumas

### One typed API

Start from `GET /v1/served-models`: it lists every served model and profile
with its provider, context size and capabilities, so you discover models
instead of configuring profiles by hand. Send operations to
`/v1/model-operations`. A request needs only `model`, `input` and `output`; add
`capability` when Pumas answers `ambiguous_operation`, and `profile` when
`profile_required` is true. Every request accepted before stays valid.

Every error carries a stable `code`, an `outcome` and a human-readable
`message`, echoes your `request_id` when it is readable, and uses an HTTP
status that matches the code. Provider failure reasons reach you with
credential-shaped text redacted.

Limits come from the serving runtime rather than fixed caps: `max_tokens` is
bounded by the served context, ONNX batch limits apply only to ONNX, and a
requested embedding dimension is applied by Matryoshka truncation when the
provider ignores it. `/v1/models` lists each model ID once, with its profiles.

Responses may gain fields, capabilities and enum values within contract
version 1. Ignore what you do not use. See the
[model operations contract](contracts/model-operations-v1.md).

Refusals that happen before a request reaches its handler return
`{"error":{"code","message"}}`, and `request_limit` refusals name the bound.
`pumas-rpc` limits open connections (256) and the time allowed to send request
headers (30 s).

### New capabilities

- **`decision`**: typed choice, score and yes/no questions answered with
  probabilities by llama.cpp native decision models such as Kev, OpenJev and
  Laya. Needs llama.cpp **b11429** or newer.
- **`rerank`**: scores documents against a query, most relevant first, with
  an optional `top_n`, using a llama.cpp reranker (for example
  Qwen3-Reranker), an ONNX Runtime cross-encoder, or a Torch reranker from a
  local Safetensors package (for example bge-reranker-v2-m3). Served
  rerankers are also reachable at the Jina/Cohere-style `POST /v1/rerank`
  route.
- **Text embeddings on ONNX Runtime and Torch**: both compute what the model
  declares (pooling, normalization, prompts, and Dense and LayerNorm modules
  such as EmbeddingGemma's), from sentence-transformers declarations that
  Pumas records with the model at a fixed commit. Use
  `resolve_model_declarations` to record them for models already in the
  library.
- **`multimodal_embedding`**: image and mixed text/image embeddings from
  llama.cpp embedding models with a projector, such as EmbeddingGemma 2. Needs
  llama.cpp **b11454** or newer.
- **Audio input**: chat messages, embedding content items and `decision`
  input accept audio parts (raw PCM, up to 30 seconds per clip) for models
  whose runtime declares audio (verified with LFM3-Audio-2B on llama.cpp
  b12099).
  Every served model and capability now reports the modalities it accepts
  and produces, so a consumer can compose a request for a model it has no
  specific support for. See [Modalities](contracts/model-operations-v1.md#modalities).
- **`audio_transcription`**: speech to text with Cohere ASR
  (`CohereLabs/cohere-transcribe-03-2026`) served by Torch, in 14 languages and
  up to 30 seconds per request. Every Torch install includes it (Transformers
  5.4 or newer where the installed Torch allows).
- **Image-to-text**: named image and image-bearing message inputs with bounded
  sizes and typed refusals. Vision-language models published as
  `image-text-to-text` (Gemma, Qwen VL) are available for chat and
  image-to-text. See the [image-to-text contract](contracts/image-to-text-v0.8.md).

### Embedding Pumas

Local applications can discover a running Pumas owner, attach to a library
explicitly, keep that owner alive while they use it, and detect when the
library or its owner changed underneath them. Attaching never terminates
another application's owner, and a disconnect or uncertain result never
replays an inference request.

Rust consumers pin an immutable Git revision of `pumas-library`. Update the
manifest, lockfile and CI pin together, then run your own library and
lifecycle tests. HTTP consumers should check the advertised protocol and
schema versions rather than matching product versions.

### Changes for Rust consumers

- `ModelImporter::import` and `import_with_progress` (including copied
  Diffusers bundles) need the `ModelLibrary` owned by `PumasApi` or
  `PumasLibraryInstance`. A directly constructed library returns
  `PumasError::Config` before staging anything; read-only use is unchanged.
- `ModelImporter::discover_shard_recovery` (and `_async`) returns a typed,
  read-only report. `recover_incomplete_shards` is deprecated: a guessed path
  never starts a download.
- Merging libraries refuses copied imports that carry a publication receipt
  bound to their current library root; leave those in place.
- `OllamaClient::new` and `VersionManager::version_path` return `Result`.
- Removed: `network::DownloadManager` and `DownloadProgress`,
  `NetworkManager::execute`, `CacheStrategy`'s `allow_stale_on_offline` and
  `max_stale_age`, `OllamaVersionManager`, `ProcessManagerFactory` and the
  app-manager `process` module.

### Acquisition

- Local imports are bound to the digest of the file you selected.
- S3 imports of a single object or an authored bundle, optionally conditional
  on a strong ETag plus SHA-256, for any package the importer supports. These
  are tested against controlled HTTPS fixtures; specific providers (AWS, MinIO
  and others) are not yet certified.
- Hugging Face browsing and cache results stay separate from the download
  details resolved after you select a model.
- An import keeps the Hugging Face repository you name for it (`repo_id`);
  copied imports had dropped it.
- A managed GGUF keeps its selected primary file and projector through library
  moves, and a directory's primary file is never its projector.
- A credential-less `http://` source on a network host is fetched over HTTPS
  first. Pumas falls back to the plaintext URL, with a logged warning, only
  when the HTTPS attempt gets no HTTP response. Credentialed plaintext sources
  are still refused.

### llama.cpp serving

- Embedding and reranking models launch in embedding/reranking mode in
  dedicated profiles too, with a micro-batch that fits whole inputs (the served
  context, otherwise 8,192 tokens, never more than the trained context).
- Non-causal decision models such as Laya (ModernBERT) get the same
  whole-input micro-batch, so inputs longer than 512 tokens are no longer
  refused. Causal decision models such as Kev are unchanged.
- A reranker GGUF without a ranking head (`cls.output.weight`), such as one
  converted as a plain language model, is refused when served, with a reason,
  instead of returning meaningless scores. Use a GGUF converted for reranking,
  such as `ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF`.
- Image and audio input work through router profiles as well as dedicated
  ones; the router loads each model's projector and reports its modalities.
- Ollama embedding models load through `/api/embed`.

## Upgrading an existing library

Libraries from earlier versions need a one-time metadata upgrade. Read
[Opening an older library](library-upgrade.md) first. In short:

1. Stop every other app, backend and tool that uses the library.
2. Choose **Upgrade Library** on the desktop recovery screen (or use the
   documented offline command on Linux).
3. Pumas backs up `downloads.json` exactly, then converts it. Model files are
   never copied, scanned, hashed or rewritten.

The upgrade is one-way: older Pumas versions cannot open the upgraded
metadata. If anything fails, keep the metadata, backup and logs; do not restore
the backup automatically.

After upgrading, two optional steps bring older models up to date:

- `resolve_model_declarations` records sentence-transformers declarations.
  Embedders whose declarations were recorded before 0.8.0 and that use Dense
  modules (such as EmbeddingGemma) are refused by Torch until you run it.
- `backfill_library_companion_files` fetches missing model cards, licenses
  and code. Run it as a dry run first to see what it would download.

## Downloads

Sixteen files are published, for Linux x86_64, Windows x86_64 and macOS ARM64.
The standard builds include inference-plugin integration; **no-inference**
builds leave it out and only manage libraries and downloads. No build bundles
an inference runtime: llama.cpp, ONNX Runtime and Torch are downloaded from
the app or the RPC API, and each has its own setup requirements.

| Use | Linux x86_64 | Windows x86_64 | macOS ARM64 |
| --- | --- | --- | --- |
| Desktop app | `Pumas.Library-0.8.0.AppImage`, `pumas-library-electron_0.8.0_amd64.deb` | `Pumas.Library.Setup.0.8.0.exe` (installer), `Pumas.Library.0.8.0.exe` (portable) | `Pumas.Library-0.8.0-arm64.dmg` |
| Desktop app, no inference | `Pumas.Library-no-inference-0.8.0.AppImage`, `pumas-library-electron-no-inference_0.8.0_amd64.deb` | `Pumas.Library.Setup.no-inference.0.8.0.exe`, `Pumas.Library.no-inference.0.8.0.exe` | `Pumas.Library-no-inference-0.8.0-arm64.dmg` |
| Headless server | `pumas-rpc-inference-0.8.0-linux-x86_64.tar.gz` | `pumas-rpc-inference-0.8.0-windows-x86_64.zip` | `pumas-rpc-inference-0.8.0-macos-arm64.tar.gz` |
| Headless server, no inference | `pumas-rpc-no-inference-0.8.0-linux-x86_64.tar.gz` | `pumas-rpc-no-inference-0.8.0-windows-x86_64.zip` | `pumas-rpc-no-inference-0.8.0-macos-arm64.tar.gz` |

The full and no-inference installers (`.deb`, Windows setup, `.dmg`) share a
product name, so install only one of them per machine; AppImage, portable and
headless builds can sit side by side.
Headless archives include the core, so no separate core binary is needed.
`THIRD-PARTY-NOTICES-0.8.0.txt` and `pumas-library-0.8.0.provenance.jsonl`
accompany the release.

## Known limitations

- Installers are not code-signed or notarized. Windows SmartScreen and macOS
  Gatekeeper will warn on first launch.
- Audio transcription supports Cohere ASR through Torch only, one recording
  of up to 30 seconds per request. Pumas CI exercises the sidecar and gateway
  with test fixtures; the Cohere weights are gated, so the real model has not
  been run in CI.
- Speech output and video input are not available yet, even for models
  whose runtime declares them.
- An adapter with no task tag or base-model evidence (for example the LTX-2.3
  IC-LoRA) stays under image generation until you choose **Video generation**
  for it.
- A model reports only the inputs and outputs its metadata records. LTX-2.3
  shows *image → video*; it also accepts text and produces audio, which its
  published metadata does not state.
- S3 imports are not certified for specific providers.
- Rerank scores from llama.cpp use a different scale from ONNX Runtime and
  Torch for the same model. The order of the documents is the same; compare
  scores only within one provider.
- Some ONNX Runtime CUDA builds publish no usable CUDA dependency metadata
  (CUDA 12 builds from 1.30, CUDA 13 builds before 1.28). Pumas refuses those
  with the reason and names the builds that install.
- An import made through the RPC API records its Hugging Face source only
  when the caller names it (`repo_id`). Automatic identification of imported
  files is planned for 0.8.x.
- No UniFFI or Rustler host packages are published; the Python session helper
  is a reference example, not a supported SDK.

The full list of changes is in the [changelog](../CHANGELOG.md).
