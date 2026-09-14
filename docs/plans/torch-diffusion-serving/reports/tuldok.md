# Tuldok workflow evidence

A3 passed: the actual browser discovered Nunchaku on release gateway 18767,
generated and displayed a prompt-matching 512×512 PNG in 11.047 seconds, saved
matching bytes and imported them into a collection. Evidence:
`launcher-data/cache/torch-qualification/tuldok-nunchaku-real/` contains
`display.png`, `saved.png` and `result.json`; see [Nunchaku details](nunchaku.md).

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
the mixed-provider regression is being added. Shared desktop and real model
acceptance remain pending.

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
