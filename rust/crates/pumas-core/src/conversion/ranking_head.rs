//! Ranking-head derivation: copy a causal-LM GGUF and add the llama.cpp
//! ranking head described by the model's sentence-transformers `LogitScore`
//! scoring contract (ADR 0006).
//!
//! The contract is read from the source package when it holds one, otherwise
//! fetched by a [`ScoringContractSource`] from a named repository or the
//! source model's single recorded base model. The derivation script owns the
//! contract semantics and every refusal; this module owns acquisition,
//! staging, publication and provenance.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs;
use tokio::process::Command;
use tracing::info;

use super::outputs::OutputWorkspace;
use super::progress::ConversionProgressTracker;
use super::script_process;
use super::scripts;
use super::types::{ConversionSource, ConversionStatus, ScoringContractOrigin};
use crate::cancel::CancellationToken;
use crate::model_library::ModelLibrary;
use crate::models::ModelMetadata;
use crate::{PumasError, Result};

/// Repository identity recorded when the source package supplied the contract.
pub const SOURCE_PACKAGE_CONTRACT: &str = "source_package";

/// Marker file whose presence means a package carries a sentence-transformers
/// module list; the script decides whether it is a usable `LogitScore` contract.
const CONTRACT_MARKER: &str = "modules.json";

/// Fetches a scoring contract's files into a local directory.
#[async_trait::async_trait]
pub trait ScoringContractSource: Send + Sync {
    /// Write the contract files of `repo_id` into `destination` (which exists
    /// and is empty) and return the immutable revision they were read at.
    /// Implementations check `cancel` between files and return
    /// `ConversionCancelled` once it is set.
    async fn fetch_scoring_contract(
        &self,
        repo_id: &str,
        destination: &Path,
        cancel: &CancellationToken,
    ) -> Result<String>;
}

/// Inputs of one derivation, resolved at admission.
pub(super) struct RankingHeadJob {
    pub conversion_id: String,
    pub launcher_root: PathBuf,
    pub source_model_id: String,
    pub source_file: PathBuf,
    pub source_metadata: ModelMetadata,
    pub contract: ContractLocation,
}

/// Where the contract will be read from, decided before work starts.
pub(super) enum ContractLocation {
    SourcePackage(PathBuf),
    Repository {
        repo_id: String,
        source: Arc<dyn ScoringContractSource>,
    },
}

/// Decide where a derivation's contract comes from, refusing with the reason
/// when none can be named. Pure apart from one existence check.
pub(super) async fn locate_contract(
    model_dir: &Path,
    metadata: &ModelMetadata,
    requested_repo: Option<&str>,
    source: Option<Arc<dyn ScoringContractSource>>,
) -> Result<ContractLocation> {
    if requested_repo.is_none() {
        let marker = model_dir.join(CONTRACT_MARKER);
        if fs::try_exists(&marker)
            .await
            .map_err(|error| PumasError::io("checking scoring contract", &marker, error))?
        {
            return Ok(ContractLocation::SourcePackage(model_dir.to_path_buf()));
        }
    }
    let repo_id = match requested_repo {
        Some(repo) => repo.to_string(),
        None => match metadata.base_model.as_deref() {
            Some([single]) if !single.trim().is_empty() => single.trim().to_string(),
            Some([]) | None => {
                return Err(PumasError::InvalidParams {
                    message: "The source package has no scoring contract and its model records \
                              no base model; name the contract with scoring_contract_repo"
                        .into(),
                })
            }
            Some(_) => {
                return Err(PumasError::InvalidParams {
                    message: "The source model records several base models; name the scoring \
                              contract with scoring_contract_repo"
                        .into(),
                })
            }
        },
    };
    let source = source.ok_or_else(|| PumasError::InvalidParams {
        message: format!(
            "The scoring contract must be fetched from {repo_id}, but Hugging Face access is \
             disabled for this library instance"
        ),
    })?;
    Ok(ContractLocation::Repository { repo_id, source })
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
    let workspace = OutputWorkspace::prepare(&output_dir_for(&source_dir)?).await?;
    let file_name = job
        .source_file
        .file_name()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: "The source GGUF has no file name".into(),
        })?;
    let output_file = workspace.staging_path().join(file_name);

    // Fetched contract files and the script's report live in a per-job
    // directory outside the library, so model classification never sees them.
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
    let (contract_dir, repo_id, revision) = match &job.contract {
        ContractLocation::SourcePackage(dir) => {
            (dir.clone(), SOURCE_PACKAGE_CONTRACT.to_string(), None)
        }
        ContractLocation::Repository { repo_id, source } => {
            let dir = work.path().join("contract");
            fs::create_dir(&dir).await.map_err(|error| {
                PumasError::io("creating scoring contract directory", &dir, error)
            })?;
            let revision = source.fetch_scoring_contract(repo_id, &dir, cancel).await?;
            (dir, repo_id.clone(), Some(revision))
        }
    };
    let report_path = work.path().join("report.json");

    info!(
        "Starting ranking-head derivation {} for {} with contract {}",
        job.conversion_id, job.source_model_id, repo_id
    );
    let mut command = Command::new(&python);
    command
        .arg(scripts::scripts_dir(&job.launcher_root).join(scripts::RANKING_HEAD_SCRIPT_NAME))
        .arg("--input")
        .arg(&job.source_file)
        .arg("--contract")
        .arg(&contract_dir)
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
    drop(work);

    let output_dir = workspace.publish().await?;
    progress.set_status(&job.conversion_id, ConversionStatus::Importing);
    let metadata = derived_metadata(
        &job.source_model_id,
        &job.source_metadata,
        ScoringContractOrigin {
            repo_id,
            revision,
            true_token_id: report.true_token_id,
            false_token_id: report.false_token_id,
        },
    );
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

/// What the script built, as it reports it. The script owns contract
/// semantics; this only decodes its bounded, closed report.
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

fn derived_metadata(
    source_model_id: &str,
    source: &ModelMetadata,
    contract: ScoringContractOrigin,
) -> ModelMetadata {
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
        repo_id: source.repo_id.clone(),
        base_model: source.base_model.clone(),
        selected_artifact_quant: source.selected_artifact_quant.clone(),
        conversion_source: Some(ConversionSource {
            source_model_id: source_model_id.to_string(),
            source_format: "gguf".into(),
            source_quant: source.selected_artifact_quant.clone(),
            target_format: "gguf".into(),
            target_quant: source.selected_artifact_quant.clone(),
            was_dequantized: false,
            conversion_date: chrono::Utc::now().to_rfc3339(),
            scoring_contract: Some(contract),
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoFetch;

    #[async_trait::async_trait]
    impl ScoringContractSource for NoFetch {
        async fn fetch_scoring_contract(
            &self,
            _: &str,
            _: &Path,
            _: &CancellationToken,
        ) -> Result<String> {
            unreachable!("location tests never fetch")
        }
    }

    fn metadata(base_model: Option<Vec<&str>>) -> ModelMetadata {
        ModelMetadata {
            base_model: base_model.map(|items| items.into_iter().map(String::from).collect()),
            ..Default::default()
        }
    }

    fn reason(error: PumasError) -> String {
        match error {
            PumasError::InvalidParams { message } => message,
            other => panic!("expected a typed refusal, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_package_contract_wins_unless_a_repository_is_named() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONTRACT_MARKER), "[]").unwrap();
        let located = locate_contract(dir.path(), &metadata(Some(vec!["a/b"])), None, None)
            .await
            .unwrap();
        assert!(matches!(located, ContractLocation::SourcePackage(path) if path == dir.path()));
        let named = locate_contract(
            dir.path(),
            &metadata(None),
            Some("org/contract"),
            Some(Arc::new(NoFetch)),
        )
        .await
        .unwrap();
        assert!(
            matches!(named, ContractLocation::Repository { repo_id, .. } if repo_id == "org/contract")
        );
    }

    #[tokio::test]
    async fn the_single_base_model_names_the_contract() {
        let dir = tempfile::tempdir().unwrap();
        let located = locate_contract(
            dir.path(),
            &metadata(Some(vec![" zeroentropy/zerank-2-reranker "])),
            None,
            Some(Arc::new(NoFetch)),
        )
        .await
        .unwrap();
        assert!(
            matches!(located, ContractLocation::Repository { repo_id, .. }
            if repo_id == "zeroentropy/zerank-2-reranker")
        );
    }

    #[tokio::test]
    async fn an_unnamed_contract_is_refused_with_its_reason() {
        let dir = tempfile::tempdir().unwrap();
        let none = metadata(None);
        let missing = locate_contract(dir.path(), &none, None, Some(Arc::new(NoFetch)));
        assert!(reason(missing.await.err().unwrap()).contains("records no base model"));
        let two = metadata(Some(vec!["a/b", "c/d"]));
        let several = locate_contract(dir.path(), &two, None, Some(Arc::new(NoFetch)));
        assert!(reason(several.await.err().unwrap()).contains("several base models"));
        let one = metadata(Some(vec!["a/b"]));
        let offline = locate_contract(dir.path(), &one, None, None);
        assert!(reason(offline.await.err().unwrap()).contains("Hugging Face access is disabled"));
    }

    #[test]
    fn the_output_sits_beside_the_source_package() {
        assert_eq!(
            output_dir_for(Path::new("/lib/reranker/qwen3/model-gguf")).unwrap(),
            Path::new("/lib/reranker/qwen3/model-gguf-ranking")
        );
    }
}
