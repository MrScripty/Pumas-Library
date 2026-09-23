# Packaged upstream Torch acceptance

Date: 2026-09-23
Source commit: `5d373bf8791c43f1b995f4ba51a17da1610a6686`
Packaged artifact: `Pumas.Library-0.7.0.AppImage`
SHA-256: `74d6c6143c5342a9492f7afbc30b301f751f870aca04ec05f231ff6ee140f71c`

The Linux x86_64 AppImage was built from the exact source commit above with a
temporary output-path override. The app ran with an explicit isolated
`PUMAS_LAUNCHER_ROOT` and a separate Electron profile. Tests used a fresh root
and a second disposable root seeded with a physical copy of the host's installed
`torch-runtime-0.1.4`; neither root was the host's active Pumas library.

## Discovery and install

The packaged Torch globe fetched 69 releases from `pytorch/pytorch`; the shared
version manager displayed `v2.9.1` as the only installable release because it is
the only upstream tag mapped to a qualified app recipe. No Pumas Torch release
or tag was used. The UI showed the v2.9.1 release date and the install action.

Installing from that row completed in the packaged app. The registered metadata
identified `releaseTag: v2.9.1`, path `v2.9.1`, Python 3.12.3, and
`dependenciesInstalled: true`. The direct Torch wheel came from the official
PyTorch CUDA 13.0 wheel host and the recipe identity was
`torch-upstream-2.9.1-r1`. The install log reported the locked packages,
including `torch-2.9.1+cu130`, `torchvision-0.24.1+cu130`, and
`nunchaku-1.2.0+torch2.9`.

The packaged manager's install validation started the embedded sidecar and
reported recipe `torch-upstream-2.9.1-r1`, Torch `2.9.1+cu130`, CUDA `13.0`,
GPU `NVIDIA GeForce RTX 5090 Laptop GPU`, and health
`{"status":"ok","protocol":3,"capabilities":["image_generation"]}`. A
managed profile started from the shared Torch UI; its `/health` endpoint
returned HTTP 200 with protocol 3 and `image_generation`. The profile stopped
through the UI. Restarting the app preserved the installed version and left the
profile stopped.

## Legacy-runtime migration, cancellation, removal, and restart

In the second isolated root, the copied `torch-runtime-0.1.4` was selected and
usable before the update attempt. Starting its managed profile through the UI
returned HTTP 200 from `/health` with protocol 1 on the same RTX 5090 Laptop
GPU. The profile was stopped before changing versions.

The shared UI began installing v2.9.1, reached locked dependency setup, and then
the Cancel installation confirmation was accepted from the in-progress release
row. The manager returned to its version list with only
`torch-runtime-0.1.4` installed. The staging directory was removed; the Torch
metadata still contained only `.1.4`, `lastSelectedVersion` and the active marker
remained `.1.4`, and `defaultVersion` remained unset. No installer or sidecar
process remained. After an app restart, the UI still showed `.1.4` as installed
and v2.9.1 as the available release.

The same root then completed a second v2.9.1 installation through the shared UI.
With both versions present, the selector switched active runtime to v2.9.1 and
the managed profile started healthy on the RTX 5090. After stopping the profile,
the version manager removed the now-inactive copied `.1.4` runtime. The manager
log recorded removal of its directory. After another app restart, the UI showed
v2.9.1 installed and the profile stopped; metadata contained only v2.9.1, the
active marker and `lastSelectedVersion` were v2.9.1, and the default remained
unset.

## Result and limits

A1 passed for the currently qualified upstream recipe: packaged discovery,
shared-UI install, migration from the previously installed runtime, activation,
removal, cancellation preservation, sidecar health, and restart recovery were
observed. The host's installed versions (`0.1.1`–`0.1.4`), active `.1.4`, and
unset default remained unchanged. No runtime was published or tagged, and no
default runtime was changed.

Only `v2.9.1` currently has a qualified upstream recipe, so this evidence does
not claim an upgrade between two different PyTorch upstream tags. A subsequent
recipe can be tested when another upstream version is qualified.

## Concurrent version selection and removal

The packaged trace above exercises selection and removal sequentially. A source
review then identified that concurrent shared-manager requests could change
active or default selection after removal had checked its target but before its
filesystem and cached-state cleanup completed. `VersionManager` now serializes
active/default selection and removal with a dedicated lifecycle mutex; these
selection changes do not wait on the installation mutex held during downloads.

Deterministic Torch manager regressions verify removal-first active selection
waits and then rejects the deleted tag. The sequential switch-first check makes
removal refuse and preserve the active installation. Another test pauses after
metadata deletion and before state refresh: setting the removed tag as default
waits, then returns `VersionNotFound`. Default-first removal clears the default.
The tests verify runtime files, Torch metadata, `.active-version-torch`, default
selection, and reconstructed manager state. They passed in the changed source
on 2026-09-23.
The AppImage above was built from the prior source commit and does not contain
this additional synchronization fix; the source-level regressions cover the
concurrency behavior.
