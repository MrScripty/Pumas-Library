# Linux Torch managed-runtime restart acceptance

This Linux x86_64 run installed and selected Torch 2.14.0 CPU/Core through the
native Pumas RPC backend, shut that backend down, then started a second backend
against the same launcher root. The second session confirmed the persisted
version and managed CPU profile, compared ten managed-CPython identity fields
with the first session, then invoked the persisted Torch venv interpreter to
run a fresh CPU tensor operation (`sum([1, 4, 9]) == 14`). The RPC probe endpoint
revalidated the retained install-time probe report and runtime context. The
session repeated the protocol 3 sidecar trial, stopped the sidecar by its
generation, and shut down gracefully. The fresh operation result, Torch
version, and device are recorded in `restart.cpu_operation` in
[acceptance.json](acceptance.json).

Both sessions used Pumas-provisioned CPython 3.14.7 and the pinned uv 0.12.18
with host Python, pip, PyPy, and uv absent from the backend `PATH`. The selected
Python full-archive SHA-256 matched the retained [Linux CPython license
evidence](../managed-python-license-collection/linux-x86_64-cpython-3.14.7/README.md).
This evidence is Linux-only and does not establish Windows or macOS acceptance,
CUDA execution, or Tuldok image generation.

The acceptance client allows 900 seconds for release-option discovery because
the first call provisions managed CPython before the resolver's own bounded
official-wheel scan begins. In this latest fresh-root run, the
`get_available_versions` preflight found the exact `v2.14.0` release-list
`tagName` on its first attempt, before installation. The full two-session
Linux acceptance then passed with that budget.

## Retained evidence

- [Acceptance result](acceptance.json) records both sessions and the compared
  source, catalog, provider, executable path, and executable hash.
- [Initial backend log](initial-backend-session.txt) and [restart backend log](restart-backend-session.txt)
  record the two process lifecycles.
- [Installed runtime](runtime.json), [resolution](resolution.json),
  [pip resolution](pip-resolution.json), and [runtime probes](probe-results.json)
  retain the exact installed profile, wheel/dependency artifacts, and probes.
- The selected CPython archive metadata and license texts are retained in the
  linked [Linux CPython license evidence](../managed-python-license-collection/linux-x86_64-cpython-3.14.7/README.md).
