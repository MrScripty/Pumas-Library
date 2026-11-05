# Pumas Library v0.8.0

Pumas Library 0.8 makes Pumas easier to build on. Applications that use Pumas
for their models get one typed API to discover what is served and run it, and
the desktop library now shows models by what they are used for.

The largest changes are:

- a typed model-operations API with model discovery (`GET /v1/served-models`)
- new `decision` and `multimodal_embedding` capabilities, and bounded
  image-to-text
- models grouped by use, with new **Decision and evaluation** and **Video
  generation** categories, input/output badges and adapter marking
- a model's category is metadata: changing it never moves a model or changes
  its ID
- local owner discovery for applications that embed Pumas
- digest-checked local imports and S3 imports
- inference-enabled headless builds for Linux, Windows and macOS
- an explicit, backed-up upgrade for older libraries

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

### Torch runtimes are add-ons you choose

Pumas no longer installs Torch from a fixed package lock. The fixed v2.9.1
preset is replaced by **Verified combinations**: the install dialog offers each
one as a shortcut (for example *cu130 · Python 3.12 · Nunchaku*), and
installing it resolves current package versions. A runtime is marked verified
only when its installed packages still match the versions that were verified.
Nunchaku Z-Image is now a regular adapter choice. Runtimes installed from the
old preset keep working.

### Clearer startup and recovery

- If the backend cannot start, the recovery screen shows the actual reason.
- Download state written by a newer Pumas no longer stops the backend: the
  library, models and inference start, downloads are paused without rewriting
  that state, and a banner explains why.
- A failed Torch startup trial says why the managed launch failed.
- Desktop views that receive an invalid update report it and reload instead of
  going stale.

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

### New capabilities

- **`decision`**: typed choice, score and yes/no questions answered with
  probabilities by llama.cpp native decision models such as Kev and OpenJev.
  Needs llama.cpp **b11429** or newer.
- **`multimodal_embedding`**: image and mixed text/image embeddings from
  llama.cpp embedding models with a projector, such as EmbeddingGemma 2. Needs
  llama.cpp **b11454** or newer.
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

### Acquisition

- Local imports are bound to the digest of the file you selected.
- S3 imports of a single object or an authored bundle, optionally conditional
  on a strong ETag plus SHA-256, for any package the importer supports. These
  are tested against controlled HTTPS fixtures; specific providers (AWS, MinIO
  and others) are not yet certified.
- Hugging Face browsing and cache results stay separate from the download
  details resolved after you select a model.
- A managed GGUF keeps its selected primary file and projector through library
  moves, and a directory's primary file is never its projector.

### llama.cpp serving

- Embedding and reranking models launch in embedding/reranking mode in
  dedicated profiles too, with a micro-batch that fits whole inputs (the served
  context, otherwise 8,192 tokens, never more than the trained context).
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

## Downloads

Sixteen files are published, for Linux x86_64, Windows x86_64 and macOS ARM64.
The standard builds include inference-plugin integration; **no-inference**
builds leave it out and only manage libraries and downloads. Inference runtimes
such as llama.cpp have their own setup requirements.

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
- Production audio-to-text (speech transcription) is not available yet.
- An adapter with no task tag or base-model evidence (for example the LTX-2.3
  IC-LoRA) stays under image generation until you choose **Video generation**
  for it.
- A model reports only the inputs and outputs its metadata records. LTX-2.3
  shows *image → video*; it also accepts text and produces audio, which its
  published metadata does not state.
- S3 imports are not certified for specific providers.
- No UniFFI or Rustler host packages are published; the Python session helper
  is a reference example, not a supported SDK.

The full list of changes is in the [changelog](../CHANGELOG.md).
