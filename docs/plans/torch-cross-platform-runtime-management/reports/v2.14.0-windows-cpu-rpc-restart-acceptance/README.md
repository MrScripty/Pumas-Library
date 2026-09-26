# Windows Torch managed-runtime restart acceptance

The native `windows-2025` x64 job in manual workflow run `36214227835` at
commit `a0658131` passed the full two-session RPC acceptance for Torch
`v2.14.0` CPU/Core. The first backend installed, selected, and probed the
managed CPU profile. A second backend used the same launcher root, confirmed
the persisted version and managed CPython identity, and ran a fresh CPU tensor
operation through the persisted Torch environment (`sum([1, 4, 9]) == 14`,
Torch `2.14.0+cpu`, device `cpu`). Both sessions passed the protocol 3 sidecar
trial and generation-owned stop; the restarted backend shut down gracefully.
The release-list preflight found the exact `v2.14.0` tag on its first attempt.

The selected managed interpreter was CPython 3.14.7, provisioned by uv 0.12.18.
The [acceptance result](acceptance.json) records `success: true` and the
cross-session interpreter identity. [Initial](initial-backend-session.txt) and
[restart](restart-backend-session.txt) backend logs record the process
lifecycles. [Runtime](runtime.json), [resolution](resolution.json),
[pip resolution](pip-resolution.json), and [probe results](probe-results.json)
retain the installed profile, dependencies, and CPU probes.

The [full-archive manifest](managed-python-licenses/full-archive-manifest.json)
identifies the selected Windows CPython archive and hashes its license files.
This acceptance report retains the manifest only; license text collection is
not included here. Windows release license integration remains open. This
Windows CPU/Core result does not establish macOS acceptance, CUDA execution,
Tuldok image generation, or all-target acceptance.
