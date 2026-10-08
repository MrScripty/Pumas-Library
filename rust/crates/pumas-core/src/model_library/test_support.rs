//! Explicit integration-fixture support, absent from default production builds.
//!
//! This adapter exercises current configured-root admission. It does not import
//! legacy stores or bypass the durable publisher. Explicit loopback fixtures
//! exercise real network workers without ambient source credentials.
//! Owned blocking fixtures exercise shutdown through the real HF task owner.

use super::download_recovery::DownloadDestinationRoot;
use super::download_store::{
    DownloadAdmissionDomain, DownloadAdmissionRequest, DownloadPersistence,
};
use crate::models::DownloadStatus;
use crate::{PumasError, Result};
use std::path::Path;

pub use super::download_store::PersistedDownload;

/// An explicit caller-owned literal-loopback origin for HF integration tests.
/// No production environment setting selects this non-default adapter.
#[derive(Clone)]
pub struct HfLoopbackFixture {
    origin: String,
}

impl HfLoopbackFixture {
    pub fn parse(origin: &str) -> Result<Self> {
        let refused = || {
            PumasError::Validation {
            field: "fixture.hf_origin".into(),
            message: "HF fixtures require an HTTP literal-loopback origin with an explicit port and no access material or path".into(),
        }
        };
        let url = reqwest::Url::parse(origin).map_err(|_| refused())?;
        if url.scheme() != "http"
            || !url.host().is_some_and(|host| match host {
                url::Host::Ipv4(ip) => ip.is_loopback(),
                url::Host::Ipv6(ip) => ip.is_loopback(),
                url::Host::Domain(_) => false,
            })
            || url.port().is_none_or(|port| port == 0)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(refused());
        }
        Ok(Self {
            origin: url.origin().ascii_serialization(),
        })
    }

    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    pub(crate) fn transport(&self) -> Result<reqwest::Client> {
        self.transport_builder()
            .build()
            .map_err(|error| PumasError::Other(format!("HF fixture transport: {error}")))
    }

    pub(crate) fn api_transport(&self) -> Result<reqwest::Client> {
        self.transport_builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|error| PumasError::Other(format!("HF fixture API transport: {error}")))
    }

    fn transport_builder(&self) -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(30))
    }
}

/// Run isolated fixture work through the real download invocation/effect owner.
///
/// The returned future owns its client reference so the API can move into its
/// real server supervisor. The fixture owns every file and synchronization gate
/// used by `work`; this helper does not grant live-library mutation authority.
pub fn run_download_blocking_fixture<T, F>(
    api: &crate::PumasApi,
    work: F,
) -> impl std::future::Future<Output = Result<T>> + Send + 'static
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let client = api.primary().hf_client.clone();
    async move {
        let client = client.ok_or_else(|| PumasError::Config {
            message: "Download fixture requires an HF client".into(),
        })?;
        client
            .run_download_invocation(move |context| async move {
                context
                    .run_fallible_blocking_named("download integration fixture", work)
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Download fixture observation failed: {error}"))
                    })?
            })
            .await
    }
}

/// Admit a paused fixture through the real current-format store owner.
///
/// The caller owns an isolated launcher root and supplies all material snapshot
/// fields. This synchronous setup performs filesystem I/O; invoke it before
/// starting the process under test, not from a production async request.
pub fn admit_paused_download(launcher_root: &Path, snapshot: &PersistedDownload) -> Result<()> {
    if snapshot.status != DownloadStatus::Paused {
        return Err(PumasError::Validation {
            field: "fixture.status".into(),
            message: "Admission fixtures must be paused".into(),
        });
    }
    let root = DownloadDestinationRoot::open(&launcher_root.join("shared-resources/models"))?;
    let destination = root.resolve(&snapshot.dest_dir)?;
    let request = DownloadAdmissionRequest {
        snapshot: snapshot.clone(),
        domain: DownloadAdmissionDomain::Ambient,
        destination: destination.persisted_identity()?,
        requested_payload_files: snapshot.download_request.filenames.clone().ok_or_else(|| {
            PumasError::Validation {
                field: "fixture.download_request.filenames".into(),
                message: "Admission fixtures require explicit payload files".into(),
            }
        })?,
        execution_files: snapshot.filenames.clone(),
    };
    DownloadPersistence::new(&launcher_root.join("launcher-data"))
        .admit_download(&uuid::Uuid::new_v4().to_string(), &request)?
        .into_result()?;
    Ok(())
}

/// Exercise the existing internal link-cleanup dispatch branch. This does not
/// add a wire operation: the current typed local IPC allowlist excludes it.
pub async fn clean_broken_links_dispatch(
    api: &crate::PumasApi,
) -> Result<crate::models::CleanBrokenLinksResponse> {
    let value = crate::ipc::server::IpcDispatch::dispatch(
        api.primary().as_ref(),
        "clean_broken_links",
        serde_json::json!({}),
    )
    .await?;
    Ok(serde_json::from_value(value)?)
}
