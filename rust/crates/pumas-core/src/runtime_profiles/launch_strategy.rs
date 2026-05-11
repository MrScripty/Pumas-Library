//! Managed runtime-profile launch strategy contracts.

use serde::{Deserialize, Serialize};

use crate::models::{RuntimeManagementMode, RuntimeProfileConfig};
use crate::providers::{
    ProviderBehavior, ProviderBinaryLaunchTarget, ProviderInProcessRuntimeTarget,
    ProviderManagedLaunchTarget,
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
pub enum RuntimeProfileInProcessRuntimeKind {
    OnnxRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum RuntimeProfileLaunchStrategy {
    BinaryProcess(RuntimeProfileBinaryLaunchKind),
    InProcessRuntime(RuntimeProfileInProcessRuntimeKind),
    ExternalOnly,
}

impl RuntimeProfileLaunchStrategy {
    pub fn for_profile(
        profile: &RuntimeProfileConfig,
        behavior: &ProviderBehavior,
    ) -> Result<Self> {
        if behavior.provider != profile.provider {
            return Err(PumasError::InvalidParams {
                message: "runtime profile launch strategy provider mismatch".to_string(),
            });
        }

        if profile.management_mode == RuntimeManagementMode::External {
            return Ok(Self::ExternalOnly);
        }

        behavior
            .managed_launch_target(profile.provider_mode)
            .map(Self::from)
            .ok_or_else(|| PumasError::InvalidParams {
                message: "runtime profile provider mode does not declare a managed launch strategy"
                    .to_string(),
            })
    }
}

impl From<ProviderManagedLaunchTarget> for RuntimeProfileLaunchStrategy {
    fn from(target: ProviderManagedLaunchTarget) -> Self {
        match target {
            ProviderManagedLaunchTarget::BinaryProcess(target) => {
                Self::BinaryProcess(target.into())
            }
            ProviderManagedLaunchTarget::InProcessRuntime(target) => {
                Self::InProcessRuntime(target.into())
            }
        }
    }
}

impl From<ProviderBinaryLaunchTarget> for RuntimeProfileBinaryLaunchKind {
    fn from(target: ProviderBinaryLaunchTarget) -> Self {
        match target {
            ProviderBinaryLaunchTarget::OllamaServe => Self::OllamaServe,
            ProviderBinaryLaunchTarget::LlamaCppRouter => Self::LlamaCppRouter,
            ProviderBinaryLaunchTarget::LlamaCppDedicated => Self::LlamaCppDedicated,
        }
    }
}

impl From<ProviderInProcessRuntimeTarget> for RuntimeProfileInProcessRuntimeKind {
    fn from(target: ProviderInProcessRuntimeTarget) -> Self {
        match target {
            ProviderInProcessRuntimeTarget::OnnxRuntime => Self::OnnxRuntime,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RuntimeProviderId, RuntimeProviderMode};

    #[test]
    fn launch_strategy_maps_existing_managed_profiles_to_binary_processes() {
        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(
                &RuntimeProfileConfig::default_ollama(),
                &ProviderBehavior::ollama()
            )
            .unwrap(),
            RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::OllamaServe
            )
        );

        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.provider = RuntimeProviderId::LlamaCpp;
        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;
        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(&profile, &ProviderBehavior::llama_cpp())
                .unwrap(),
            RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppRouter
            )
        );

        profile.provider_mode = RuntimeProviderMode::LlamaCppDedicated;
        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(&profile, &ProviderBehavior::llama_cpp())
                .unwrap(),
            RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppDedicated
            )
        );
    }

    #[test]
    fn launch_strategy_maps_onnx_managed_profile_to_in_process_runtime() {
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.provider = RuntimeProviderId::OnnxRuntime;
        profile.provider_mode = RuntimeProviderMode::OnnxServe;

        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(&profile, &ProviderBehavior::onnx_runtime())
                .unwrap(),
            RuntimeProfileLaunchStrategy::InProcessRuntime(
                RuntimeProfileInProcessRuntimeKind::OnnxRuntime
            )
        );
    }

    #[test]
    fn launch_strategy_represents_external_profiles_explicitly() {
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.management_mode = RuntimeManagementMode::External;

        assert_eq!(
            RuntimeProfileLaunchStrategy::for_profile(&profile, &ProviderBehavior::ollama())
                .unwrap(),
            RuntimeProfileLaunchStrategy::ExternalOnly
        );
    }

    #[test]
    fn launch_strategy_rejects_missing_provider_mapping() {
        let mut behavior = ProviderBehavior::ollama();
        behavior.managed_launch_strategies.clear();

        assert!(matches!(
            RuntimeProfileLaunchStrategy::for_profile(
                &RuntimeProfileConfig::default_ollama(),
                &behavior
            ),
            Err(PumasError::InvalidParams { .. })
        ));
    }
}
