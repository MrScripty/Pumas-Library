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

The text routes accept the following request subset:

- an exact set of known fields;
- a nonblank model identifier of at most 256 characters;
- `system`, `user`, and `assistant` chat messages with nonempty string content,
  at most 256 messages, and at most 1,000,000 aggregate content characters;
- one nonempty completion prompt of at most 1,000,000 characters;
- `temperature` from 0 through 2 and `top_p` above 0 through 1, with `top_p=1`
  required when `temperature=0`;
- `max_tokens` from 1 through 4,096; and
- `stream=false` and `stop=null` only.

Unknown fields, streaming, stop sequences, other chat roles or content forms,
multiple completion prompts, and values outside those bounds are rejected.
Current response usage remains placeholder data, terminal reasons are not yet
fully truthful, and synchronous text inference does not yet have an accepted work,
cancellation, overload, or shutdown owner.

`ModelManager` owns loaded model slots and eviction. `DeviceManager` owns
device discovery and selection. Loader modules own framework-specific loading;
route handlers must not duplicate those policies.

## Install and Run

Use the Python version pinned at the repository root. From the repository root:

```bash
python3 -m venv torch-server/.venv
torch-server/.venv/bin/pip install -r torch-server/requirements.txt
torch-server/.venv/bin/pip install -r torch-server/requirements-dev.txt
torch-server/.venv/bin/python torch-server/serve.py --host 127.0.0.1 --port 8400 --max-models 4
```

These commands are for direct development only. Managed installs use no fixed
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

Loopback is the default. Binding to a non-loopback address requires both:

```bash
PUMAS_TORCH_ALLOW_LAN=1
PUMAS_TORCH_API_TOKEN=<secret>
```

All non-health routes then require the token through
`X-Pumas-Torch-Token` or a bearer authorization header. Do not expose a local
model server to an untrusted network merely because it has a token; also apply
host firewall and network controls.

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
the [current standards audit](../docs/audits/current-standards-2026-09-03/README.md).

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
`trust_remote_code=False` to both tokenizer and model loading. Downloading,
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
