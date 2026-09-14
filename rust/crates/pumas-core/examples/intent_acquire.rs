//! Acquire one unambiguous GGUF artifact and poll using the returned pinned requirement.

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, ModelRequirement, ModelSelector, ObservedModelState,
};
use pumas_library::models::PackageArtifactKind;
use pumas_library::{PumasApi, PumasError, Result};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let root = arguments
        .next()
        .expect("usage: intent_acquire <launcher-root> <repository-id> [revision]");
    let repository_id = arguments.next().expect("repository-id is required");
    let revision = arguments.next();
    let api = PumasApi::builder(root)
        .auto_create_dirs(true)
        .with_process_manager(false)
        .build()
        .await?;
    let requirement = ModelRequirement {
        selector: ModelSelector::UpstreamRepository {
            repository_id,
            revision,
        },
        artifact: ArtifactRequirement {
            format: Some(PackageArtifactKind::Gguf),
            ..ArtifactRequirement::default()
        },
        acquisition_policy: AcquisitionPolicy::AllowUpstream,
    };
    let result = tokio::time::timeout(Duration::from_secs(180), async {
        let mut state = api.intent().get_model(&requirement).await?;
        loop {
            match state {
                ObservedModelState::Available { handle } => {
                    println!("{}", handle.local_load_path);
                    println!("revision: {:?}", handle.identity.model_ref.revision);
                    return Ok(());
                }
                ObservedModelState::Acquiring {
                    resolved_requirement,
                    ..
                } => {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    state = api.intent().get_model_status(&resolved_requirement).await?;
                }
                other => return Err(PumasError::Other(format!("Acquisition stopped: {other:?}"))),
            }
        }
    })
    .await
    .map_err(|_| PumasError::Other("Acquisition timed out".into()))
    .and_then(|value| value);
    // Await owned work even when selection, transfer, or verification fails.
    let shutdown = api.shutdown_downloads().await;
    result?;
    shutdown
}
