# Torch 2.14.0 packaged Linux CPU/Core acceptance

**Status:** Pass for the packaged Linux RPC backend on commit `21041697`.
This exercises the backend packaged into the local v0.7.0 AppImage/deb build;
it does not automate or claim full Electron UI interaction.

The acceptance harness ran against
`electron/release/linux-unpacked/resources/pumas-rpc`. `smoke-linux-packages.py`
verified that the AppImage and deb contain the same backend bytes and release
notices before this run.

| Item | Result |
| --- | --- |
| Release/build | Torch `v2.14.0`, official CPU wheel profile |
| Host target | Linux x86_64 GNU |
| Managed interpreter | CPython `3.14.7`, provisioned by pinned uv `0.12.18`; backend `PATH` was cleared |
| Official wheel/dependency artifacts | 25 SHA-256-verified artifacts |
| Installed runtime | `2.14.0+cpu`; CPU tensor result `14` after restart |
| Resolver probe | Passed |
| Sidecar | Protocol 3 trial, generation-owned stop, and restart passed |
| Shutdown/cleanup | Graceful shutdown; disposable launcher root removed safely |

The exact retained result is [acceptance.json](acceptance.json). The packaged
backend SHA-256 is
`1b42b6cbedc0bf842990a895cf759537f60ba1564e054a8c8b5f7f022e4a2114`.
The local AppImage SHA-256 is
`1398ef0a9da1c0aab90681d3c91674ef88c6229a84984047938e7bd6eb350acd`; the deb
SHA-256 is
`468b6f7c2af00ff8785f80e5486cd5133979e875dd351b4b9f5284ddfd195043`.

This CPU/Core result does not qualify CUDA, macOS MPS, v2.14.0 image generation
through Tuldok, packaged UI-driven installation, or packaged Windows/macOS
installations. These locally built v0.7.0 artifacts have not been published and
do not change the toolbar-linked public release.
