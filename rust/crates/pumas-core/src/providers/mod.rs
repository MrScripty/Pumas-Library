//! Runtime provider behavior contracts and built-in provider registry.

use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};

use crate::models::{
    RuntimeDeviceMode, RuntimeManagementMode, RuntimeProviderId, RuntimeProviderMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutableArtifactFormat {
    Gguf,
}

impl ExecutableArtifactFormat {
    pub fn from_extension(extension: &str) -> Option<Self> {
        if extension.eq_ignore_ascii_case("gguf") {
            Some(Self::Gguf)
        } else {
            None
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|extension| extension.to_str())
            .and_then(Self::from_extension)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingTask {
    Chat,
    Completion,
    Embedding,
    Reranking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiGatewayEndpoint {
    Models,
    ChatCompletions,
    Completions,
    Embeddings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLaunchKind {
    BinaryProcess,
    PythonSidecar,
    ExternalOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBinaryLaunchTarget {
    OllamaServe,
    LlamaCppRouter,
    LlamaCppDedicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPythonSidecarTarget {
    OnnxRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ProviderManagedLaunchTarget {
    BinaryProcess(ProviderBinaryLaunchTarget),
    PythonSidecar(ProviderPythonSidecarTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProviderManagedLaunchStrategy {
    pub provider_mode: RuntimeProviderMode,
    pub target: ProviderManagedLaunchTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderModelIdPolicy {
    GatewayAlias,
    LibraryModelId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderGatewayAliasPolicy {
    OllamaModelName,
    LibraryModelId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderServingAdapterKind {
    OllamaProviderApi,
    LlamaCppRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderUnloadBehavior {
    ProviderApi,
    RouterPreset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProviderBehavior {
    pub provider: RuntimeProviderId,
    pub provider_modes: Vec<RuntimeProviderMode>,
    pub device_modes: Vec<RuntimeDeviceMode>,
    pub local_artifact_formats: Vec<ExecutableArtifactFormat>,
    pub serving_tasks: Vec<ServingTask>,
    pub openai_endpoints: Vec<OpenAiGatewayEndpoint>,
    pub launch_kinds: Vec<ProviderLaunchKind>,
    pub managed_launch_strategies: Vec<ProviderManagedLaunchStrategy>,
    pub provider_model_id_policy: ProviderModelIdPolicy,
    pub gateway_alias_policy: ProviderGatewayAliasPolicy,
    pub serving_adapter_kind: ProviderServingAdapterKind,
    pub unload_behavior: ProviderUnloadBehavior,
    pub supports_managed_profiles: bool,
    pub supports_external_profiles: bool,
    pub supports_model_catalog: bool,
    pub supports_dedicated_model_processes: bool,
}

impl ProviderBehavior {
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
            local_artifact_formats: vec![ExecutableArtifactFormat::Gguf],
            serving_tasks: vec![
                ServingTask::Chat,
                ServingTask::Completion,
                ServingTask::Embedding,
            ],
            openai_endpoints: vec![
                OpenAiGatewayEndpoint::Models,
                OpenAiGatewayEndpoint::ChatCompletions,
                OpenAiGatewayEndpoint::Completions,
                OpenAiGatewayEndpoint::Embeddings,
            ],
            launch_kinds: vec![
                ProviderLaunchKind::BinaryProcess,
                ProviderLaunchKind::ExternalOnly,
            ],
            managed_launch_strategies: vec![ProviderManagedLaunchStrategy {
                provider_mode: RuntimeProviderMode::OllamaServe,
                target: ProviderManagedLaunchTarget::BinaryProcess(
                    ProviderBinaryLaunchTarget::OllamaServe,
                ),
            }],
            provider_model_id_policy: ProviderModelIdPolicy::GatewayAlias,
            gateway_alias_policy: ProviderGatewayAliasPolicy::OllamaModelName,
            serving_adapter_kind: ProviderServingAdapterKind::OllamaProviderApi,
            unload_behavior: ProviderUnloadBehavior::ProviderApi,
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
            local_artifact_formats: vec![ExecutableArtifactFormat::Gguf],
            serving_tasks: vec![
                ServingTask::Chat,
                ServingTask::Completion,
                ServingTask::Embedding,
                ServingTask::Reranking,
            ],
            openai_endpoints: vec![
                OpenAiGatewayEndpoint::Models,
                OpenAiGatewayEndpoint::ChatCompletions,
                OpenAiGatewayEndpoint::Completions,
                OpenAiGatewayEndpoint::Embeddings,
            ],
            launch_kinds: vec![
                ProviderLaunchKind::BinaryProcess,
                ProviderLaunchKind::ExternalOnly,
            ],
            managed_launch_strategies: vec![
                ProviderManagedLaunchStrategy {
                    provider_mode: RuntimeProviderMode::LlamaCppRouter,
                    target: ProviderManagedLaunchTarget::BinaryProcess(
                        ProviderBinaryLaunchTarget::LlamaCppRouter,
                    ),
                },
                ProviderManagedLaunchStrategy {
                    provider_mode: RuntimeProviderMode::LlamaCppDedicated,
                    target: ProviderManagedLaunchTarget::BinaryProcess(
                        ProviderBinaryLaunchTarget::LlamaCppDedicated,
                    ),
                },
            ],
            provider_model_id_policy: ProviderModelIdPolicy::LibraryModelId,
            gateway_alias_policy: ProviderGatewayAliasPolicy::LibraryModelId,
            serving_adapter_kind: ProviderServingAdapterKind::LlamaCppRuntime,
            unload_behavior: ProviderUnloadBehavior::RouterPreset,
            supports_managed_profiles: true,
            supports_external_profiles: true,
            supports_model_catalog: true,
            supports_dedicated_model_processes: true,
        }
    }

    pub fn supports_mode(&self, mode: RuntimeProviderMode) -> bool {
        self.provider_modes.contains(&mode)
    }

    pub fn supports_openai_endpoint(&self, endpoint: OpenAiGatewayEndpoint) -> bool {
        self.openai_endpoints.contains(&endpoint)
    }

    pub fn supports_artifact_format(&self, format: ExecutableArtifactFormat) -> bool {
        self.local_artifact_formats.contains(&format)
    }

    pub fn supports_serving_task(&self, task: ServingTask) -> bool {
        self.serving_tasks.contains(&task)
    }

    pub fn supports_launch_kind(&self, launch_kind: ProviderLaunchKind) -> bool {
        self.launch_kinds.contains(&launch_kind)
    }

    pub fn supports_management_mode(&self, management_mode: RuntimeManagementMode) -> bool {
        match management_mode {
            RuntimeManagementMode::Managed => {
                self.supports_launch_kind(ProviderLaunchKind::BinaryProcess)
                    || self.supports_launch_kind(ProviderLaunchKind::PythonSidecar)
            }
            RuntimeManagementMode::External => {
                self.supports_launch_kind(ProviderLaunchKind::ExternalOnly)
            }
        }
    }

    pub fn managed_launch_target(
        &self,
        provider_mode: RuntimeProviderMode,
    ) -> Option<ProviderManagedLaunchTarget> {
        self.managed_launch_strategies
            .iter()
            .find(|strategy| strategy.provider_mode == provider_mode)
            .map(|strategy| strategy.target)
    }

    pub fn provider_request_model_id(
        &self,
        library_model_id: &str,
        gateway_alias: Option<&str>,
    ) -> String {
        match self.provider_model_id_policy {
            ProviderModelIdPolicy::GatewayAlias => gateway_alias
                .map(str::to_string)
                .unwrap_or_else(|| library_model_id.to_string()),
            ProviderModelIdPolicy::LibraryModelId => library_model_id.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    providers: HashMap<RuntimeProviderId, ProviderBehavior>,
}

impl ProviderRegistry {
    pub fn builtin() -> Self {
        Self::from_behaviors([ProviderBehavior::ollama(), ProviderBehavior::llama_cpp()])
    }

    pub fn from_behaviors(behaviors: impl IntoIterator<Item = ProviderBehavior>) -> Self {
        let providers = behaviors
            .into_iter()
            .map(|behavior| (behavior.provider, behavior))
            .collect();
        Self { providers }
    }

    pub fn get(&self, provider: RuntimeProviderId) -> Option<&ProviderBehavior> {
        self.providers.get(&provider)
    }

    pub fn contains(&self, provider: RuntimeProviderId) -> bool {
        self.providers.contains_key(&provider)
    }

    pub fn providers(&self) -> Vec<&ProviderBehavior> {
        let mut providers = Vec::new();
        for provider in [RuntimeProviderId::Ollama, RuntimeProviderId::LlamaCpp] {
            if let Some(behavior) = self.providers.get(&provider) {
                providers.push(behavior);
            }
        }
        providers
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_contains_existing_runtime_providers() {
        let registry = ProviderRegistry::builtin();

        assert!(registry.contains(RuntimeProviderId::Ollama));
        assert!(registry.contains(RuntimeProviderId::LlamaCpp));
        assert_eq!(registry.providers().len(), 2);
    }

    #[test]
    fn ollama_behavior_matches_existing_profile_surface() {
        let registry = ProviderRegistry::builtin();
        let behavior = registry.get(RuntimeProviderId::Ollama).unwrap();

        assert!(behavior.supports_mode(RuntimeProviderMode::OllamaServe));
        assert!(behavior.supports_openai_endpoint(OpenAiGatewayEndpoint::Models));
        assert!(behavior.supports_openai_endpoint(OpenAiGatewayEndpoint::ChatCompletions));
        assert!(behavior.supports_openai_endpoint(OpenAiGatewayEndpoint::Completions));
        assert!(behavior.supports_openai_endpoint(OpenAiGatewayEndpoint::Embeddings));
        assert_eq!(
            behavior.provider_model_id_policy,
            ProviderModelIdPolicy::GatewayAlias
        );
        assert_eq!(
            behavior.gateway_alias_policy,
            ProviderGatewayAliasPolicy::OllamaModelName
        );
        assert_eq!(
            behavior.serving_adapter_kind,
            ProviderServingAdapterKind::OllamaProviderApi
        );
        assert!(behavior.supports_artifact_format(ExecutableArtifactFormat::Gguf));
        assert!(behavior.supports_managed_profiles);
        assert!(behavior.supports_external_profiles);
    }

    #[test]
    fn llama_cpp_behavior_matches_existing_profile_surface() {
        let registry = ProviderRegistry::builtin();
        let behavior = registry.get(RuntimeProviderId::LlamaCpp).unwrap();

        assert!(behavior.supports_mode(RuntimeProviderMode::LlamaCppRouter));
        assert!(behavior.supports_mode(RuntimeProviderMode::LlamaCppDedicated));
        assert!(behavior.supports_artifact_format(ExecutableArtifactFormat::Gguf));
        assert!(behavior.supports_serving_task(ServingTask::Embedding));
        assert!(behavior.supports_serving_task(ServingTask::Reranking));
        assert_eq!(
            behavior.provider_model_id_policy,
            ProviderModelIdPolicy::LibraryModelId
        );
        assert_eq!(
            behavior.gateway_alias_policy,
            ProviderGatewayAliasPolicy::LibraryModelId
        );
        assert_eq!(
            behavior.serving_adapter_kind,
            ProviderServingAdapterKind::LlamaCppRuntime
        );
        assert_eq!(
            behavior.unload_behavior,
            ProviderUnloadBehavior::RouterPreset
        );
        assert!(behavior.supports_model_catalog);
        assert!(behavior.supports_dedicated_model_processes);
    }

    #[test]
    fn provider_behavior_serializes_contract_enums_as_snake_case() {
        let behavior = ProviderBehavior::llama_cpp();
        let serialized = serde_json::to_value(behavior).unwrap();

        assert_eq!(serialized["provider"], "llama_cpp");
        assert_eq!(serialized["provider_modes"][0], "llama_cpp_router");
        assert_eq!(serialized["local_artifact_formats"][0], "gguf");
        assert_eq!(serialized["openai_endpoints"][3], "embeddings");
        assert_eq!(serialized["launch_kinds"][0], "binary_process");
        assert_eq!(
            serialized["managed_launch_strategies"][0]["provider_mode"],
            "llama_cpp_router"
        );
        assert_eq!(
            serialized["managed_launch_strategies"][0]["target"]["kind"],
            "binary_process"
        );
        assert_eq!(
            serialized["managed_launch_strategies"][0]["target"]["value"],
            "llama_cpp_router"
        );
        assert_eq!(serialized["provider_model_id_policy"], "library_model_id");
        assert_eq!(serialized["gateway_alias_policy"], "library_model_id");
        assert_eq!(serialized["serving_adapter_kind"], "llama_cpp_runtime");
        assert_eq!(serialized["unload_behavior"], "router_preset");
    }

    #[test]
    fn provider_request_model_id_uses_declared_model_id_policy() {
        let ollama = ProviderBehavior::ollama();
        assert_eq!(
            ollama.provider_request_model_id("library/model.gguf", Some("gateway-alias")),
            "gateway-alias"
        );
        assert_eq!(
            ollama.provider_request_model_id("library/model.gguf", None),
            "library/model.gguf"
        );

        let llama_cpp = ProviderBehavior::llama_cpp();
        assert_eq!(
            llama_cpp.provider_request_model_id("library/model.gguf", Some("gateway-alias")),
            "library/model.gguf"
        );
    }

    #[test]
    fn executable_artifact_format_parses_supported_paths_once() {
        assert_eq!(
            ExecutableArtifactFormat::from_path(Path::new("/models/example.GGUF")),
            Some(ExecutableArtifactFormat::Gguf)
        );
        assert_eq!(
            ExecutableArtifactFormat::from_path(Path::new("/models/example.onnx")),
            None
        );
        assert_eq!(
            ExecutableArtifactFormat::from_path(Path::new("/models")),
            None
        );
    }

    #[test]
    fn provider_management_modes_derive_from_launch_kinds() {
        let mut behavior = ProviderBehavior::ollama();
        behavior.launch_kinds = vec![ProviderLaunchKind::ExternalOnly];

        assert!(!behavior.supports_management_mode(RuntimeManagementMode::Managed));
        assert!(behavior.supports_management_mode(RuntimeManagementMode::External));

        behavior.launch_kinds = vec![ProviderLaunchKind::PythonSidecar];

        assert!(behavior.supports_management_mode(RuntimeManagementMode::Managed));
        assert!(!behavior.supports_management_mode(RuntimeManagementMode::External));
    }

    #[test]
    fn provider_managed_launch_target_maps_existing_modes() {
        let ollama = ProviderBehavior::ollama();
        assert_eq!(
            ollama.managed_launch_target(RuntimeProviderMode::OllamaServe),
            Some(ProviderManagedLaunchTarget::BinaryProcess(
                ProviderBinaryLaunchTarget::OllamaServe
            ))
        );

        let llama_cpp = ProviderBehavior::llama_cpp();
        assert_eq!(
            llama_cpp.managed_launch_target(RuntimeProviderMode::LlamaCppRouter),
            Some(ProviderManagedLaunchTarget::BinaryProcess(
                ProviderBinaryLaunchTarget::LlamaCppRouter
            ))
        );
        assert_eq!(
            llama_cpp.managed_launch_target(RuntimeProviderMode::LlamaCppDedicated),
            Some(ProviderManagedLaunchTarget::BinaryProcess(
                ProviderBinaryLaunchTarget::LlamaCppDedicated
            ))
        );
        assert_eq!(
            llama_cpp.managed_launch_target(RuntimeProviderMode::OllamaServe),
            None
        );
    }
}
