# Torch Inference Sidecar

The Torch sidecar is a provider process managed by Pumas, not a public
endpoint. External clients must use the Pumas `/v1` gateway
([public image contract](../docs/contracts/image-generation.md)); the
sidecar's own routes are private and defined by the
[Torch provider protocol](../docs/contracts/torch-provider-protocol.md).
A managed runtime bundle is being qualified; it is not yet a published
or accepted Pumas runtime.
Promotion requires a resolved production dependency set and real ASGI,
model-loader, generation, device, responsiveness, and shutdown evidence.

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

These commands are for direct development only. Managed installations use
`runtime/requirements.lock`, which pins dependencies and artifact hashes. The
shared VersionManager discovers upstream releases from `pytorch/pytorch`; it
offers only tags with a qualified Pumas recipe. The current recipe maps upstream
`v2.9.1` to Torch `2.9.1+cu130` and torchvision `0.24.1+cu130`, using their
official hash-pinned wheels. The sidecar and lock are embedded in the Pumas app,
so no Pumas-hosted Torch release bundle is required. The recipe supports Linux
x86_64, CPython 3.12 and NVIDIA sm_120 only.

Create candidate assets with:

```bash
python3 scripts/package-torch-runtime.py --output /tmp/pumas-torch-assets
```

This script creates the local qualification archive used by earlier candidate
tests; the shared manager no longer downloads that archive. Managed installation
downloads the official pinned wheels and other hash-locked dependencies only
after the user selects a supported upstream release. Model weights remain
library-owned. Candidate `torch-runtime-0.1.6` remains unpublished and is not the
default runtime. See the [active plan](../docs/plans/torch-diffusion-serving/plan.md).
Existing installed `0.1.x` entries remain available. Stop a running Torch
process before changing its runtime.

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
have separate real release/GPU/browser evidence in the active plan.

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
checkpoint kernel and full pipeline qualification are recorded in the plan.
