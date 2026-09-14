# Issues

| ID | Severity | Evidence / objective impact | Owner / boundary | Disposition | Verification / revisit |
| --- | --- | --- | --- | --- | --- |
| T1 | High | Existing Torch recipe targets pytorch/pytorch source releases rather than a complete serving environment | App manager / installation | Fix in M1 | Real shared-manager install, health, failed update and restart recovery |
| T2 | High | Complete compatible dependency pins and published runtime recipe source are not yet qualified | Runtime packaging | Resolve in M1 | Artifact integrity, matching wheels and GPU startup; re-plan if unavailable |
| T3 | High | Standalone checkpoint availability does not establish all matching pipeline assets | Library and Torch adapters | Resolved: both complete pipelines generate; encoder converted via Pumas | Exact component identities and successful real pipeline loads |
| T4 | High | Tuldok's existing local-model client sends VLM chat requests, not image generation | Tuldok / HTTP and browser | Fix in M3 | Real prompt, displayed image and saved decoded content |
| T5 | Medium | 24 GB VRAM is shared with other inference; full pipeline fit and latency unproven | Serving lifecycle | Measure and handle in M2/M4/M5 | Peak memory, explicit offload, OOM recovery and client deadline evidence |
| T6 | Low | LLaDA and other platforms are not qualified for this implementation | Runtime adapter owner | Defer | Revisit after primary flow acceptance and user prioritization |
| T7 | High | Pumas release API currently has six releases and no Torch runtime bundle; shared UI cannot discover the new candidate | Runtime distribution | Qualified bundle publication required for M1 acceptance; no remote publication performed | Real shared UI install after runtime qualification and publication |
| T8 | Medium | Shared install mutex previously ended when the background task was spawned; release failure could leave a phantom installing tag | App manager lifecycle | Fixed in M1 | App-manager regression suite |
| T9 | High | Torch activation previously wrote llama.cpp's `.active-version` marker | App manager selection | Fixed in M1 with a Torch-specific marker inside shared state management | Marker-preservation regression |
| T10 | High | Real Nunchaku wheel import requires libcudart.so.13; CUDA 12.8 candidate rejected before publication | Runtime recipe | Resolved with official Torch 2.9.1 CUDA 13.0; real imports, CUDA execution and protocol-1 health passed | Real imports, CUDA operation and sidecar health |
| T11 | Medium | Full base-pipeline acquisition is 30.64 GiB and current transfer is slow | Library acquisition | Completed through Pumas authority; real Nunchaku and FLUX images passed | Complete validated library bundle before model loading |

| T12 | High | Nunchaku 1.2 positional forward arguments bind to Diffusers 0.37 ControlNet fields; native FP4 forward raises TypeError | Torch adapter compatibility | Fixed by a narrow subclass preserving rotary hooks and naming Diffusers arguments; native kernel pass and focused tests passed | Resolved: complete Nunchaku pipeline and real Tuldok workflow passed |
| T13 | High | Real Serve on an already-running managed Torch profile tried to launch it again; newly launched profiles also lacked a health wait | Torch serving lifecycle | Reuse the current owned process observation; wait for owned listener and health before load | Resolved: actual release Serve and Tuldok image flow passed |

| T14 | Medium | Standalone VAE download resolves to a package directory | Torch serving asset resolution | Fixed: resolve the known VAE file within the validated package | Final release FLUX/Tuldok FP8 image passed |
| T15 | Medium | Conversion metadata omitted indexed source names and labeled non-GGUF output as GGUF | Existing conversion pipeline | Fixed: hydrate column-owned identity, label the actual format, retain source classification | Real FP8 conversion, visible final encoder names, final release build |

| T16 | Medium | Restarting the desktop during active NVFP4 setup left the previous pip process orphaned (PPid 1, own process group) | Existing Electron/backend shutdown boundary | Operationally stopped the verified task-owned orphan before either installer wrote packages; current setup continues. Broader shutdown fix deferred from conversion-format scope | Revisit desktop shutdown while dependency setup is active; root setup unit evidence alone did not cover this actual desktop path |
