# Torch Inference Sidecar

The Torch sidecar is a provider process managed by Pumas, not a public
endpoint. External clients must use the Pumas `/v1` gateway
([public image contract](../docs/contracts/image-generation.md)); the
sidecar's own routes are private and defined by the
[Torch provider protocol](../docs/contracts/torch-provider-protocol.md).
Pumas installs Torch into a managed Python environment from official pinned
wheels; no Pumas-hosted runtime bundle is published (see
[Install and Run](#install-and-run)). Accepting a runtime for a platform
requires a resolved production dependency set and real ASGI, model-loader,
generation, device, responsiveness, and shutdown evidence.

## HTTP Surface

- `GET /v1/models`: OpenAI-shaped model-list response
- `POST /v1/chat/completions`: text-only, non-streaming chat request subset
- `POST /v1/completions`: single-text, non-streaming completion request subset
- `POST /api/images/generate`: private single-PNG generation through the
  concrete diffusion adapters
  ([provider protocol](../docs/contracts/torch-provider-protocol.md));
  Nunchaku and Klein with an FP8 text encoder
  passed real Tuldok generation, display and save
- `/api/*`: Pumas load, unload, status, and device controls
- `/health`: process health with the provider protocol version and capabilities

Protocol 3 image generation is connection-bounded and duration-unbounded after
admission. Elapsed time and response silence do not fail valid work. Disconnect
requests cancellation, while the sidecar keeps worker and device custody until
cleanup actually finishes; a lost response remains uncertain and is never
automatically replayed. See the shared
[generation lifetime](../docs/contracts/generation-lifetime.md).

Image admission happens under the device lease, before any denoising. Each
adapter declares the largest output its recipe supports (`max_pixels`) and its
peak device memory per output pixel. A request above the adapter's limit is
refused with `400 dimensions_exceed_limit` (its detail carries
`max_pixels`); a request whose pixels exceed free
GPU memory (`torch.cuda.mem_get_info`) divided by that per-pixel estimate is
refused with `507 out_of_memory`. Both reasons name the limit and the value.

The text routes accept the following request subset:

- an exact set of known fields;
- a nonblank model identifier of at most 256 characters;
- `system`, `user`, and `assistant` chat messages with nonempty string content,
  at most 256 messages, and at most 1,000,000 aggregate content characters;
- one nonempty completion prompt of at most 1,000,000 characters;
- `temperature` from 0 through 2 and `top_p` above 0 through 1, with `top_p=1`
  required when `temperature=0`;
- `max_tokens` of at least 1; and
- `stream=false` and `stop=null` only.

Unknown fields, streaming, stop sequences, other chat roles or content forms,
multiple completion prompts, and values outside those bounds are rejected.
The character and message counts are request-body safety bounds, not model
limits. The model limit is the loaded model's own context: the tokenized prompt
plus `max_tokens` must fit its `max_position_embeddings` (or the equivalent
config field). A request that does not fit is refused with
`400 context_length_exceeded` naming both lengths; input is never truncated.

Text generation runs on a worker thread under the slot's device lease, the
same lease image generation and loading use, so `/health` and the control
routes keep answering during a long generation. Work queued behind a held lease
is refused with `409 runtime_busy`. A client disconnect or owner cancellation
stops decoding at the next token through a stopping criterion; the lease is
released only after the worker returns (`499 cancelled`). CUDA out-of-memory
empties the allocator cache and is refused with `507 out_of_memory`.
Response usage remains placeholder data and terminal reasons are not yet fully
truthful.

Refusals from the text, image and control routes carry
`detail: {"code": ..., "message": ...}`. The codes are stable; the messages are
written for the caller. Library error text, which can carry resolved paths,
is written to the runtime log, and the response says only that loading or
generation failed. Control codes: `slot_limit_reached`, `slot_limit_conflict`,
`slot_loading`, `runtime_busy`, `owned_model`, `slot_not_found`,
`model_rejected` (a sidecar refusal with its reason), `model_unloadable` (a
library refusal), `out_of_memory`, `cancelled` and `internal_error`.

`/api/load` accepts `model_type` only from the implemented set
(`text-generation`, `dllm`, `sherry`, `cohere-asr`, `nunchaku-z-image-turbo`,
`flux2-klein-9b-kv-fp8`); omitting it detects a text type from `config.json`.
`pipeline_path` is required by, and only accepted with, an image adapter, and
`vae_path` only with Klein. `model_path`, `pipeline_path` and `vae_path` are
resolved (symlinks followed) and must sit inside a root listed in
`PUMAS_TORCH_MODEL_ROOTS`. Before Z-Image is loaded, every `(library, class)`
pair in its `model_index.json` is checked against the adapter's allowlist,
because Diffusers imports and instantiates whatever the index names.

`ModelManager` owns loaded model slots and eviction. `DeviceManager` owns
device discovery and selection. Loader modules own framework-specific loading;
route handlers must not duplicate those policies.

## Install and Run

Use the Python version pinned at the repository root. From the repository root:

```bash
python3 -m venv torch-server/.venv
torch-server/.venv/bin/pip install -r torch-server/requirements.txt
torch-server/.venv/bin/pip install --require-hashes -r torch-server/requirements-dev.lock
torch-server/.venv/bin/python torch-server/serve.py --host 127.0.0.1 --port 8400 --max-models 4
```

These commands are for direct development only. `requirements.txt` is an
unpinned list of the runtime packages (Torch and its serving stack) and is never
shipped. `requirements-dev.lock` is the hash-locked set CI installs for lint and
unit tests; the unit suite needs only that lock. Managed installs use no fixed
package lock: each resolves current package versions when it is installed and
records the exact versions and hashes it received. Pumas does not choose those
versions for the user, so it does not own their advisories.
`runtime/verified-runtimes.json` records combinations Pumas has verified, with
the package versions present at the time. It is evidence, never an install
constraint: the install dialog offers each record as a shortcut, and an
installed runtime is reported as verified only when its packages still carry
the recorded versions. A Torch release does not
declare one Python version for every install: its official wheels declare
compatible Python ABI and platform tags. For an upstream release, Pumas
provisions the newest stable native CPython candidate from its pinned provider
catalog, checks the exact official Torch wheel and complete dependency set, then
uses an older candidate only when the newer one is proven incompatible. Pumas
does not require Python on the host or a Python choice from the user. CPython
3.10 is the sidecar's minimum; there is no configured upper minor-version cap.
Provider, network, or incomplete-scan failures remain inconclusive and do not
silently select an older interpreter. The manager records the selected
distribution source, full version, target, provider pin, executable
fingerprint, exact wheel URLs, hashes, and package versions with the installed
runtime. Installation uses official binary wheels only and never falls back to
a source build.

Native managed-Python provisioning, Torch CPU/Core installation, restart,
and sidecar lifecycle passed on Linux x86_64, Windows x64, and macOS arm64
for source `21041697` in
[run 36229508586](https://github.com/MrScripty/Pumas-Library/actions/runs/36229508586).
That historical result does not qualify subsequent source changes, packaged
Electron installation, CUDA/MPS device use, or image adapters. The
[cross-platform runtime plan](../docs/plans/torch-cross-platform-runtime-management/plan.md)
retains those acceptance gates and the provider-attribution follow-up.
The sidecar is embedded in the Pumas app,
so no Pumas-hosted Torch release bundle is required. Adapter dependencies are
outside the core automatic install; an unavailable adapter affects its feature,
not the installation of Torch itself.

Managed installation downloads official binary wheels only after the user
selects a supported upstream release. Model weights remain library-owned.
Existing installed `0.1.x` entries remain available.

Version rules:

- Release discovery lists stable `vMAJOR.MINOR.PATCH` tags from
  `pytorch/pytorch`; prereleases and nightlies are excluded. A failed page, or
  running past 20 pages or 120 seconds, is an error, never a partial list.
  Installed versions stay inspectable offline.
- Installing a version does not activate it. Selecting, setting the default,
  starting and stopping are separate explicit operations.
- One Torch version serves at a time. Unload its models and stop the Torch
  process before changing its runtime.
- Nunchaku is installed from its project's `1.2.0+torch2.9` wheel, which is
  built only for Torch 2.9.1, CUDA 13.0, CPython 3.12 and Linux x86_64, so the
  `nunchaku` adapter installs only there. Runtimes installed by the former
  fixed `v2.9.1` preset keep working and are reported as verified.
- XPU builds are not offered, and Torch is never built from source.

Not yet supported: native NVFP4 encoder serving, LLaDA, image adapters outside
Linux, and upgrading in place between two upstream Torch tags.

## Network Safety

When Pumas launches the sidecar it sets, through the environment and never
argv:

- `PUMAS_TORCH_API_TOKEN`: a random token generated once per Pumas backend
  process and never persisted. All routes except `/health` then require it
  through `X-Pumas-Torch-Token` or a bearer authorization header; a request
  without it is refused with `401 unauthorized`. Pumas sends it only to a
  sidecar it launched and still owns, never to another server (such as one
  named by a caller's `connection_url`); its Torch clients never follow
  redirects and bypass proxies for loopback servers.
  The managed token replaces any `PUMAS_TORCH_API_TOKEN` in the user's own
  environment.
- `PUMAS_TORCH_MODEL_ROOTS`: the library's model root plus the directory of
  every external-reference model the library knows about at launch. Model and
  component paths outside them are refused. The set is read at launch: a model
  imported by external reference while Torch runs is refused, with that
  reason, until the Torch profile is restarted.
- `PUMAS_TORCH_PARENT_PID`: the launching process. A watchdog thread polls its
  identity (pid plus start time) once a second; when the launcher is gone the
  server stops itself, then exits forcibly if in-flight workers have not
  finished within 10 seconds. Polling is used instead of `PR_SET_PDEATHSIG`,
  which fires when the spawning thread exits rather than the process.

A direct development run without these variables has no token, no root
confinement and no watchdog, and stays on loopback. Binding to a non-loopback
address requires both:

```bash
PUMAS_TORCH_ALLOW_LAN=1
PUMAS_TORCH_API_TOKEN=<secret>
```

Do not expose a local model server to an untrusted network merely because it
has a token; also apply host firewall and network controls.

## Verification

```bash
python3 -m ruff check torch-server
python3 -m ruff format --check torch-server
python3 -m unittest discover -s torch-server/tests
```

The unit suite can install local fakes for missing Torch/runtime dependencies.
Those tests prove focused request and control logic only. They do not prove
ASGI middleware, production dependency resolution, device discovery, model
loading, inference, control responsiveness during inference, disconnect or
cancellation behavior, or bounded shutdown. The Linux sm_120 Nunchaku and Klein/FP8 workflows
have separate real release/GPU/browser evidence, recorded in the completed
diffusion serving plan (`git show 78c52fcf:docs/plans/torch-diffusion-serving/`).

The managed runtime lock is the production dependency candidate; the root
`requirements.txt` remains a direct-development input. Keep qualification,
vulnerability and license evidence attached to the exact managed lock. See [Releasing](../RELEASING.md), [Security](../docs/SECURITY.md), and
the [open desktop, release and Torch issues](../docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/issues.md).

Native compatibility checks (using a qualified managed Python environment):

```bash
torch-versions/torch-runtime-0.1.2/venv/bin/python -I -m unittest discover -s torch-server/tests_native
```

The Z-Image adapter includes a narrow Nunchaku 1.2/Diffusers 0.37 forward-call
compatibility boundary. It keeps packed rotary hooks and passes patch dimensions
by name. Native dependency imports alone do not establish this compatibility;
checkpoint kernel and full pipeline qualification were recorded in the completed
diffusion serving plan.

## Model-supplied Python code

Text loaders (standard Hugging Face, Sherry, and DLLM) explicitly pass
`trust_remote_code=False` and `local_files_only=True` to both tokenizer and
model loading, and `use_safetensors=True` to model loading. A text model
without `.safetensors` weights is refused with that reason instead of loading
a pickle checkpoint. Downloading,
importing, or selecting a model does not authorize executing Python supplied by
that model repository. [Transformers documents custom-model loading as a separate
trust decision](https://huggingface.co/docs/transformers/custom_models).

Models supported by the installed framework implementations remain eligible.
Models that require repository-supplied code now fail through the normal model
load error path; the server does not retry with elevated trust or prompt in the
background. This includes custom-code-only Sherry or DLLM models. There is no
global environment, command-line, or request toggle to bypass this boundary.

Supporting such models later requires a reviewed approval contract bound to an
immutable code revision/content identity, its provenance and execution scope,
with invalidation when the code changes. That contract is not implemented yet.
This restriction is not a general sandbox for model files, framework dependencies,
or native extensions; existing artifact validation and runtime isolation still
matter.
