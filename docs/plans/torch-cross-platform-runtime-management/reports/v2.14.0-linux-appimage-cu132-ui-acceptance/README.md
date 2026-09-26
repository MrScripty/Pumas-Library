# Torch 2.14.0 Linux AppImage UI progress acceptance

**Status:** Pass for the local Linux x86_64 v0.7.0 AppImage install flow.
This evidence covers the packaged Electron UI and its bundled backend. It does
not qualify GPU execution, image generation, or packaged Windows/macOS use.

The acceptance used the normal Torch release browser and review controls in the
rebuilt AppImage, with an isolated Pumas library root and separate XDG profile.
The window viewport was 800×1000. The previous v2.14.0 test runtime was removed
through the UI before repeating the install. The reviewed preset was cu132,
Python selected automatically, and Core runtime only; the preview reported 44
exact official artifacts with hashes.

| Claim | Result |
| --- | --- |
| Packaged artifact | Local `Pumas.Library-0.7.0.AppImage`; SHA-256 `a28f302822ce99a9d687797606574c93a5afb9c184f293526b16b972294a670b` |
| Companion package | Local `pumas-library-electron_0.7.0_amd64.deb`; SHA-256 `d8effc33ba37466171c5fae0178e2555064850e40544c93249966fedbf4bce88` |
| Target and runtime selection | Linux x86_64; Torch `v2.14.0`, cu132; Python selected automatically; 44 exact hashed artifacts resolved |
| First sampled install state | At 9 seconds elapsed, header read `Installing Torch v2.14.0 · Installing resolved wheel artifacts`; dialog showed `Working…`, `Final Setup · In progress`, and `Cancel` |
| Progress semantics | Overall and setup bars had no `aria-valuenow`; no 95% value was shown while setup progress was unmeasured |
| Cancel affordance | Visible with accessible name `Cancel current installation`; cancellation was not triggered in this completion run |
| Terminal state | `1 installed`; v2.14.0 row showed `Installed cu132 · python3.14 · none · unverified` and `Ready` |
| Installed interpreter and CPU operation | Managed CPython `3.14.7`; Torch `2.14.0+cu132`; CPU tensor sum returned `5`; CUDA unavailable on this host |
| Package smoke | Artifact naming/resource checks passed; both extracted package backends passed `/health` |
| Isolation | Test used a workspace-owned launcher root; resolver scratch was removed; the pre-test host pip-cache snapshot was unchanged |

The install used the isolated library's existing package cache. This run
records activity feedback at the first sampled UI state (9 seconds elapsed),
indeterminate setup presentation, and the terminal UI state without claiming a
cold-cache download-speed sample. Focused source tests separately verify the
pending-install state before the first progress poll. The full frontend test
suite and typecheck passed for the reviewed progress behavior.

The exact machine-readable result is [acceptance.json](acceptance.json). This is
a local candidate only and was not uploaded to the toolbar-linked public
release. The successful CPU tensor operation does not qualify CUDA device use,
Tuldok image generation, or Windows/macOS packaged installation.
