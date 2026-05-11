//! Shared llama.cpp serving adapter helpers.

use super::serving::serving_error;
use crate::server::AppState;
use pumas_library::models::{
    ModelServeError, ModelServeErrorCode, RuntimeDeviceMode, RuntimeDeviceSettings,
    ServeModelRequest,
};
use pumas_library::runtime_profiles::RuntimeProfileLaunchOverrides;
use pumas_library::ProviderRegistry;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
use tracing::warn;

pub(super) fn provider_request_model_id(
    request: &ServeModelRequest,
    registry: &ProviderRegistry,
) -> String {
    let library_model_id = request.model_id.trim();
    registry
        .get(request.config.provider)
        .map(|behavior| {
            behavior
                .provider_request_model_id(library_model_id, request.config.model_alias.as_deref())
        })
        .unwrap_or_else(|| library_model_id.to_string())
}

pub(super) async fn active_llama_cpp_runtime(
    state: &AppState,
    request: &ServeModelRequest,
) -> pumas_library::Result<Option<(String, PathBuf)>> {
    let Some(version_manager) = super::get_version_manager(state, "llama-cpp").await else {
        return Ok(None);
    };
    let Some(tag) = version_manager.get_active_version().await? else {
        warn!(
            model_id = %request.model_id,
            profile_id = %request.config.profile_id.as_str(),
            "No active llama.cpp runtime version is set"
        );
        return Ok(None);
    };
    let version_dir = version_manager.version_path(&tag);
    Ok(Some((tag, version_dir)))
}

pub(super) fn llama_cpp_launch_overrides(
    request: &ServeModelRequest,
) -> RuntimeProfileLaunchOverrides {
    RuntimeProfileLaunchOverrides {
        device: Some(RuntimeDeviceSettings {
            mode: request.config.device_mode,
            device_id: request.config.device_id.clone(),
            gpu_layers: request.config.gpu_layers,
            tensor_split: request.config.tensor_split.clone(),
        }),
        context_size: request.config.context_size,
    }
}

pub(super) fn llama_cpp_runtime_support_error(
    version_dir: &Path,
    request: &ServeModelRequest,
) -> Option<ModelServeError> {
    if !llama_cpp_request_needs_gpu_runtime(request)
        || llama_cpp_runtime_has_gpu_backend(version_dir)
    {
        return None;
    }
    Some(serving_error(
        ModelServeErrorCode::DeviceUnavailable,
        "selected llama.cpp profile requires GPU offload, but the active llama.cpp runtime is CPU-only; install a GPU-capable llama.cpp runtime build such as the Vulkan or ROCm archive, or choose a CPU profile",
        request,
    ))
}

fn llama_cpp_request_needs_gpu_runtime(request: &ServeModelRequest) -> bool {
    matches!(
        request.config.device_mode,
        RuntimeDeviceMode::Gpu | RuntimeDeviceMode::Hybrid | RuntimeDeviceMode::SpecificDevice
    ) || request.config.gpu_layers.is_some_and(|layers| layers != 0)
        || request
            .config
            .tensor_split
            .as_ref()
            .is_some_and(|split| !split.is_empty())
}

fn llama_cpp_runtime_has_gpu_backend(version_dir: &Path) -> bool {
    let mut stack = vec![version_dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.filter_map(std::result::Result::ok) {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if llama_cpp_backend_filename_is_gpu(entry_path.file_name()) {
                return true;
            }
        }
    }
    false
}

fn llama_cpp_backend_filename_is_gpu(file_name: Option<&OsStr>) -> bool {
    let Some(file_name) = file_name else {
        return false;
    };
    let file_name = file_name.to_string_lossy().to_ascii_lowercase();
    [
        "ggml-vulkan",
        "ggml-cuda",
        "ggml-hip",
        "ggml-rocm",
        "ggml-sycl",
        "ggml-metal",
        "ggml-kompute",
        "ggml-opencl",
    ]
    .iter()
    .any(|backend| file_name.contains(backend))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumas_library::models::{
        ModelServingConfig, RuntimeDeviceMode, RuntimeProfileId, RuntimeProviderId,
    };

    #[test]
    fn provider_request_model_id_uses_provider_behavior_policy() {
        let request = ServeModelRequest {
            model_id: "llm/qwen/model-gguf".to_string(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::LlamaCpp,
                profile_id: RuntimeProfileId::parse("llama-router").unwrap(),
                device_mode: RuntimeDeviceMode::Gpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: Some(8192),
                keep_loaded: true,
                model_alias: Some("qwen-gpu".to_string()),
            },
        };
        let registry = ProviderRegistry::builtin();

        assert_eq!(
            provider_request_model_id(&request, &registry),
            "llm/qwen/model-gguf"
        );

        let ollama_request = ServeModelRequest {
            model_id: "llm/qwen/model-gguf".to_string(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::Ollama,
                profile_id: RuntimeProfileId::parse("ollama-default").unwrap(),
                device_mode: RuntimeDeviceMode::Gpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: Some(8192),
                keep_loaded: true,
                model_alias: Some("qwen-gpu".to_string()),
            },
        };

        assert_eq!(
            provider_request_model_id(&ollama_request, &registry),
            "qwen-gpu"
        );
    }

    #[test]
    fn llama_cpp_runtime_gpu_backend_detection_requires_gpu_library() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("libggml-cpu-alderlake.so"), b"cpu").unwrap();

        assert!(!llama_cpp_runtime_has_gpu_backend(temp_dir.path()));

        std::fs::write(temp_dir.path().join("libggml-vulkan.so"), b"vulkan").unwrap();

        assert!(llama_cpp_runtime_has_gpu_backend(temp_dir.path()));
    }

    #[test]
    fn llama_cpp_runtime_support_error_rejects_gpu_profile_on_cpu_only_runtime() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("libggml-cpu-alderlake.so"), b"cpu").unwrap();
        let request = ServeModelRequest {
            model_id: "models/example.gguf".to_string(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::LlamaCpp,
                profile_id: RuntimeProfileId::parse("llama-gpu").unwrap(),
                device_mode: RuntimeDeviceMode::Gpu,
                device_id: None,
                gpu_layers: Some(-1),
                tensor_split: None,
                context_size: None,
                keep_loaded: true,
                model_alias: None,
            },
        };

        let error = llama_cpp_runtime_support_error(temp_dir.path(), &request).unwrap();

        assert_eq!(error.code, ModelServeErrorCode::DeviceUnavailable);
        assert!(error.message.contains("CPU-only"));
    }

    #[test]
    fn llama_cpp_runtime_support_error_allows_cpu_profile_on_cpu_runtime() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("libggml-cpu-alderlake.so"), b"cpu").unwrap();
        let request = ServeModelRequest {
            model_id: "models/example.gguf".to_string(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::LlamaCpp,
                profile_id: RuntimeProfileId::parse("llama-cpu").unwrap(),
                device_mode: RuntimeDeviceMode::Cpu,
                device_id: None,
                gpu_layers: Some(0),
                tensor_split: None,
                context_size: None,
                keep_loaded: true,
                model_alias: None,
            },
        };

        assert!(llama_cpp_runtime_support_error(temp_dir.path(), &request).is_none());
    }
}
