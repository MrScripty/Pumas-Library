# Tuldok workflow evidence

A3 passed: the actual browser discovered Nunchaku on release gateway 18767,
generated and displayed a prompt-matching 512×512 PNG in 11.047 seconds, saved
matching bytes and imported them into a collection. Evidence:
`launcher-data/cache/torch-qualification/tuldok-nunchaku-real/` contains
`display.png`, `saved.png` and `result.json`; see [Nunchaku details](nunchaku.md).

## Corrected-candidate 1280×720 acceptance

TIPC-10 passed separately against refreshed Tuldok
`a61daeec83779868cf03b14fb5c811fcdc608fc2`, Pumas
`ab3a95a9369a5471127003fcd8e6fff529596cb5`, and production-installed exact
`torch-runtime-0.1.6` archive SHA-256
`74f9b593dff1e447a73571dc465efd520238ba0c56d2730393a17eee2b97426c`.

The real headless browser discovered
`diffusion/nunchaku-ai/nunchaku-z-image-turbo`, submitted the red ceramic
teapot/yellow lemon/blue table prompt at 1280×720 with seed 42, displayed the
result at 1280×720, saved the exact response PNG, and automatically added it to
the temporary collection. Browser elapsed time was 10.036 seconds; runtime
generation reported 9.553 seconds, eight steps, guidance zero, and sequential
CPU offload. Saved PNG SHA-256 is
`cdf9ae632f621606032d91975f86f960d546b39349a83ce6b851b50390fb52a3`.
Visual inspection confirmed the prompt. Evidence:
`launcher-data/cache/torch-qualification/tipc-0.1.6-tuldok-real/` contains
`display.png`, `saved.png`, and `result.json`. This new result does not relabel
the earlier 512×512 evidence.

Implemented a separate Generate images workspace. Discovery admits only models with
`image_generation`; the gateway URL is independent of the existing VLM settings.
Requests validate prompt, dimensions and seed, permit one operation, never retry,
and propagate cancellation/disconnect. Responses must decode as bounded PNGs with
matching dimensions before display. Save PNG preserves response bytes; Add to
collection uses the existing dataset source-image boundary.

Verification on 2026-09-13:

- Python suite: 21 passed, including transport, response validation, cancellation,
  and existing corner detection behavior.
- New real-browser fixture: discovered the image model while excluding a VLM;
  displayed 512×512, saved matching bytes and imported matching source content;
  malformed output preserved the previous image; cancellation closed the backend
  request; the following request succeeded; four actions made exactly four requests.
- Existing `node tests/browser.cjs`: passed camera, timer, labels, validation,
  corner dragging, metadata, export/import, responsive layout, all three AI
  providers and unsaved suggestions.
- Visually inspected the new browser screenshot. The fixture image is synthetic
  transport evidence and does not establish model inference or prompt relevance.

Evidence under `launcher-data/cache/torch-qualification/`:
`tuldok-tests.log`, `tuldok-browser-images.log`,
`tuldok-vlm-browser-regression.log`, and `tuldok-browser-fixture/`.
Fixture saved PNG SHA-256:
`2cea36f05c163fbbb9d1ce96d01f61b7079ff7d394fd38724112fa4095be04a8`.

Pumas shared provider declarations now admit Torch profiles and mixed Torch/llama
serving status. The Torch panel uses shared runtime profiles and exposes the
advertised Pumas gateway URL. Type checking and 58 existing serving tests passed;
the mixed-provider regression was being added. At that source-only checkpoint,
shared desktop and real model acceptance remained pending.

## Real VLM regression

The real Tuldok browser passed corner suggestion and save through the M2 Pumas
release gateway `http://127.0.0.1:18767/v1`. Model:
`vlm/qwen35/huihui-ai--huihui-qwen3_8-27b-abliterated-gguf__files_c9865f62d5e8`,
llama.cpp `b10883+vulkan`, separate managed profile `torch-acceptance-vlm`,
4096 context, GPU layers -1. An existing book photo was imported into a temporary
dataset; the user's source dataset was unchanged. In 80.512 seconds, the VLM
identified the book and returned four visible corners. The browser displayed an
unsaved suggestion, then saved it with `llamacpp` and exact model provenance.
Screenshot and JSON: `tuldok-vlm-real/`; log: `tuldok-vlm-real.log`.

Earlier attempts are retained as failures: the 9B package lacked an image
projector, and an IQ4_KS package used unsupported GGML type 133. A subsequent
profile cleanup initially returned an internal error; the following managed
launch succeeded. No model files or user runtime profiles were altered to work
around these failures.

## FLUX.2 with Pumas-converted FP8 encoder — A4 passed

The real browser flow passed against release
`b11742c95e7dc68d82abd790618a6cba9e012dbc95e7c8ad8cb60f9165b45b74`
and gateway `http://127.0.0.1:38035/v1`: capability discovery, prompt submission,
512x512 display, PNG save and exact-byte collection import. Generation took
8.843 seconds. Evidence: `tuldok-flux2-fp8-real/` under the qualification cache.
Both image models remain served. Existing VLM and library-only evidence was
retained rather than expanding verification contrary to the user's instruction.
