//! What a served model accepts and produces, as its runtime declares it
//! (ADR 0005). Read-only and bounded; this is a runtime declaration, not
//! model-byte or semantic attestation.
//!
//! llama.cpp reports a model's declared modalities in `/v1/models`
//! (`architecture.input_modalities`/`output_modalities`, from the GGUF and its
//! projector, for loaded and unloaded router models alike) and the loaded
//! session's live ones in `/props` (`modalities.vision`/`audio`/`video`).
//! Other providers declare a fixed set.
use super::super::openai_gateway;
use super::types::{Modality, ModalitySet};
use crate::server::AppState;
use pumas_library::models::{RuntimeProviderId, ServedModelStatus};
use serde_json::Value;
use std::{collections::BTreeSet, sync::OnceLock, time::Duration};

const MAX_MODELS_BYTES: usize = 256 * 1024;
const MAX_PROPS_BYTES: usize = 64 * 1024;

/// One observation of a served model's modalities.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Observation {
    /// What the model accepts and produces.
    pub declared: ModalitySet,
    /// Non-text inputs the loaded session reports ready now.
    pub live: BTreeSet<Modality>,
    /// The runtime reports the model as a native decision model.
    pub decisions: bool,
    /// The runtime answered; `false` means its declaration is unknown.
    pub observed: bool,
    /// The runtime published the model's own declaration (llama.cpp
    /// `/v1/models` architecture), not a fixed provider default.
    pub runtime_declared: bool,
}

impl Observation {
    pub fn declares_input(&self, modality: Modality) -> bool {
        self.declared.input.contains(&modality)
    }
    pub fn live(&self, modality: Modality) -> bool {
        modality == Modality::Text || self.live.contains(&modality)
    }
}

/// What the library knows about a served model before asking its runtime.
/// llama.cpp derives non-text modalities from the GGUF and its projector, so a
/// text-only model with no projector is never probed.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Hints<'a> {
    /// Library category; refines llama.cpp's generic `text` output for
    /// embedders and rerankers.
    pub model_type: Option<&'a str>,
    /// A projector sits beside the weights.
    pub projector: bool,
    /// The recorded task implies non-text input (vision, audio, multimodal
    /// embedding).
    pub media_task: bool,
    /// The recorded task or category may be a native decision model.
    pub decision: bool,
}

/// Observe a served model.
pub(super) async fn observe(
    state: &AppState,
    served: &ServedModelStatus,
    hints: Hints<'_>,
) -> Observation {
    let model_type = hints.model_type;
    match served.provider {
        RuntimeProviderId::LlamaCpp => llama_cpp(state, served, hints).await,
        RuntimeProviderId::OnnxRuntime => fixed(
            [Modality::Text],
            [pooled_output(model_type, Modality::Vector)],
        ),
        RuntimeProviderId::Torch if model_type == Some("audio") => {
            fixed([Modality::Audio], [Modality::Text])
        }
        RuntimeProviderId::Torch if matches!(model_type, Some("embedding" | "reranker")) => fixed(
            [Modality::Text],
            [pooled_output(model_type, Modality::Vector)],
        ),
        RuntimeProviderId::Torch => fixed([Modality::Text], [Modality::Image]),
        _ => fixed(
            [Modality::Text],
            [pooled_output(model_type, Modality::Text)],
        ),
    }
}

fn fixed<const I: usize, const O: usize>(
    input: [Modality; I],
    output: [Modality; O],
) -> Observation {
    Observation {
        declared: ModalitySet {
            input: input.to_vec(),
            output: output.to_vec(),
        },
        live: input.into_iter().filter(|m| *m != Modality::Text).collect(),
        decisions: false,
        observed: true,
        runtime_declared: false,
    }
}

/// Embedders produce vectors and rerankers produce scored rankings, although
/// llama.cpp declares both as `text`.
fn pooled_output(model_type: Option<&str>, otherwise: Modality) -> Modality {
    match model_type {
        Some("embedding" | "embeddings") => Modality::Vector,
        Some("reranker") => Modality::Json,
        _ => otherwise,
    }
}

fn content_modality(name: &str) -> Option<Modality> {
    Some(match name {
        "text" => Modality::Text,
        "image" | "vision" => Modality::Image,
        "audio" => Modality::Audio,
        "video" => Modality::Video,
        _ => return None,
    })
}

async fn llama_cpp(state: &AppState, served: &ServedModelStatus, hints: Hints<'_>) -> Observation {
    let model_type = hints.model_type;
    let read_props = hints.projector || hints.media_task;
    let read_models = hints.projector || hints.decision;
    if !read_props && !read_models {
        // No projector and no decision head: text in, text (or vector) out.
        return fixed(
            [Modality::Text],
            [pooled_output(model_type, Modality::Text)],
        );
    }
    let Some(base) = endpoint_base(served) else {
        return Observation::default();
    };
    let provider_id = openai_gateway::provider_request_model_id(served, &state.provider_registry);
    let models_url = format!("{base}/v1/models");
    let props_url = format!("{base}/props");
    let props_query = [
        ("model", served.model_id.as_str()),
        // Never load a model to answer a read-only query.
        ("autoload", "false"),
    ];
    let (models, props) = tokio::join!(
        async {
            if read_models {
                bounded_json(&models_url, &[], MAX_MODELS_BYTES).await
            } else {
                None
            }
        },
        async {
            if read_props {
                bounded_json(&props_url, &props_query, MAX_PROPS_BYTES).await
            } else {
                None
            }
        },
    );
    let live: BTreeSet<Modality> = props
        .filter(|props| {
            props["is_sleeping"].as_bool() == Some(false)
                && props["build_info"]
                    .as_str()
                    .is_some_and(|v| !v.is_empty() && v.len() <= 256)
        })
        .map(|props| {
            props["modalities"]
                .as_object()
                .map(|flags| {
                    flags
                        .iter()
                        .filter(|(_, on)| on.as_bool() == Some(true))
                        .filter_map(|(name, _)| content_modality(name))
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();
    // A router lists every model; a dedicated server lists its one model.
    let entry = models
        .as_ref()
        .and_then(|models| {
            let entries = models["data"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            entries
                .iter()
                .find(|entry| entry["id"].as_str() == Some(provider_id.as_str()))
                .or(match entries {
                    [only] => Some(only),
                    _ => None,
                })
                .cloned()
        })
        // An entry without modality fields (older llama.cpp) declares nothing; the
        // live session stands in for it, exactly as for a missing entry.
        .filter(|entry| {
            ["input_modalities", "output_modalities"]
                .iter()
                .any(|key| entry["architecture"][key].is_array())
        });
    let Some(entry) = entry else {
        // Without a declaration the loaded session's live inputs stand in for
        // it; with neither, the declaration is unknown.
        let observed = !live.is_empty();
        let mut input = live.clone();
        input.insert(Modality::Text);
        return Observation {
            declared: ModalitySet {
                input: input.into_iter().collect(),
                output: vec![pooled_output(model_type, Modality::Text)],
            },
            live,
            decisions: false,
            observed,
            runtime_declared: false,
        };
    };
    let names = |key: &str| -> Vec<String> {
        entry["architecture"][key]
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut input: BTreeSet<Modality> = names("input_modalities")
        .iter()
        .filter_map(|name| content_modality(name))
        .collect();
    input.insert(Modality::Text);
    let outputs = names("output_modalities");
    let decisions = outputs.iter().any(|name| name == "decisions");
    let mut output = BTreeSet::new();
    if decisions {
        output.insert(Modality::Json);
    } else {
        output.insert(pooled_output(model_type, Modality::Text));
        // Speech output is declared by the runtime even though Pumas has no
        // transport for it yet; discovery reports what the model produces.
        output.extend(
            outputs
                .iter()
                .filter_map(|name| content_modality(name))
                .filter(|m| *m != Modality::Text),
        );
    }
    let live = live.into_iter().filter(|m| input.contains(m)).collect();
    Observation {
        declared: ModalitySet {
            input: input.into_iter().collect(),
            output: output.into_iter().collect(),
        },
        live,
        decisions,
        observed: true,
        runtime_declared: true,
    }
}

/// The provider endpoint base, when it can carry appended API paths.
fn endpoint_base(served: &ServedModelStatus) -> Option<String> {
    let endpoint = served.endpoint_url.as_ref()?;
    let base = reqwest::Url::parse(endpoint.as_str()).ok()?;
    // The provider transport appends API paths to the endpoint base, so a
    // query or fragment cannot safely take part in that path contract.
    if base.query().is_some() || base.fragment().is_some() {
        return None;
    }
    Some(endpoint.as_str().trim_end_matches('/').to_owned())
}

async fn bounded_json(url: &str, query: &[(&str, &str)], max_bytes: usize) -> Option<Value> {
    static CLIENT: OnceLock<crate::provider_clients::ProviderHttp> = OnceLock::new();
    let client = CLIENT
        .get_or_init(|| {
            crate::provider_clients::ProviderHttp::build(|builder| {
                builder
                    .connect_timeout(Duration::from_secs(2))
                    .retry(reqwest::retry::never())
            })
            .expect("modality declaration client")
        })
        .for_url(url);
    let read = async {
        let mut response = client.get(url).query(query).send().await.ok()?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > max_bytes as u64)
        {
            return None;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len().checked_add(chunk.len())? > max_bytes {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice::<Value>(&bytes).ok()
    };
    tokio::time::timeout(Duration::from_secs(5), read)
        .await
        .ok()
        .flatten()
}
