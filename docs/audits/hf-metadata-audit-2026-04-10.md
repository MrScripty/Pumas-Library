# Hugging Face Metadata Audit

- Generated: `2026-04-10T23:38:37.311401969+00:00`
- Sample size: `48`
- Seed: `20260410`
- Models needing review after projection: `0`

## Issue Counts

- `model-type-mismatch-with-task`: `4`

## Samples

| Repo | Search Kind | HF Pipeline | SQLite Task | SQLite Type | Issues |
|------|-------------|-------------|-------------|-------------|--------|
| `Nuwaisir/Quran_speech_recognizer` | `automatic-speech-recognition` | `automatic-speech-recognition` | `automatic-speech-recognition` | `audio` | `none` |
| `FoxBaze/Try_On_Qwen_Edit_Lora_Alpha` | `image-to-image` | `image-to-image` | `image-to-image` | `diffusion` | `none` |
| `autoevaluate/image-multi-class-classification` | `image-classification` | `image-classification` | `image-classification` | `vision` | `none` |
| `Qwen/Qwen3-Reranker-8B` | `text-ranking` | `text-ranking` | `text-ranking` | `reranker` | `none` |
| `kp-forks/DepthPro` | `depth-estimation` | `depth-estimation` | `depth-estimation` | `vision` | `none` |
| `keras-io/Image-Classification-using-EANet` | `image-classification` | `image-classification` | `image-classification` | `vision` | `none` |
| `alexgusevski/LLaMA-Mesh-q8-mlx` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `llm` | `model-type-mismatch-with-task` |
| `Gustav0-Freind/LLaMA-Mesh` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `diffusion` | `none` |
| `3DTopia/3DTopia` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `diffusion` | `none` |
| `sil-ai/swh-bible-audio-speecht5` | `text-to-audio` | `text-to-audio` | `text-to-audio` | `audio` | `none` |
| `Violetmae14/audiogen-creators` | `text-to-audio` | `text-to-audio` | `text-to-audio` | `audio` | `none` |
| `naver/trecdl22-crossencoder-rankT53b-repro` | `text-ranking` | `text-ranking` | `text-ranking` | `reranker` | `none` |
| `uwantcheats/LLaMA-Mesh` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `llm` | `model-type-mismatch-with-task` |
| `hakurei/waifu-diffusion` | `text-to-image` | `text-to-image` | `text-to-image` | `diffusion` | `none` |
| `MarcusLoren/MeshGPT-preview` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `diffusion` | `none` |
| `meta-llama/Llama-3.2-1B` | `text-generation` | `text-generation` | `text-generation` | `llm` | `none` |
| `declare-lab/tango-full-ft-audiocaps` | `text-to-audio` | `text-to-audio` | `text-to-audio` | `audio` | `none` |
| `areegtarek/siglip-nih-5` | `image-classification` | `image-classification` | `image-classification` | `vision` | `none` |
| `unsloth/Qwen-Image-Edit-2511-GGUF` | `image-to-image` | `image-to-image` | `image-to-image` | `diffusion` | `none` |
| `alexgusevski/LLaMA-Mesh-q6-mlx` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `llm` | `model-type-mismatch-with-task` |
| `FormalZz/AudioX` | `text-to-audio` | `text-to-audio` | `text-to-audio` | `audio` | `none` |
| `0xSero/gemma-4-21b-a4b-it-REAP` | `text-generation` | `text-generation` | `text-generation` | `llm` | `none` |
| `areegtarek/siglip-nih-2all` | `image-classification` | `image-classification` | `image-classification` | `vision` | `none` |
| `rwood-97/sam_os_counties` | `image-segmentation` | `image-segmentation` | `image-segmentation` | `vision` | `none` |
| `LiheYoung/depth_anything_vitl14` | `depth-estimation` | `depth-estimation` | `depth-estimation` | `vision` | `none` |
| `FaisaI/tadabur-Whisper-Small` | `automatic-speech-recognition` | `automatic-speech-recognition` | `automatic-speech-recognition` | `audio` | `none` |
| `robbyant/lingbot-depth-pretrain-vitl-14-v0.5` | `depth-estimation` | `depth-estimation` | `depth-estimation` | `vision` | `none` |
| `SenseTime/deformable-detr-single-scale-dc5` | `object-detection` | `object-detection` | `object-detection` | `vision` | `none` |
| `openai/whisper-large-v3-turbo` | `automatic-speech-recognition` | `automatic-speech-recognition` | `automatic-speech-recognition` | `audio` | `none` |
| `jobs-git/Hunyuan3D-1` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `diffusion` | `none` |
| `Systran/faster-whisper-large-v3` | `automatic-speech-recognition` | `automatic-speech-recognition` | `automatic-speech-recognition` | `audio` | `none` |
| `philschmid/stable-diffusion-2-inpainting-endpoint` | `image-to-image` | `image-to-image` | `image-to-image` | `diffusion` | `none` |
| `facebook/audio-magnet-small` | `text-to-audio` | `text-to-audio` | `text-to-audio` | `audio` | `none` |
| `samitizerxu/segformer-b0-finetuned-segments-sidewalk-oct-22` | `image-segmentation` | `image-segmentation` | `image-segmentation` | `vision` | `none` |
| `Shakker-Labs/FLUX.1-dev-LoRA-Logo-Design` | `text-to-image` | `text-to-image` | `text-to-image` | `diffusion` | `none` |
| `sam1120/safety-utcustom-terrain` | `image-segmentation` | `image-segmentation` | `image-segmentation` | `vision` | `none` |
| `samitizerxu/segformer-b0-finetuned-kelp-segments-jan-18-10am` | `image-segmentation` | `image-segmentation` | `image-segmentation` | `vision` | `none` |
| `liuwenhan/RankMistral100` | `text-ranking` | `text-ranking` | `text-ranking` | `reranker` | `none` |
| `NicolasG2523/hunyuan3d-dit-v2-0-endpoint` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `diffusion` | `none` |
| `Orenguteng/Llama-3.1-8B-Lexi-Uncensored-V2` | `text-generation` | `text-generation` | `text-generation` | `llm` | `none` |
| `trackdr/LLaMA-Mesh-Q4_K_M-GGUF` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `diffusion` | `none` |
| `mistralai/Mistral-7B-Instruct-v0.2` | `text-generation` | `text-generation` | `text-generation` | `llm` | `none` |
| `Zhengyi/LLaMA-Mesh` | `text-to-3d` | `text-to-3d` | `text-to-3d` | `llm` | `model-type-mismatch-with-task` |
| `ShinoharaHare/Waifu-Inpaint-XL` | `image-to-image` | `image-to-image` | `image-to-image` | `diffusion` | `none` |
| `ogkalu/Comic-Diffusion` | `text-to-image` | `text-to-image` | `text-to-image` | `diffusion` | `none` |
| `kp-forks/InstantMesh` | `image-to-3d` | `image-to-3d` | `image-to-3d` | `diffusion` | `none` |
| `ibm-granite/granite-speech-3.3-2b` | `automatic-speech-recognition` | `automatic-speech-recognition` | `automatic-speech-recognition` | `audio` | `none` |
| `NhatPham/vit-base-patch16-224-recylce-ft` | `image-classification` | `image-classification` | `image-classification` | `vision` | `none` |

## Detailed Findings

### `alexgusevski/LLaMA-Mesh-q8-mlx`

- Search plan: `text-to-3d` query=`mesh` offset=`18`
- Search kind: `text-to-3d`
- HF pipeline tag: `text-to-3d`
- Effective pipeline tag: `text-to-3d`
- SQLite task/type: `text-to-3d` / `llm`
- Input/output modalities: `["text"]` -> `["3d"]`
- Issues: `model-type-mismatch-with-task`
- Review reasons: `model-type-low-confidence`

### `uwantcheats/LLaMA-Mesh`

- Search plan: `text-to-3d` query=`mesh` offset=`26`
- Search kind: `text-to-3d`
- HF pipeline tag: `text-to-3d`
- Effective pipeline tag: `text-to-3d`
- SQLite task/type: `text-to-3d` / `llm`
- Input/output modalities: `["text"]` -> `["3d"]`
- Issues: `model-type-mismatch-with-task`
- Review reasons: `model-type-low-confidence`

### `alexgusevski/LLaMA-Mesh-q6-mlx`

- Search plan: `text-to-3d` query=`mesh` offset=`17`
- Search kind: `text-to-3d`
- HF pipeline tag: `text-to-3d`
- Effective pipeline tag: `text-to-3d`
- SQLite task/type: `text-to-3d` / `llm`
- Input/output modalities: `["text"]` -> `["3d"]`
- Issues: `model-type-mismatch-with-task`
- Review reasons: `model-type-low-confidence`

### `Zhengyi/LLaMA-Mesh`

- Search plan: `text-to-3d` query=`mesh` offset=`12`
- Search kind: `text-to-3d`
- HF pipeline tag: `text-to-3d`
- Effective pipeline tag: `text-to-3d`
- SQLite task/type: `text-to-3d` / `llm`
- Input/output modalities: `["text"]` -> `["3d"]`
- Issues: `model-type-mismatch-with-task`
- Review reasons: `model-type-low-confidence`
