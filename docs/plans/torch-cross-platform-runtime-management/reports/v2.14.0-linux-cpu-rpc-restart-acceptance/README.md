# Linux Torch managed-runtime restart acceptance

Manual native RPC run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
on commit `07f7e9f6` passed v2.14.0 CPU/Core installation and restart on
Ubuntu 24.04 x86_64. The run discovered the requested release, resolved and
installed the exact official `torch-2.14.0+cpu` CPython 3.14 Linux wheel, and
retained 25 hashed artifacts for the core profile. A second backend reopened
the same launcher root, confirmed the persisted version and managed interpreter,
and ran a fresh CPU tensor operation (`sum([1, 4, 9]) == 14`, Torch
`2.14.0+cpu`, device `cpu`). The RPC probe passed, the protocol 3 sidecar trial
and generation-owned stop passed, and shutdown was graceful.

The managed runtime used Pumas-provisioned CPython 3.14.7 from uv 0.12.18 with
host Python, pip, PyPy, and uv absent from the backend `PATH`. The
[acceptance result](acceptance.json) records both sessions and the compared
source, catalog, provider, executable path, and executable hash. The release-list
preflight found `v2.14.0` on its first attempt. The selected CPython archive and
license evidence are linked in the [Linux CPython license
report](../managed-python-license-collection/linux-x86_64-cpython-3.14.7/README.md).

## Retained evidence

- [Initial backend log](initial-backend-session.txt) and [restart backend log](restart-backend-session.txt)
  record both process lifecycles.
- [Installed runtime](runtime.json), [resolution](resolution.json),
  [pip resolution](pip-resolution.json), and [runtime probes](probe-results.json)
  retain the exact official Torch wheel URL and SHA-256, all resolved dependency
  artifacts, installed profile, and probes.
- The [full-archive manifest and raw CPython license files](../managed-python-license-collection/linux-x86_64-cpython-3.14.7/managed-python-licenses/)
  record the license texts and hashes for this target. The selected install-only
  archive identity remains in `runtime.json` and `acceptance.json`.

This is CPU/Core RPC acceptance. It does not establish CUDA execution,
packaged desktop installation, or Tuldok image generation.
