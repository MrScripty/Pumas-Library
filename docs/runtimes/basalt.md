# Managed Basalt

Pumas can download a publisher-verified [Basalt 0.5 release](https://github.com/jesdga95/basalt/releases/tag/v0.5.6), install its Python environment alongside the engine, and serve a single `.basalt` v2 pack per managed runtime profile. The Basalt build feature is enabled by default in `pumas-rpc`. Use `--no-default-features --features inference-plugins` for the other inference plugins without Basalt.

## Host requirements

- Linux x64 with Python 3.10 or newer and its venv module. Installation downloads the upstream release archive from GitHub and the release's pinned Python wheels.
- An NVIDIA Blackwell GPU of compute capability 12.0, a compatible NVIDIA driver and CUDA 13 user-space libraries. See the [Basalt platform requirements](https://github.com/jesdga95/basalt/blob/v0.5.6/docs/platforms.md).
- Enough memory to load the chosen pack. The managed launcher runs Basalt's GPU planner under the profile's CUDA device visibility before starting the server.

Pumas uses the system CUDA library search path by default. If the libraries are installed in a separate directory, start Pumas with `PUMAS_BASALT_CUDA_LIB_DIR` pointing to the absolute directory containing `libcudart.so.13`, `libcublas.so.13` and `libcublasLt.so.13`.

## Install and serve

1. Open **Basalt** in the sidebar and install the runtime. Activate it, or pin the installed version in the profile.
2. Download or import the `Inkling-2-Mini-NVFP4-DQ8.basalt` pack. Hugging Face also lists an MX8 pack supported by Basalt 0.5.6. Choose a single pack for one model record.
3. In **Runtime Profiles**, add an enabled managed Basalt profile. Select **Auto** or a specific compatible GPU. The managed process listens on loopback.
4. Serve the model with that profile. Pumas records it as loaded once the exact owned Basalt process reports this model ready. Unload stops that profile's serving process.

Basalt provides chat generation through the Pumas gateway. Available image input is declared by the loaded runtime. Use the alias and profile returned by `GET /v1/served-models`.

## Set model defaults

Ctrl+click the model name, open **Inference**, then choose a **Thinking Level** and any sampling overrides. Use **Model default** or clear a numeric field to use the pack's defaults. Save settings to apply them to subsequent requests without reloading the model.

Inkling can emit reasoning tokens even at Thinking Level `none`. Allow enough output tokens for the model's answer. See the [Inkling chat and thinking behavior](https://github.com/jesdga95/basalt/blob/v0.5.6/docs/models/inkling2.md#chat-thinking-and-tools).

API clients can read the descriptors with the `get_inference_settings` JSON-RPC method and submit the edited descriptors with `update_inference_settings`. Both take `model_id`. For an update, pass the edited descriptors as `settings`; the `default` field carries the selected per-model value. An empty settings list clears the overrides. Pumas validates values against the provider schema.

Settings include thinking level, maximum output tokens, temperature, top P, top K, min P, repetition, presence and frequency penalties, and seed. On `POST /v1/chat/completions` send explicit values in the request body. On `POST /v1/model-operations` use text-generation options, with additional values such as `reasoning_effort` under `options.sampling`. Explicit request values take precedence. See the [Pumas model operations contract](../contracts/model-operations-v1.md).

## Diagnose a launch

A missing CUDA library, unsupported GPU, or unavailable device returns a serving error before model readiness is published. If a compatible GPU is already busy, use the runtime log for model-load errors and verify memory availability. The profile status exposes the log path. To release memory, unload the model through Pumas.
