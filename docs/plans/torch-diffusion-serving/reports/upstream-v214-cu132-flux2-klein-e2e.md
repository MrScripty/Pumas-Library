# Torch 2.14 CUDA FLUX.2 Klein and Tuldok acceptance

Date: 2026-09-26 America/Vancouver. Pumas source: `work/torch-version-management`, after `b1ff6b54` and the FP8-specific admission repair in this change. This trial used the isolated launcher root `launcher-data/cache/torch-qualification/v214-flux2-e2e-root/`, not the main desktop launcher root. Its managed runtime is upstream Torch `2.14.0+cu132`, managed Python 3.14.7, adapter `flux2`, Diffusers 0.37.0 and Transformers 4.57.6. The selected image profile was `torch-image-acceptance`.

## Why the checkpoint size was misleading

The FLUX.2 Klein checkpoint is 9,818,935,984 bytes (9.14 GiB) on disk. The installed adapter expands its scaled FP8 transformer weights to BF16 and also loads the Qwen3-8B encoder and standalone VAE. Pumas selected the library-managed FP8 Qwen3 package, whose config declares `quant_method: fp8` and 128×128 blocks. The previous unconditional 42 GiB available-RAM admission rejected this host before model load at about 39 GiB available. The repaired admission permits 38 GiB only when both the selected library record identifies FP8 and the safely resolved Qwen3 config matches. BF16 and unknown variants retain 42 GiB.

## Real bounded memory and image runs

All runs used the installed runtime and real library assets, no mocked progress or image data. The native probe is `scripts/acceptance/flux2_memory_probe.py`; the managed acceptance runner is `scripts/acceptance/flux2_v214_rpc_acceptance.py`. They require a systemd cgroup memory limit and disabled swap. The managed runner starts a fresh Pumas RPC on port zero, verifies model readiness in `/v1/models`, calls `/v1/images/generations`, runs the actual Tuldok browser test when requested, and unloads/stops task-owned processes.

| Run | Result | Cgroup peak | Limit pressure / OOM |
| --- | --- | ---: | --- |
| Native 512×512 | PNG, seed 42, four steps; SHA-256 `8351e2b416e5651d64039238a76426e4c777c054b92511648894f7799b0fcb60` | 27,054,182,400 B | 0 / 0 at 32 GiB cap |
| Native 1280×720 | PNG, seed 42; SHA-256 `dd8c7b98590851c4595f56ce432658e88beca8832d47897f1f03528601e65578` | 34,359,738,368 B | 74 / 0 at 32 GiB cap |
| Managed RPC/gateway 512×512 | Loaded and advertised image model; endpoint returned the same SHA-256 as native; unload and profile stop passed | 30,644,625,408 B | 0 / 0 at 32 GiB cap |
| Managed RPC/gateway plus Tuldok browser | 512×512 gateway request passed; Tuldok discovered, generated, displayed, saved and imported a real 1280×720 PNG | 36,507,394,048 B | 490 / 0 at 34 GiB cap |

The 1280×720 runs touched their cgroup caps and incurred memory reclaim; their peaks are **capped observations**, not uncapped workload maxima. There were no cgroup OOM or OOM-kill events. During the full Tuldok trial, the active guard measured at least 22,444,679,168 bytes (20.9 GiB) of host `MemAvailable`; the RPC and Torch sidecar were verified in the same bounded scope. The 38 GiB Pumas check is an admission heuristic for this qualified FP8 encoder path, not a memory reservation or guarantee for arbitrary image dimensions.

The real Tuldok browser requested 1280×720, seed 42, four steps and guidance 1. It reported 13.995 seconds for the browser job, displayed the decoded image at 1280×720, saved its PNG, and automatically added it to the collection. Saved PNG SHA-256: `ba32e3506ebee5a9e48c35c506777fa6b946b3ea9d7c45c86d1b6befd454750f`. Visual inspection shows the requested red teapot and yellow lemon on a blue table; the image also contains a second red vessel. Retained evidence is under `launcher-data/cache/torch-qualification/v214-flux2-e2e-root/tuldok-flux2-v214-real/{result.json,saved.png,display.png,browser.log}`. This evidence directory is local and ignored by Git.

Five focused Rust admission tests, Rust formatting, a rebuilt debug RPC binary, syntax checks for both acceptance scripts, the native image runs, managed gateway runs, and real Tuldok browser acceptance passed. Independent read-only review found no remaining blocker in the FP8 classifier or test cleanup. All task-owned RPC, sidecar and browser processes stopped after the runs.

## Desktop deployment state

The main selected `v2.14.0` runtime remains the earlier Core (`adapter: none`)
installation; it cannot serve images until replaced through the managed
installer with a FLUX.2-enabled recipe. A new local Linux AppImage and deb were
built with the repaired release RPC. The AppImage SHA-256 is
`b05e198d0a29ec5049b7afaaa9678afd41bde861e7ce34d5f7e27fabdc60711c`;
the deb SHA-256 is
`af8d0e8ae81e087806218aa254a1a28a7be656d75a04c0211c22bd7b8d0f17c6`.
Both extracted packages matched their staged RPC/resources and passed RPC
`/health` smoke tests. The packaged desktop install/serve/Tuldok flow still
needs acceptance after the main runtime is replaced. Nothing was published.
