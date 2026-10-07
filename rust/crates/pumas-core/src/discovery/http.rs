//! Generation-scoped HTTP service observations, not owner reclamation or a daemon.
use super::{CompatibilityRequirements, InstanceDescription, LocalDiscovery};
use crate::registry::{InstanceEntry, LibraryRegistry};
use crate::{PumasApi, PumasBuildInfo, PumasError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Weak};

pub const HTTP_ADVERTISEMENT_SCHEMA_VERSION: u32 = 1;
pub const LOCAL_HTTP_PROTOCOL: &str = "pumas.local-http";
pub const LOCAL_HTTP_VERSION: u32 = 1;
pub const HTTP_DISCOVERY_PATH: &str = "/.well-known/pumas";

/// Numeric loopback HTTP only. No redirects, credentials, proxy hostnames,
/// query strings or remote URL interpretation belongs to local rendezvous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LoopbackHttpEndpoint(String);
impl LoopbackHttpEndpoint {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let url = url::Url::parse(&value).map_err(|_| invalid("invalid local HTTP endpoint"))?;
        let loopback = match url.host() {
            Some(url::Host::Ipv4(address)) => address.is_loopback(),
            Some(url::Host::Ipv6(address)) => address.is_loopback(),
            _ => false,
        };
        if url.scheme() != "http"
            || !loopback
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || url.port_or_known_default().is_none_or(|port| port == 0)
        {
            return Err(invalid(
                "local HTTP endpoint must be a numeric loopback base URL",
            ));
        }
        Ok(Self(url.to_string().trim_end_matches('/').to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for LoopbackHttpEndpoint {
    type Error = PumasError;
    fn try_from(value: String) -> Result<Self> {
        Self::parse(value)
    }
}
impl From<LoopbackHttpEndpoint> for String {
    fn from(value: LoopbackHttpEndpoint) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpServiceDescription {
    pub advertisement_schema_version: u32,
    pub service_generation: String,
    pub instance: InstanceDescription,
    pub endpoint: LoopbackHttpEndpoint,
    /// HTTP producer identity; the instance separately describes its core build.
    pub build_info: PumasBuildInfo,
}

/// Prepared registration is invisible until the listener/router owner publishes
/// it. This guard revokes only this HTTP generation and never stops the core.
pub struct HttpServiceRegistration {
    registry: LibraryRegistry,
    owner: InstanceEntry,
    description: HttpServiceDescription,
    published: bool,
    primary: Weak<crate::api::PrimaryState>,
    settlement: Option<tokio::sync::oneshot::Sender<Result<()>>>,
}
impl HttpServiceRegistration {
    pub fn description(&self) -> &HttpServiceDescription {
        &self.description
    }
    pub fn publish(&mut self) -> Result<()> {
        if self.published {
            return Err(invalid("HTTP generation already published"));
        }
        let primary = self
            .primary
            .upgrade()
            .ok_or_else(|| invalid("HTTP owner unavailable"))?;
        if primary.instance_shutdown.get().is_some() {
            return Err(invalid("HTTP owner is closing"));
        }
        self.registry
            .publish_http_service(&self.owner, &self.description)?;
        self.published = true;
        if primary.instance_shutdown.get().is_some() {
            self.revoke()?;
            return Err(invalid("HTTP owner closed during publication"));
        }
        Ok(())
    }
    /// Report only after the listener, accepted requests and all HTTP-owned
    /// effects have settled. A failed/abandoned receipt retains core authority.
    pub fn complete_shutdown(&mut self, outcome: Result<()>) -> Result<()> {
        if self.published {
            return Err(invalid("revoke HTTP admission before reporting cessation"));
        }
        let sender = self
            .settlement
            .take()
            .ok_or_else(|| invalid("HTTP cessation already reported"))?;
        sender
            .send(outcome)
            .map_err(|_| invalid("HTTP cessation observer unavailable"))
    }

    pub fn revoke(&mut self) -> Result<bool> {
        if !self.published {
            return Ok(false);
        }
        let removed = self
            .registry
            .revoke_http_service(&self.owner, &self.description.service_generation)?;
        self.published = false;
        Ok(removed)
    }
}
impl Drop for HttpServiceRegistration {
    fn drop(&mut self) {
        let _ = self.revoke();
        // Dropping this promise cannot supply evidence of external owner cessation.
        // A closed channel is archived by the independently retained owner task.
        let _ = self.settlement.take();
    }
}

impl PumasApi {
    pub fn instance_description(&self) -> Result<InstanceDescription> {
        let primary = self.primary();
        if primary.instance_shutdown.get().is_some() {
            return Err(invalid("local owner is closing"));
        }
        let registry = primary
            .registry
            .as_ref()
            .ok_or_else(|| invalid("owner registry unavailable"))?;
        let instance = primary
            .ready_instance
            .get()
            .ok_or_else(|| invalid("owner is not ready"))?;
        if !registry.matches_ready_instance(instance)? {
            return Err(invalid("owner generation changed"));
        }
        let library = registry
            .get_by_path(&instance.library_path)?
            .ok_or_else(|| invalid("library registration unavailable"))?;
        Ok(InstanceDescription::local(&library, instance))
    }

    /// Does not publish. The HTTP listener owner must publish only after readiness.
    pub fn prepare_http_service(
        &self,
        endpoint: LoopbackHttpEndpoint,
        build_info: PumasBuildInfo,
    ) -> Result<HttpServiceRegistration> {
        if !build_info.supports_schema(
            "pumas.http-advertisement",
            HTTP_ADVERTISEMENT_SCHEMA_VERSION,
        ) || build_info.build_info_schema_version != crate::build_info::BUILD_INFO_SCHEMA_VERSION
            || build_info
                .protocol_version(LOCAL_HTTP_PROTOCOL, &[LOCAL_HTTP_VERSION])
                .is_none()
        {
            return Err(invalid("HTTP build protocol/schema is incompatible"));
        }
        let description = HttpServiceDescription {
            advertisement_schema_version: HTTP_ADVERTISEMENT_SCHEMA_VERSION,
            service_generation: uuid::Uuid::new_v4().to_string(),
            instance: self.instance_description()?,
            endpoint,
            build_info,
        };
        let primary = self.primary();
        let (settlement, observer) = tokio::sync::oneshot::channel();
        let _result = primary.external_service_tasks.start_owned(
            "http-service-custody",
            move |_| async move {
                observer
                    .await
                    .map_err(|_| invalid("HTTP service owner abandoned its cessation receipt"))?
            },
        )?;
        Ok(HttpServiceRegistration {
            primary: Arc::downgrade(primary),
            settlement: Some(settlement),
            registry: primary.registry.as_ref().unwrap().clone(),
            owner: primary.ready_instance.get().unwrap().clone(),
            description,
            published: false,
        })
    }

    /// Read-only HTTP handler observation. Closing/changed owners are unavailable.
    pub fn advertised_http_service(&self) -> Result<Option<HttpServiceDescription>> {
        let instance = self.instance_description()?;
        let registry = self.primary().registry.as_ref().unwrap();
        Ok(registry.list_http_services()?.into_iter().find(|service| {
            service.instance.library_root == instance.library_root
                && service.instance.generation == instance.generation
        }))
    }
}

/// Borrowed peer has no shutdown or owner-transition operation.
pub struct BorrowedHttpService {
    description: HttpServiceDescription,
    _core: crate::PumasLocalClient,
}
impl BorrowedHttpService {
    pub fn description(&self) -> &HttpServiceDescription {
        &self.description
    }
}

impl LocalDiscovery {
    /// Verify both authenticated core identity and the advertised HTTP generation.
    /// An absent/unreachable/incompatible service is an error, never permission to start.
    pub async fn borrow_http_service(
        &self,
        root: &Path,
        requirements: &CompatibilityRequirements,
    ) -> Result<BorrowedHttpService> {
        let library = self
            .registry
            .get_by_path(root)?
            .ok_or_else(|| invalid("no selected library registration"))?;
        let instance = self
            .registry
            .get_instance(&library.path)?
            .ok_or_else(|| invalid("no tracked library owner"))?;
        let (core, core_description, _) = super::attach(instance, &library, requirements).await?;
        let advertised = self
            .registry
            .list_http_services()?
            .into_iter()
            .find(|service| {
                service.instance.library_root == library.path
                    && service.instance.generation == core_description.generation
            })
            .ok_or_else(|| invalid("selected owner has no HTTP advertisement"))?;
        let observed = fetch_description(&advertised.endpoint).await?;
        if !observed.build_info.supports_schema(
            "pumas.http-advertisement",
            HTTP_ADVERTISEMENT_SCHEMA_VERSION,
        ) || observed.advertisement_schema_version != HTTP_ADVERTISEMENT_SCHEMA_VERSION
            || observed != advertised
            || observed.instance != core_description
            || observed.build_info.build_info_schema_version
                != crate::build_info::BUILD_INFO_SCHEMA_VERSION
            || observed
                .build_info
                .protocol_version(LOCAL_HTTP_PROTOCOL, &[LOCAL_HTTP_VERSION])
                .is_none()
        {
            return Err(invalid(
                "HTTP handshake changed context, generation or build contract",
            ));
        }
        requirements.negotiate(&observed.instance)?;
        // Reauthenticate after HTTP observation so an owner change during the
        // request remains unresolved. Observations still have no lifetime lease.
        if core.describe_instance().await? != core_description {
            return Err(invalid("core identity changed during HTTP observation"));
        }
        Ok(BorrowedHttpService {
            description: observed,
            _core: core,
        })
    }
}

async fn fetch_description(endpoint: &LoopbackHttpEndpoint) -> Result<HttpServiceDescription> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(crate::config::RegistryConfig::PRIMARY_READY_TIMEOUT)
        .build()
        .map_err(|_| invalid("could not construct local HTTP observer"))?;
    let mut response = client
        .get(format!("{}{HTTP_DISCOVERY_PATH}", endpoint.as_str()))
        .send()
        .await
        .map_err(|_| invalid("local HTTP description is unreachable"))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(invalid("local HTTP description is unavailable"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| invalid("local HTTP description is incomplete"))?
    {
        if body.len().saturating_add(chunk.len()) > 64 * 1024 {
            return Err(invalid("local HTTP description exceeds observation limit"));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| invalid("local HTTP description is invalid"))
}

fn invalid(message: &str) -> PumasError {
    PumasError::InvalidParams {
        message: message.into(),
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
