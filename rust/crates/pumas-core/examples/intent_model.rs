use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, ModelRequirement, ModelSelector, ObservedModelState,
};
use pumas_library::models::PumasModelRef;
use pumas_library::PumasApi;

#[tokio::main]
async fn main() -> pumas_library::Result<()> {
    let mut arguments = std::env::args().skip(1);
    let root = arguments
        .next()
        .expect("usage: intent_model <launcher-root> <model-id>");
    let model_id = arguments
        .next()
        .expect("usage: intent_model <launcher-root> <model-id>");
    let api = PumasApi::builder(root)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await?;
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

    match api.intent().get_model(&requirement).await? {
        ObservedModelState::Available { handle } => {
            println!("{}", handle.local_load_path);
        }
        state => println!("{state:?}"),
    }
    Ok(())
}
