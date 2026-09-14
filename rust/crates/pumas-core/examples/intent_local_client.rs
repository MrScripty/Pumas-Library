//! Connect to a running local Pumas owner and persist one local model intent.

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, EnsureModelRequest, ModelRequirement, ModelSelector,
};
use pumas_library::models::PumasModelRef;
use pumas_library::{PumasError, PumasLocalClient, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let root = arguments
        .next()
        .expect("usage: intent_local_client <launcher-root> <consumer-key> <model-id>");
    let consumer_key = arguments
        .next()
        .expect("consumer-key is required for a durable declaration");
    let model_id = arguments.next().expect("model-id is required");
    let root = std::path::PathBuf::from(root)
        .canonicalize()
        .map_err(|error| PumasError::Other(format!("invalid launcher root: {error}")))?;
    let instance = PumasLocalClient::discover_ready_instances()?
        .into_iter()
        .find(|instance| instance.library_path == root)
        .ok_or_else(|| PumasError::Other("no ready local owner for launcher root".to_string()))?;
    let client = PumasLocalClient::connect(instance).await?;
    let requirement = ModelRequirement {
        selector: ModelSelector::LocalModel {
            model_ref: PumasModelRef {
                model_id,
                ..PumasModelRef::default()
            },
        },
        artifact: ArtifactRequirement::default(),
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    };

    println!(
        "current: {:#?}",
        client.intent().get_model(&requirement).await?
    );
    let ensured = client
        .intent()
        .ensure_model(&EnsureModelRequest {
            consumer_key,
            requirement,
        })
        .await?;
    println!("ensure: {ensured:#?}");
    println!(
        "declarations: {:#?}",
        client.intent().list_declarations().await?
    );
    Ok(())
}
