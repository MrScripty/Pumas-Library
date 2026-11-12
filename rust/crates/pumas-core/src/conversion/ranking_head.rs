//! Ranking-head derivation: copy a causal-LM GGUF and add the llama.cpp
//! ranking head described by the model's recorded `LogitScore` declaration
//! (ADR 0006, ADR 0007).
//!
//! The declarations are the model's validated sentence-transformers record;
//! this module hands the scoring part to the derivation script, which owns
//! the GGUF checks, head layout and template rendering, and then publishes
//! and records the derived reranker.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio::fs;
use tokio::process::Command;
use tracing::info;

use super::outputs::OutputWorkspace;
use super::progress::ConversionProgressTracker;
use super::script_process;
use super::scripts;
use super::types::{ConversionSource, ConversionStatus};
use crate::cancel::CancellationToken;
use crate::model_library::declarations::SentenceTransformersDeclarations;
use crate::model_library::ModelLibrary;
use crate::models::ModelMetadata;
use crate::{PumasError, Result};

/// Inputs of one derivation, resolved at admission.
pub(super) struct RankingHeadJob {
    pub conversion_id: String,
    pub launcher_root: PathBuf,
    pub source_model_id: String,
    pub source_file: PathBuf,
    pub source_metadata: ModelMetadata,
    pub contract: ScoringContract,
}

/// The scoring part of a model's declarations, as the script consumes it.
#[derive(Debug, Clone, Serialize)]
pub(super) struct ScoringContract {
    true_token_id: u32,
    true_token: String,
    false_token_id: Option<u32>,
    false_token: Option<String>,
    chat_template: String,
    chat_template_options: BTreeMap<String, bool>,
    special_tokens: BTreeMap<String, String>,
}

impl ScoringContract {
    /// The scoring contract a derivation needs, or the reason it is absent.
    pub(super) fn from_declarations(
        model_id: &str,
        declarations: Option<&SentenceTransformersDeclarations>,
    ) -> Result<Self> {
        let refusal = |reason: &str| PumasError::InvalidParams {
            message: format!(
                "Ranking-head derivation needs a LogitScore declaration; {model_id} {reason}. \
                 Resolve its declarations (resolve_model_declarations), naming the repository \
                 that publishes them if its lineage does not record one"
            ),
        };
        let declarations = declarations.ok_or_else(|| refusal("has no recorded declarations"))?;
        let score = declarations
            .logit_score
            .as_ref()
            .ok_or_else(|| refusal("declares no LogitScore module"))?;
        let chat_template = declarations
            .chat_template
            .clone()
            .ok_or_else(|| refusal("declares no chat template"))?;
        Ok(Self {
            true_token_id: score.true_token_id,
            true_token: score.true_token.clone(),
            false_token_id: score.false_token_id,
            false_token: score.false_token.clone(),
            chat_template,
            chat_template_options: declarations.chat_template_options.clone(),
            special_tokens: declarations.special_tokens.clone(),
        })
    }
}

fn output_dir_for(source_dir: &Path) -> Result<PathBuf> {
    let name = source_dir
        .file_name()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: format!("Invalid model path: {}", source_dir.display()),
        })?
        .to_string_lossy();
    let parent = source_dir
        .parent()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: format!("Model path has no parent: {}", source_dir.display()),
        })?;
    Ok(parent.join(format!("{name}-ranking")))
}

/// Run one derivation to a published, indexed reranker model.
pub(super) async fn run(
    job: RankingHeadJob,
    progress: &ConversionProgressTracker,
    cancel: &CancellationToken,
    library: &ModelLibrary,
) -> Result<()> {
    let python = scripts::venv_python(&job.launcher_root);
    if !fs::try_exists(&python)
        .await
        .map_err(|error| PumasError::io("checking conversion python", &python, error))?
    {
        return Err(PumasError::ConversionFailed {
            message: "Conversion environment not set up. Call setup_conversion_environment first."
                .into(),
        });
    }
    scripts::ensure_scripts_deployed(&job.launcher_root).await?;
    progress.set_status(&job.conversion_id, ConversionStatus::Validating);

    let source_dir = job
        .source_file
        .parent()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: "The source GGUF has no package directory".into(),
        })?
        .to_path_buf();
    let file_name = job
        .source_file
        .file_name()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: "The source GGUF has no file name".into(),
        })?;
    let workspace = OutputWorkspace::prepare(&output_dir_for(&source_dir)?).await?;
    let output_file = workspace.staging_path().join(file_name);

    // The contract and the script's report live in a per-job directory outside
    // the library, so model classification never sees them.
    let scripts_dir = scripts::scripts_dir(&job.launcher_root);
    let work = tokio::task::spawn_blocking(move || {
        tempfile::Builder::new()
            .prefix("pumas-ranking-head-")
            .tempdir_in(&scripts_dir)
            .map_err(|error| {
                PumasError::io("allocating derivation work directory", &scripts_dir, error)
            })
    })
    .await
    .map_err(|error| PumasError::ConversionFailed {
        message: format!("Derivation work directory worker failed: {error}"),
    })??;
    let contract_path = work.path().join("contract.json");
    let contract = serde_json::to_vec(&job.contract)?;
    fs::write(&contract_path, contract)
        .await
        .map_err(|error| PumasError::io("writing scoring contract", &contract_path, error))?;
    let report_path = work.path().join("report.json");

    info!(
        "Starting ranking-head derivation {} for {}",
        job.conversion_id, job.source_model_id
    );
    let mut command = Command::new(&python);
    command
        .arg(scripts::scripts_dir(&job.launcher_root).join(scripts::RANKING_HEAD_SCRIPT_NAME))
        .arg("--input")
        .arg(&job.source_file)
        .arg("--contract")
        .arg(&contract_path)
        .arg("--output")
        .arg(&output_file)
        .arg("--report")
        .arg(&report_path);
    script_process::run(
        &mut command,
        "ranking head derivation",
        &job.conversion_id,
        progress,
        cancel,
    )
    .await?;
    let report = read_report(&report_path).await?;
    if report.true_token_id != job.contract.true_token_id
        || report.false_token_id != job.contract.false_token_id
    {
        return Err(PumasError::ConversionFailed {
            message: "The derivation script reported a head for different tokens".into(),
        });
    }
    drop(work);

    let output_dir = workspace.publish().await?;
    progress.set_status(&job.conversion_id, ConversionStatus::Importing);
    let metadata = derived_metadata(&job.source_model_id, &job.source_metadata);
    library.save_metadata(&output_dir, &metadata).await?;
    library.index_model_dir(&output_dir).await?;
    let output_model_id = library
        .get_relative_path(&output_dir)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| output_dir.to_string_lossy().to_string());
    progress.set_output_model_id(&job.conversion_id, output_model_id);
    info!("Ranking-head derivation {} completed", job.conversion_id);
    Ok(())
}

/// What the script built, as it reports it.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivationReport {
    #[allow(dead_code)]
    architecture: String,
    true_token_id: u32,
    false_token_id: Option<u32>,
    #[allow(dead_code)]
    labels: [String; 2],
}

async fn read_report(path: &Path) -> Result<DerivationReport> {
    let bytes = fs::read(path)
        .await
        .map_err(|error| PumasError::io("reading derivation report", path, error))?;
    serde_json::from_slice(&bytes).map_err(|error| PumasError::ConversionFailed {
        message: format!("The derivation script wrote an invalid report: {error}"),
    })
}

/// The derived reranker keeps the source's declarations: it scores by the
/// same contract, and the record names where that contract was read. It does
/// not claim the source's repository: it is not a file that repository
/// publishes, and its lineage is `conversion_source` and `base_model`.
fn derived_metadata(source_model_id: &str, source: &ModelMetadata) -> ModelMetadata {
    ModelMetadata {
        model_id: source.model_id.clone(),
        family: source.family.clone(),
        model_type: Some("reranker".into()),
        official_name: source
            .official_name
            .as_ref()
            .map(|name| format!("{name} (ranking head)")),
        tags: Some(
            source
                .tags
                .clone()
                .unwrap_or_default()
                .into_iter()
                .chain(["converted".to_string(), "ranking-head".to_string()])
                .collect(),
        ),
        match_source: Some("conversion".into()),
        schema_version: source.schema_version,
        pipeline_tag: Some("text-ranking".into()),
        task_type_primary: Some("text-ranking".into()),
        base_model: source.base_model.clone(),
        selected_artifact_quant: source.selected_artifact_quant.clone(),
        sentence_transformers: source.sentence_transformers.clone(),
        conversion_source: Some(ConversionSource {
            source_model_id: source_model_id.to_string(),
            source_format: "gguf".into(),
            source_quant: source.selected_artifact_quant.clone(),
            target_format: "gguf".into(),
            target_quant: source.selected_artifact_quant.clone(),
            was_dequantized: false,
            conversion_date: chrono::Utc::now().to_rfc3339(),
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_library::declarations::{
        DeclarationSource, LogitScore, SentenceTransformersKind,
    };

    fn declared(
        logit: Option<LogitScore>,
        template: Option<&str>,
    ) -> SentenceTransformersDeclarations {
        SentenceTransformersDeclarations {
            source: DeclarationSource::Package,
            model_kind: SentenceTransformersKind::CrossEncoder,
            modules: Vec::new(),
            module_details: Vec::new(),
            pooling: None,
            normalize: false,
            logit_score: logit,
            activation: None,
            chat_template: template.map(str::to_string),
            chat_template_options: Default::default(),
            special_tokens: Default::default(),
            prompts: Default::default(),
            default_prompt_name: None,
            max_seq_length: None,
        }
    }

    fn reason(error: PumasError) -> String {
        match error {
            PumasError::InvalidParams { message } => message,
            other => panic!("expected a typed refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_contract_is_the_declared_logit_score_and_template() {
        let score = LogitScore {
            true_token_id: 9454,
            true_token: "Yes".into(),
            false_token_id: None,
            false_token: None,
        };
        let contract =
            ScoringContract::from_declarations("m", Some(&declared(Some(score), Some("{{ x }}"))))
                .unwrap();
        let value = serde_json::to_value(&contract).unwrap();
        assert_eq!(value["true_token_id"], 9454);
        assert_eq!(value["false_token_id"], serde_json::Value::Null);
        assert_eq!(value["chat_template"], "{{ x }}");
    }

    #[test]
    fn a_missing_declaration_is_refused_with_its_reason() {
        assert!(
            reason(ScoringContract::from_declarations("m", None).unwrap_err())
                .contains("has no recorded declarations")
        );
        assert!(reason(
            ScoringContract::from_declarations("m", Some(&declared(None, Some("t")))).unwrap_err()
        )
        .contains("declares no LogitScore"));
    }

    #[test]
    fn the_output_sits_beside_the_source_package() {
        assert_eq!(
            output_dir_for(Path::new("/lib/reranker/qwen3/model-gguf")).unwrap(),
            Path::new("/lib/reranker/qwen3/model-gguf-ranking")
        );
    }
}
