//! Discovery of the runtime providers this server registers.
//!
//! The desktop renders version management, runtime profile choices and the
//! provider-to-runtime mapping from this projection instead of copying the
//! provider registry. Adding a provider or a field is additive.

use pumas_library::models::{RuntimeDeviceMode, RuntimeProviderId, RuntimeProviderMode};
use pumas_library::{ProviderBehavior, ProviderRegistry};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct RuntimeProvidersOutcome {
    success: bool,
    /// Registered providers in the registry's declared order.
    providers: Vec<RuntimeProviderDescriptor>,
}

/// One registered runtime provider, as the desktop may present it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct RuntimeProviderDescriptor {
    provider: RuntimeProviderId,
    /// Version-manager app id whose installed versions a managed profile of
    /// this provider launches.
    app_id: String,
    /// Whether this server holds a version manager for `app_id`. False when
    /// the build has none or it failed to initialize.
    version_managed: bool,
    provider_modes: Vec<RuntimeProviderMode>,
    /// Device modes a profile of this provider may request.
    device_modes: Vec<RuntimeDeviceMode>,
    supports_managed_profiles: bool,
    supports_external_profiles: bool,
}

impl RuntimeProviderDescriptor {
    pub(crate) fn from_behavior(behavior: &ProviderBehavior, version_managed: bool) -> Self {
        Self {
            provider: behavior.provider,
            app_id: behavior.managed_runtime_app_id.clone(),
            version_managed,
            provider_modes: behavior.provider_modes.clone(),
            device_modes: behavior.device_modes.clone(),
            supports_managed_profiles: behavior.supports_managed_profiles,
            supports_external_profiles: behavior.supports_external_profiles,
        }
    }
}

impl RuntimeProvidersOutcome {
    /// Projects `registry`, asking `is_version_managed` about each provider's
    /// runtime app id.
    pub(crate) fn from_registry(
        registry: &ProviderRegistry,
        is_version_managed: impl Fn(&str) -> bool,
    ) -> Self {
        Self {
            success: true,
            providers: registry
                .providers()
                .into_iter()
                .map(|behavior| {
                    let managed = is_version_managed(&behavior.managed_runtime_app_id);
                    RuntimeProviderDescriptor::from_behavior(behavior, managed)
                })
                .collect(),
        }
    }
}

/// Producer-made samples for the desktop conformance tests.
#[cfg(feature = "export-contract")]
pub(crate) fn runtime_provider_fixtures() -> serde_json::Result<serde_json::Value> {
    let registry = ProviderRegistry::builtin();
    Ok(serde_json::json!({
        "runtime_providers": RuntimeProvidersOutcome::from_registry(&registry, |app_id| {
            app_id != "torch"
        }),
        "runtime_providers_empty": RuntimeProvidersOutcome::from_registry(
            &ProviderRegistry::from_behaviors([]),
            |_| true,
        ),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_every_builtin_provider_with_its_runtime_app_and_modes() {
        let registry = ProviderRegistry::builtin();
        let outcome =
            RuntimeProvidersOutcome::from_registry(&registry, |app_id| app_id == "onnx-runtime");
        let value = serde_json::to_value(&outcome).unwrap();

        assert_eq!(value["success"], true);
        let providers = value["providers"].as_array().unwrap();
        let ids: Vec<_> = providers
            .iter()
            .map(|provider| provider["provider"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["ollama", "llama_cpp", "onnx_runtime", "torch"]);

        let onnx = &providers[2];
        assert_eq!(
            onnx,
            &serde_json::json!({
                "provider": "onnx_runtime",
                "app_id": "onnx-runtime",
                "version_managed": true,
                "provider_modes": ["onnx_serve"],
                "device_modes": ["auto", "cpu", "gpu"],
                "supports_managed_profiles": true,
                "supports_external_profiles": false,
            })
        );
        assert_eq!(providers[1]["app_id"], "llama-cpp");
        assert_eq!(providers[1]["version_managed"], false);
        assert_eq!(
            providers[3]["device_modes"],
            serde_json::json!(["gpu", "hybrid"])
        );
    }
}
