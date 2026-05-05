//! Provider-neutral runtime profile service contracts.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::{
    RuntimeDeviceMode, RuntimeProfileConfig, RuntimeProviderId, RuntimeProviderMode,
};
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProviderCapabilities {
    pub provider: RuntimeProviderId,
    pub provider_modes: Vec<RuntimeProviderMode>,
    pub device_modes: Vec<RuntimeDeviceMode>,
    pub supports_managed_profiles: bool,
    pub supports_external_profiles: bool,
    pub supports_model_catalog: bool,
    pub supports_dedicated_model_processes: bool,
}

impl RuntimeProviderCapabilities {
    pub fn ollama() -> Self {
        Self {
            provider: RuntimeProviderId::Ollama,
            provider_modes: vec![RuntimeProviderMode::OllamaServe],
            device_modes: vec![
                RuntimeDeviceMode::Auto,
                RuntimeDeviceMode::Cpu,
                RuntimeDeviceMode::Gpu,
                RuntimeDeviceMode::Hybrid,
            ],
            supports_managed_profiles: true,
            supports_external_profiles: true,
            supports_model_catalog: false,
            supports_dedicated_model_processes: false,
        }
    }

    pub fn llama_cpp() -> Self {
        Self {
            provider: RuntimeProviderId::LlamaCpp,
            provider_modes: vec![
                RuntimeProviderMode::LlamaCppRouter,
                RuntimeProviderMode::LlamaCppDedicated,
            ],
            device_modes: vec![
                RuntimeDeviceMode::Auto,
                RuntimeDeviceMode::Cpu,
                RuntimeDeviceMode::Gpu,
                RuntimeDeviceMode::SpecificDevice,
            ],
            supports_managed_profiles: true,
            supports_external_profiles: true,
            supports_model_catalog: true,
            supports_dedicated_model_processes: true,
        }
    }
}

#[async_trait]
pub trait RuntimeProviderAdapter: Send + Sync {
    fn provider(&self) -> RuntimeProviderId;
    fn capabilities(&self) -> RuntimeProviderCapabilities;
    async fn validate_profile(&self, profile: &RuntimeProfileConfig) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_capabilities_separate_ollama_and_llama_cpp_modes() {
        let ollama = RuntimeProviderCapabilities::ollama();
        assert_eq!(ollama.provider, RuntimeProviderId::Ollama);
        assert_eq!(
            ollama.provider_modes,
            vec![RuntimeProviderMode::OllamaServe]
        );
        assert!(!ollama.supports_dedicated_model_processes);

        let llama_cpp = RuntimeProviderCapabilities::llama_cpp();
        assert_eq!(llama_cpp.provider, RuntimeProviderId::LlamaCpp);
        assert!(llama_cpp
            .provider_modes
            .contains(&RuntimeProviderMode::LlamaCppRouter));
        assert!(llama_cpp
            .provider_modes
            .contains(&RuntimeProviderMode::LlamaCppDedicated));
        assert!(llama_cpp.supports_dedicated_model_processes);
    }
}
