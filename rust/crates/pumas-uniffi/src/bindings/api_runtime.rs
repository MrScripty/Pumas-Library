use super::{
    FfiApiConfig, FfiDiskSpaceResponse, FfiError, FfiPumasApi, FfiStatusResponse,
    FfiSystemResourcesResponse,
};
use std::sync::Arc;

#[uniffi::export(async_runtime = "tokio")]
impl FfiPumasApi {
    /// Create a new API instance with default options.
    #[uniffi::constructor]
    pub async fn new(launcher_root: String) -> Result<Arc<Self>, FfiError> {
        Self::new_with_default_root(launcher_root).await
    }

    /// Create a new API instance with a configuration record.
    #[uniffi::constructor]
    pub async fn with_config(config: FfiApiConfig) -> Result<Arc<Self>, FfiError> {
        Self::new_with_configured_root(config).await
    }

    /// Check if the network is currently online.
    pub fn is_online(&self) -> bool {
        self.primary().is_online()
    }

    /// Get disk space information for the launcher root.
    pub async fn get_disk_space(&self) -> Result<FfiDiskSpaceResponse, FfiError> {
        let resp = self
            .primary()
            .get_disk_space()
            .await
            .map_err(FfiError::from)?;
        Ok(FfiDiskSpaceResponse::from(resp))
    }

    /// Get overall system status including running processes and resources.
    pub async fn get_status(&self) -> Result<FfiStatusResponse, FfiError> {
        let resp = self.primary().get_status().await.map_err(FfiError::from)?;
        Ok(FfiStatusResponse::from(resp))
    }

    /// Get current system resource usage (CPU, GPU, RAM, disk).
    pub async fn get_system_resources(&self) -> Result<FfiSystemResourcesResponse, FfiError> {
        let resp = self
            .primary()
            .get_system_resources()
            .await
            .map_err(FfiError::from)?;
        Ok(FfiSystemResourcesResponse::from(resp))
    }

    /// Check if the Torch inference server is running.
    pub async fn is_torch_running(&self) -> bool {
        self.primary().is_torch_running().await
    }

    /// Stop the Torch inference server.
    pub async fn torch_stop(&self) -> Result<bool, FfiError> {
        self.primary().stop_torch().await.map_err(FfiError::from)
    }
}
