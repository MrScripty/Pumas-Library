# macOS Torch managed-runtime restart acceptance

Manual native RPC run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
on commit `07f7e9f6` passed v2.14.0 CPU/Core installation and restart on
`macos-15` arm64. The run discovered the requested release, resolved and
installed the exact official plain-version `torch-2.14.0` CPython 3.14 macOS
arm64 wheel, and retained 25 hashed artifacts for the core profile. A second
backend reopened the same launcher root, confirmed the persisted version and
managed interpreter, and ran a fresh CPU tensor operation
(`sum([1, 4, 9]) == 14`, Torch `2.14.0`, device `cpu`). The RPC probe passed,
the protocol 3 sidecar trial and generation-owned stop passed, and shutdown was
graceful.

The managed runtime used Pumas-provisioned CPython 3.14.7 from uv 0.12.18. The
first release-list preflight response was rate-limited with an explicit 913
second retry delay; the acceptance harness waited the full advertised delay,
then discovered `v2.14.0` on its second attempt within the 1800-second budget.
The [acceptance result](acceptance.json) records this retry, both sessions, and
the managed interpreter identity.

The acceptance backend ran with host Python, pip, PyPy, and uv absent from its
`PATH`, using an isolated launcher root under the runner's temporary directory.

[Initial](initial-backend-session.txt) and
[restart](restart-backend-session.txt) backend logs retain both process
lifecycles. [Runtime](runtime.json), [resolution](resolution.json),
[pip resolution](pip-resolution.json), and [probe results](probe-results.json)
retain the exact wheel URL and SHA-256, resolved dependency artifacts, installed
profile, and probes. The [CPython full-archive manifest and raw license
files](../managed-python-license-collection/macos-arm64-cpython-3.14.7/managed-python-licenses/)
retain license-file hashes. The selected install-only archive identity is in
`runtime.json` and `acceptance.json`; the manifest describes a separate full
archive. The generated release attribution now includes this target's
full-archive notice superset while keeping the selected install-only identity
separate.

This is CPU/Core RPC acceptance. It does not establish MPS acceleration,
packaged desktop installation, or Tuldok image generation.
