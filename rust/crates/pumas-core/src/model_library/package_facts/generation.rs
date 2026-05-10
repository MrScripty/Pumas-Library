use crate::error::{PumasError, Result};
use crate::models::{GenerationDefaultFacts, ModelPackageDiagnostic, PackageFactStatus};
use serde_json::Value;
use std::path::Path;

pub(crate) async fn generation_default_facts(model_dir: &Path) -> Result<GenerationDefaultFacts> {
    let generation_config_path = model_dir.join("generation_config.json");
    if !tokio::fs::try_exists(&generation_config_path).await? {
        return legacy_generation_default_facts_from_config(model_dir).await;
    }

    let parsed = tokio::task::spawn_blocking(move || {
        std::fs::read_to_string(generation_config_path)
            .map_err(|err| err.to_string())
            .and_then(|raw| serde_json::from_str::<Value>(&raw).map_err(|err| err.to_string()))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join generation config parse: {}", err)))?;

    match parsed {
        Ok(defaults) => Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Present,
            source_path: Some("generation_config.json".to_string()),
            defaults: Some(defaults),
            diagnostics: Vec::new(),
        }),
        Err(message) => Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Invalid,
            source_path: Some("generation_config.json".to_string()),
            defaults: None,
            diagnostics: vec![ModelPackageDiagnostic {
                code: "invalid_generation_config_json".to_string(),
                message,
                path: Some("generation_config.json".to_string()),
            }],
        }),
    }
}

async fn legacy_generation_default_facts_from_config(
    model_dir: &Path,
) -> Result<GenerationDefaultFacts> {
    let config_path = model_dir.join("config.json");
    if !tokio::fs::try_exists(&config_path).await? {
        return Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Missing,
            source_path: None,
            defaults: None,
            diagnostics: Vec::new(),
        });
    }

    let parsed = tokio::task::spawn_blocking(move || {
        std::fs::read_to_string(config_path)
            .map_err(|err| err.to_string())
            .and_then(|raw| serde_json::from_str::<Value>(&raw).map_err(|err| err.to_string()))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join config generation parse: {}", err)))?;

    let config = match parsed {
        Ok(config) => config,
        Err(message) => {
            return Ok(GenerationDefaultFacts {
                status: PackageFactStatus::Invalid,
                source_path: Some("config.json".to_string()),
                defaults: None,
                diagnostics: vec![ModelPackageDiagnostic {
                    code: "invalid_config_json".to_string(),
                    message,
                    path: Some("config.json".to_string()),
                }],
            });
        }
    };

    let Some(config) = config.as_object() else {
        return Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Missing,
            source_path: Some("config.json".to_string()),
            defaults: None,
            diagnostics: Vec::new(),
        });
    };

    let mut defaults = serde_json::Map::new();
    for key in LEGACY_CONFIG_GENERATION_KEYS {
        if let Some(value) = config.get(*key) {
            defaults.insert((*key).to_string(), value.clone());
        }
    }

    if defaults.is_empty() {
        return Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Missing,
            source_path: Some("config.json".to_string()),
            defaults: None,
            diagnostics: Vec::new(),
        });
    }

    Ok(GenerationDefaultFacts {
        status: PackageFactStatus::Present,
        source_path: Some("config.json".to_string()),
        defaults: Some(Value::Object(defaults)),
        diagnostics: vec![ModelPackageDiagnostic {
            code: "legacy_config_generation_defaults".to_string(),
            message: "generation defaults were extracted from config.json because generation_config.json is absent".to_string(),
            path: Some("config.json".to_string()),
        }],
    })
}

const LEGACY_CONFIG_GENERATION_KEYS: &[&str] = &[
    "max_length",
    "max_new_tokens",
    "min_length",
    "min_new_tokens",
    "early_stopping",
    "max_time",
    "do_sample",
    "num_beams",
    "num_beam_groups",
    "penalty_alpha",
    "use_cache",
    "temperature",
    "top_k",
    "top_p",
    "typical_p",
    "epsilon_cutoff",
    "eta_cutoff",
    "diversity_penalty",
    "repetition_penalty",
    "encoder_repetition_penalty",
    "length_penalty",
    "no_repeat_ngram_size",
    "bad_words_ids",
    "force_words_ids",
    "renormalize_logits",
    "constraints",
    "forced_bos_token_id",
    "forced_eos_token_id",
    "remove_invalid_values",
    "exponential_decay_length_penalty",
    "suppress_tokens",
    "begin_suppress_tokens",
    "forced_decoder_ids",
    "sequence_bias",
    "token_healing",
    "guidance_scale",
    "low_memory",
    "num_return_sequences",
    "output_attentions",
    "output_hidden_states",
    "output_scores",
    "output_logits",
    "return_dict_in_generate",
    "pad_token_id",
    "bos_token_id",
    "eos_token_id",
    "encoder_no_repeat_ngram_size",
    "decoder_start_token_id",
];
