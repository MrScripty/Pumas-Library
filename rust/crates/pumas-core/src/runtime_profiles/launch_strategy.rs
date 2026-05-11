//! Managed runtime-profile launch strategy contracts.

use serde::{Deserialize, Serialize};

use crate::models::{
    RuntimeManagementMode, RuntimeProfileConfig, RuntimeProviderId, RuntimeProviderMode,
};
use crate::{PumasError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProfileBinaryLaunchKind {
    OllamaServe,
    LlamaCppRouter,
    LlamaCppDedicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProfilePythonSidecarKind {
    OnnxRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum RuntimeProfileLaunchStrategy {
    BinaryProcess(RuntimeProfileBinaryLaunchKind),
    PythonSidecar(RuntimeProfilePythonSidecarKind),
    ExternalOnly,
}

impl RuntimeProfileLaunchStrategy {
    pub fn for_profile(profile: &RuntimeProfileConfig) -> Result<Self> {
        if profile.management_mode == RuntimeManagementMode::External {
            return Ok(Self::ExternalOnly);
        }

        match profile.provider {
            RuntimeProviderId::Ollama => match profile.provider_mode {
                RuntimeProviderMode::OllamaServe => Ok(Self::BinaryProcess(
                    RuntimeProfileBinaryLaunchKind::OllamaServe,
                )),
                RuntimeProviderMode::LlamaCppRouter | RuntimeProviderMode::LlamaCppDedicated => {
                    Err(PumasError::InvalidParams {
                        message: "Ollama runtime profile cannot use llama.cpp modes".to_string(),
                    })
                }
            },
            RuntimeProviderId::LlamaCpp => match profile.provider_mode {
                RuntimeProviderMode::LlamaCppRouter => Ok(Self::BinaryProcess(
                    RuntimeProfileBinaryLaunchKind::LlamaCppRouter,
                )),
                RuntimeProviderMode::LlamaCppDedicated => Ok(Self::BinaryProcess(
                    RuntimeProfileBinaryLaunchKind::LlamaCppDedicated,
                )),
                RuntimeProviderMode::OllamaServe => Err(PumasError::InvalidParams {
                    message: "llama.cpp runtime profile cannot use ollama_serve mode".to_string(),
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_strategy_maps_existing_managed_profiles_to_binary_processes() {
        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(&RuntimeProfileConfig::default_ollama())
                .unwrap(),
            RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::OllamaServe
            )
        );

        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.provider = RuntimeProviderId::LlamaCpp;
        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;
        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(&profile).unwrap(),
            RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppRouter
            )
        );

        profile.provider_mode = RuntimeProviderMode::LlamaCppDedicated;
        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(&profile).unwrap(),
            RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppDedicated
            )
        );
    }

    #[test]
    fn launch_strategy_represents_external_profiles_explicitly() {
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.management_mode = RuntimeManagementMode::External;

        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(&profile).unwrap(),
            RuntimeProfileLaunchStrategy::ExternalOnly
        );
    }
}
