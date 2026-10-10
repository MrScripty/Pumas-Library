//! CLI projection only; no runtime, listener, authentication or owner admission.
use pumas_library::discovery::{
    LocalDiscovery, RegisteredLibraryObservation, DISCOVERY_SCHEMA_VERSION,
};
use serde::Serialize;
use std::io::Write;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum RegistryState {
    Observed,
    Missing,
    Unavailable,
}

#[derive(Serialize)]
struct Enumeration {
    cli_schema_version: u32,
    discovery_schema_version: u32,
    observation: &'static str,
    registry_state: RegistryState,
    libraries: Vec<RegisteredLibraryObservation>,
}

pub(crate) fn write() -> anyhow::Result<()> {
    let result = pumas_library::platform::registry_db_path()
        .and_then(|path| LocalDiscovery::enumerate_registered_libraries_at(&path));
    let (registry_state, libraries, mut failed) = match result {
        Ok(libraries) => (RegistryState::Observed, libraries, false),
        Err(pumas_library::PumasError::FileNotFound(_)) => {
            (RegistryState::Missing, Vec::new(), false)
        }
        // Do not echo SQL values, metadata, credentials or ambient private paths.
        Err(_) => (RegistryState::Unavailable, Vec::new(), true),
    };
    let mut document = Enumeration {
        cli_schema_version: 1,
        discovery_schema_version: DISCOVERY_SCHEMA_VERSION,
        observation: "unverified_registry_copy",
        registry_state,
        libraries,
    };
    let mut bytes = serde_json::to_vec(&document)?;
    if bytes.len() > 4 * 1024 * 1024 {
        document.registry_state = RegistryState::Unavailable;
        document.libraries.clear();
        bytes = serde_json::to_vec(&document)?;
        failed = true;
    }
    let mut output = std::io::stdout().lock();
    output.write_all(&bytes)?;
    output.write_all(b"\n")?;
    output.flush()?;
    if failed {
        anyhow::bail!("local registry enumeration unavailable");
    }
    Ok(())
}
