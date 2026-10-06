//! Evidence-only external crate. No production API or workspace manifest edits.
use pumas_library::acquisition::*;
use pumas_library::error::{PumasError, Result};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

struct Host;
#[async_trait::async_trait]
impl HttpAttemptHost for Host {
    async fn pause_requested(&self) {
        std::future::pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        false
    }
    fn cancel_requested(&self) -> bool {
        false
    }
    async fn record_progress(&mut self, _: u64) -> Result<()> {
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for Host {
    async fn retry(&mut self, _: u32, _: Option<Duration>, _: Option<&str>) -> Result<()> {
        Ok(())
    }
}
fn failure(message: impl ToString) -> PumasError {
    PumasError::Other(message.to_string())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| failure(format!("missing {key}")))
}
fn approved(url: &str, allow: &HashSet<String>) -> bool {
    reqwest::Url::parse(url).ok().is_some_and(|u| {
        u.scheme() == "http"
            && u.host_str() == Some("127.0.0.1")
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && allow.contains(url)
    })
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let spec_path = PathBuf::from(&args[1]);
    let directory = spec_path
        .parent()
        .ok_or_else(|| failure("missing parent"))?
        .to_owned();
    let spec: Value = serde_json::from_slice(&std::fs::read(&spec_path)?)?;
    let items = spec["candidates"]
        .as_array()
        .ok_or_else(|| failure("missing candidates"))?;
    if items.is_empty()
        || items.len() > 64
        || items.iter().any(|item| {
            !item["bytes"]
                .as_u64()
                .is_some_and(|size| size > 0 && size <= 1024 * 1024)
        })
    {
        return Err(failure(
            "catalog exceeds declared fixture inspection budget",
        ));
    }
    let allow: HashSet<String> = spec["approved_urls"]
        .as_array()
        .ok_or_else(|| failure("missing owner source grant"))?
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_owned())
        .collect();
    // Deliberately narrower anonymous fixture authority; no production HTTP change.
    if items
        .iter()
        .any(|item| !approved(item["url"].as_str().unwrap_or_default(), &allow))
    {
        std::fs::write(
            directory.join("outcome.json"),
            b"{\"status\":\"owner_source_refused\"}",
        )?;
        return Ok(());
    }
    let redirect_allow = allow.clone();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() < 3 && approved(attempt.url().as_str(), &redirect_allow) {
                attempt.follow()
            } else {
                attempt.error("owner refused redirect")
            }
        }))
        .build()
        .map_err(failure)?;
    let files: Vec<_> = items
        .iter()
        .map(|item| {
            ArtifactFile::new(
                text(item, "filename")?,
                text(item, "id")?,
                item["bytes"].as_u64(),
                Some(
                    Sha256Evidence::new("experiment.owner.sha256", text(item, "sha256")?)
                        .map_err(failure)?,
                ),
                FileVerificationRequirement::Sha256,
            )
            .map_err(failure)
        })
        .collect::<Result<_>>()?;
    let manifest = ArtifactManifest::new(
        ArtifactSourceIdentity::new(
            "experiment",
            format!("offline-catalog.{}", text(&spec, "case")?),
            ArtifactRevisionEvidence::new(
                "experiment.owner.catalog",
                text(&spec, "catalog_digest")?,
                RevisionStrength::Weak,
            )
            .map_err(failure)?,
        )
        .map_err(failure)?,
        files,
    )
    .map_err(failure)?;
    let root = directory.join("acquisition");
    let wheels = root.join("catalog");
    std::fs::create_dir_all(&wheels)?;
    let workspace =
        ReservedDirectory::capture(&root, Path::new("catalog"), Arc::new(()), || Ok(()))?
            .acquisition_workspace()?;
    let store = Arc::new(AcquisitionStore::new(&root));
    let service = Arc::new(AcquisitionService::new(store.clone()));
    let consumer = service.open_consumer("experiment.torch.offline-catalog")?;
    let request = AcquisitionHttpRequest {
        demand: AcquisitionDemand {
            consumer: consumer.owner().into(),
            operation: text(&spec, "case")?.into(),
        },
        manifest,
        workspace,
        sources: items
            .iter()
            .map(|item| {
                Ok(AcquisitionHttpSource {
                    url: text(item, "url")?.into(),
                    authorization: None,
                })
            })
            .collect::<Result<_>>()?,
        retry: AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::from_secs(10),
            backoff: pumas_library::network::RetryConfig::new(),
        },
    };
    let python = args[2].clone();
    let script = args[3].clone();
    let uv = args[4].clone();
    let packaging = args[5].clone();
    let publish_directory = directory.clone();
    let result = consumer
        .acquire_http(
            request,
            client,
            Box::new(Host),
            move |inputs| async move {
                let record = serde_json::to_value(inputs.record())?;
                // Verify every descriptor through the current held capability before child work.
                for index in 0..inputs.record().files.len() {
                    drop(inputs.open_file(index).await?);
                }
                let payload = inputs
                    .run_blocking(
                        "inspect acquired catalog and run offline public uv",
                        move || {
                            let output = std::process::Command::new(&python)
                                .arg("-S")
                                .arg(&script)
                                .arg("inspect")
                                .arg(&spec_path)
                                .arg(&uv)
                                .arg(&wheels)
                                .env_clear()
                                .env("PATH", "/usr/bin:/bin")
                                .env("LANG", "C.UTF-8")
                                .env("PYTHONPATH", &packaging)
                                .output()?;
                            std::fs::write(directory.join("inspection.stdout"), &output.stdout)?;
                            std::fs::write(directory.join("inspection.stderr"), &output.stderr)?;
                            if !output.status.success() {
                                return Err(failure(
                                    "inspection process failed; no solver fallback",
                                ));
                            }
                            let mut payload: Value = serde_json::from_slice(&output.stdout)?;
                            payload["held_use_record"] = record;
                            Ok(payload)
                        },
                    )
                    .await?;
                Ok((payload.clone(), payload))
            },
            move |payload, receipt| async move {
                std::fs::write(
                    publish_directory.join("completion-receipt.json"),
                    serde_json::to_vec_pretty(&receipt)?,
                )?;
                Ok(payload)
            },
        )
        .await;
    consumer.shutdown().await?;
    service.shutdown().await?;
    let outcome = match result {
        Ok(payload) => json!({"status":"experiment_completed", "payload":payload,
            "settled_records":store.acquisitions()?.into_values().collect::<Vec<_>>()}),
        Err(error) => {
            json!({"status":"acquisition_or_inspection_refused", "error":error.to_string(),
            "retained_records":store.acquisitions()?.into_values().collect::<Vec<_>>()})
        }
    };
    std::fs::write(
        publish_directory_path(&args[1])?.join("outcome.json"),
        serde_json::to_vec_pretty(&outcome)?,
    )?;
    Ok(())
}
fn publish_directory_path(path: &str) -> Result<PathBuf> {
    Path::new(path)
        .parent()
        .map(Path::to_owned)
        .ok_or_else(|| failure("missing parent"))
}
