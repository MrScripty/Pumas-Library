# Torch v2.14.0 current-source Linux CPU/Core RPC acceptance

The native RPC E2E job in [workflow run 36229508586](https://github.com/MrScripty/Pumas-Library/actions/runs/36229508586)
passed on code commit `21041697` for Linux x86_64 (Ubuntu 24.04). It discovered `v2.14.0`,
resolved and installed 25 hashed official CPU/Core artifacts, then restarted a
second backend against the same launcher root. The persisted active release and
managed Python interpreter were verified; a fresh CPU tensor operation returned
`14` with Torch `2.14.0+cpu` on device `cpu`.

The run installed Pumas-managed CPython 3.14.7 for `x86_64-unknown-linux-gnu` using the
pinned uv 0.12.18 provider and the target's official Python Build Standalone
install-only archive. The resolver probe passed, protocol 3 sidecar trial/stop
passed, shutdown was graceful, and the acceptance harness reported
`cleanup_safe: true`. This run verifies the metadata-lock follow-up on the
current runtime source. It is native RPC acceptance, not Electron UI-driven
installation or image generation through Tuldok.

## Retained evidence

- [Acceptance result](acceptance.json) records release discovery, both-session
  restart, managed-interpreter identity and hash, CPU operation, probes,
  sidecar lifecycle, and cleanup.
- [Initial backend log](initial-backend-session.txt) and
  [restart backend log](restart-backend-session.txt) record process lifecycles.
- [Installed runtime](runtime.json), [resolution](resolution.json),
  [pip resolution](pip-resolution.json), and [runtime probes](probe-results.json)
  retain the exact wheel and dependency artifacts, installed profile, and
  runtime checks.
- [Python archive manifest](managed-python-licenses/full-archive-manifest.json)
  records full-archive license hashes. The full collection's
  [README](../managed-python-license-collection/README.md) links the target's
  legal files and authoritative manifest.
