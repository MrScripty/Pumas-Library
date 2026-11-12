//! Sentence-transformers declarations: how a model pools, normalizes and
//! scores, as its sentence-transformers package declares it (ADR 0007).
//!
//! Exported formats (GGUF, ONNX) usually omit the package files, so the
//! declarations are resolved once from the model's package or upstream
//! lineage and kept in its metadata. This module is the single parser of
//! those files; runtimes read only the validated record.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PumasError, Result};

/// Validated declarations of one sentence-transformers package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SentenceTransformersDeclarations {
    /// Where the declarations were read.
    pub source: DeclarationSource,
    /// `SentenceTransformer` (embeddings) or `CrossEncoder` (scores).
    pub model_kind: SentenceTransformersKind,
    /// Module types in pipeline order, as declared.
    pub modules: Vec<String>,
    /// Embedding pooling, when the package declares a Pooling module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pooling: Option<Pooling>,
    /// Whether a Normalize module L2-normalizes embeddings.
    pub normalize: bool,
    /// Logit scoring of a causal cross-encoder, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logit_score: Option<LogitScore>,
    /// Score activation (`config_sentence_transformers.json` `activation_fn`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<String>,
    /// Chat template the package formats `query`/`document` messages with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
    /// Options the package passes to its chat template.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub chat_template_options: BTreeMap<String, bool>,
    /// Special-token strings the template may reference (`bos_token`, ...).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub special_tokens: BTreeMap<String, String>,
    /// Named input prompts (for example task instructions for queries).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prompts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_prompt_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_seq_length: Option<u32>,
}

/// Where declarations were read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeclarationSource {
    /// The model's own package directory.
    Package,
    /// A Hugging Face repository at an immutable commit.
    Repository { repo_id: String, revision: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentenceTransformersKind {
    SentenceTransformer,
    CrossEncoder,
}

/// Embedding pooling declared by a sentence-transformers Pooling module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pooling {
    Mean,
    Cls,
    Max,
    MeanSqrtLen,
    WeightedMean,
    LastToken,
}

/// Causal cross-encoder scoring: `ℓ_true − ℓ_false`, or `ℓ_true` alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LogitScore {
    pub true_token_id: u32,
    pub true_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub false_token_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub false_token: Option<String>,
}

const MODULES: &str = "modules.json";
const TRANSFORMER: &[&str] = &[
    "sentence_transformers.models.Transformer",
    "sentence_transformers.base.modules.transformer.Transformer",
];
const POOLING: &[&str] = &[
    "sentence_transformers.models.Pooling",
    "sentence_transformers.sentence_transformer.modules.pooling.Pooling",
];
const NORMALIZE: &[&str] = &[
    "sentence_transformers.models.Normalize",
    "sentence_transformers.sentence_transformer.modules.normalize.Normalize",
];
const LOGIT_SCORE: &str = "sentence_transformers.cross_encoder.modules.logit_score.LogitScore";
const POOLING_MODES: &[(&str, Pooling)] = &[
    ("pooling_mode_mean_tokens", Pooling::Mean),
    ("pooling_mode_cls_token", Pooling::Cls),
    ("pooling_mode_max_tokens", Pooling::Max),
    ("pooling_mode_mean_sqrt_len_tokens", Pooling::MeanSqrtLen),
    ("pooling_mode_weightedmean_tokens", Pooling::WeightedMean),
    ("pooling_mode_lasttoken", Pooling::LastToken),
];

fn unsupported(message: impl Into<String>) -> PumasError {
    PumasError::Validation {
        field: "sentence_transformers".into(),
        message: message.into(),
    }
}

/// Whether `dir` holds a sentence-transformers package to read.
pub fn has_package(dir: &Path) -> bool {
    dir.join(MODULES).is_file()
}

/// Read and validate the sentence-transformers package in `dir`. Blocking.
pub fn read_package(
    dir: &Path,
    source: DeclarationSource,
) -> Result<SentenceTransformersDeclarations> {
    let modules = read_json(dir, MODULES)?;
    let records = modules
        .as_array()
        .ok_or_else(|| unsupported("modules.json must be a list of module records"))?;
    let mut types = Vec::with_capacity(records.len());
    let mut pooling = None;
    let mut normalize = false;
    let mut logit = None;
    for record in records {
        let kind = record
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| unsupported("a modules.json record has no type"))?;
        let path = record
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if POOLING.contains(&kind) {
            pooling = Some(read_pooling(&read_json(dir, &module_file(path)?)?)?);
        } else if NORMALIZE.contains(&kind) {
            normalize = true;
        } else if kind == LOGIT_SCORE {
            if logit.is_some() {
                return Err(unsupported("more than one LogitScore module is declared"));
            }
            logit = Some(read_json(dir, &module_file(path)?)?);
        }
        types.push(kind.to_string());
    }
    if !types
        .first()
        .is_some_and(|kind| TRANSFORMER.contains(&kind.as_str()))
    {
        return Err(unsupported(
            "the package does not start with a Transformer module",
        ));
    }

    let model_config = optional_json(dir, "config_sentence_transformers.json")?;
    let declared_kind = model_config
        .as_ref()
        .and_then(|config| config.get("model_type"))
        .and_then(Value::as_str);
    let model_kind = match (declared_kind, logit.is_some()) {
        (Some("CrossEncoder"), _) | (None, true) => SentenceTransformersKind::CrossEncoder,
        (Some("SentenceTransformer") | None, false) => {
            SentenceTransformersKind::SentenceTransformer
        }
        (Some(other), _) => return Err(unsupported(format!("unknown model_type {other}"))),
    };
    let string_map = |key: &str| -> BTreeMap<String, String> {
        model_config
            .as_ref()
            .and_then(|config| config.get(key))
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    };
    let prompts = string_map("prompts");
    let config_string = |key: &str| {
        model_config
            .as_ref()
            .and_then(|config| config.get(key))
            .and_then(Value::as_str)
            .map(str::to_string)
    };

    let st_config = optional_json(dir, "sentence_bert_config.json")?;
    let max_seq_length = st_config
        .as_ref()
        .and_then(|config| config.get("max_seq_length"))
        .and_then(Value::as_u64)
        .map(|length| u32::try_from(length).map_err(|_| unsupported("max_seq_length does not fit")))
        .transpose()?;
    let mut chat_template_options = BTreeMap::new();
    if let Some(options) = st_config
        .as_ref()
        .and_then(|config| config.get("processing_kwargs"))
        .and_then(|kwargs| kwargs.get("chat_template"))
    {
        let options = options
            .as_object()
            .ok_or_else(|| unsupported("chat-template processing options must be an object"))?;
        for (name, value) in options {
            let value = value.as_bool().ok_or_else(|| {
                unsupported(format!("chat-template option {name} is not a boolean"))
            })?;
            chat_template_options.insert(name.clone(), value);
        }
    }

    let tokenizer_config = optional_json(dir, "tokenizer_config.json")?;
    let special_tokens = tokenizer_config
        .as_ref()
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter(|(name, _)| name.ends_with("_token"))
                .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let chat_template = match std::fs::read_to_string(dir.join("chat_template.jinja")) {
        Ok(template) => Some(template),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => tokenizer_config
            .as_ref()
            .and_then(|config| config.get("chat_template"))
            .and_then(Value::as_str)
            .map(str::to_string),
        Err(error) => return Err(PumasError::io("reading chat_template.jinja", dir, error)),
    };

    let logit_score = logit
        .map(|config| read_logit_score(dir, &config))
        .transpose()?;
    if logit_score.is_some() && chat_template.is_none() {
        return Err(unsupported(
            "a LogitScore cross-encoder must declare its chat template",
        ));
    }
    Ok(SentenceTransformersDeclarations {
        source,
        model_kind,
        modules: types,
        pooling,
        normalize,
        logit_score,
        activation: config_string("activation_fn"),
        chat_template,
        chat_template_options,
        special_tokens,
        prompts,
        default_prompt_name: config_string("default_prompt_name"),
        max_seq_length,
    })
}

/// `<path>/config.json` for a module directory of plain components.
fn module_file(path: &str) -> Result<String> {
    if path.is_empty()
        || path.contains('\\')
        || !Path::new(path)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(unsupported(format!(
            "module path {path:?} is not a package-relative directory"
        )));
    }
    Ok(format!("{path}/config.json"))
}

/// `pooling_mode` names, as newer sentence-transformers packages write them.
const POOLING_MODE_NAMES: &[(&str, Pooling)] = &[
    ("mean", Pooling::Mean),
    ("cls", Pooling::Cls),
    ("max", Pooling::Max),
    ("mean_sqrt_len_tokens", Pooling::MeanSqrtLen),
    ("weightedmean", Pooling::WeightedMean),
    ("lasttoken", Pooling::LastToken),
];

/// Read the Pooling module's mode: the `pooling_mode` name newer packages
/// write, or the `pooling_mode_*` flags older ones do. When both are present
/// they must agree.
fn read_pooling(config: &Value) -> Result<Pooling> {
    let enabled: Vec<Pooling> = POOLING_MODES
        .iter()
        .filter(|(key, _)| config.get(*key).and_then(Value::as_bool) == Some(true))
        .map(|(_, mode)| *mode)
        .collect();
    let named = match config.get("pooling_mode") {
        None | Some(Value::Null) => None,
        Some(Value::String(name)) => Some(
            POOLING_MODE_NAMES
                .iter()
                .find(|(known, _)| known.eq_ignore_ascii_case(name.trim()))
                .map(|(_, mode)| *mode)
                .ok_or_else(|| {
                    unsupported(format!("the Pooling module names an unknown mode {name:?}"))
                })?,
        ),
        Some(_) => {
            return Err(unsupported(
                "the Pooling module's pooling_mode is not a name",
            ))
        }
    };
    match (named, enabled.as_slice()) {
        (Some(mode), []) => Ok(mode),
        (Some(mode), [flag]) if *flag == mode => Ok(mode),
        (Some(_), _) => Err(unsupported(
            "the Pooling module's pooling_mode and pooling_mode_* flags disagree",
        )),
        (None, [mode]) => Ok(*mode),
        (None, []) => Err(unsupported("the Pooling module enables no pooling mode")),
        (None, _) => Err(unsupported(
            "the Pooling module combines several pooling modes",
        )),
    }
}

fn read_logit_score(dir: &Path, config: &Value) -> Result<LogitScore> {
    let map = config
        .as_object()
        .filter(|map| {
            map.keys()
                .all(|key| key == "true_token_id" || key == "false_token_id")
        })
        .ok_or_else(|| {
            unsupported("LogitScore config must hold only true_token_id and false_token_id")
        })?;
    let id = |key: &str| -> Result<Option<u32>> {
        match map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .map(Some)
                .ok_or_else(|| unsupported(format!("{key} is not a token id"))),
        }
    };
    let true_token_id =
        id("true_token_id")?.ok_or_else(|| unsupported("LogitScore has no true_token_id"))?;
    let false_token_id = id("false_token_id")?;
    if false_token_id == Some(true_token_id) {
        return Err(unsupported("LogitScore true and false tokens are the same"));
    }
    let tokens = token_strings(
        &read_json(dir, "tokenizer.json")?,
        &[Some(true_token_id), false_token_id],
    )?;
    Ok(LogitScore {
        true_token_id,
        true_token: tokens[&true_token_id].clone(),
        false_token_id,
        false_token: false_token_id.map(|id| tokens[&id].clone()),
    })
}

fn token_strings(tokenizer: &Value, ids: &[Option<u32>]) -> Result<BTreeMap<u32, String>> {
    let wanted: Vec<u32> = ids.iter().flatten().copied().collect();
    let mut found = BTreeMap::new();
    for token in tokenizer
        .get("added_tokens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(id), Some(content)) = (
            token
                .get("id")
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok()),
            token.get("content").and_then(Value::as_str),
        ) {
            if wanted.contains(&id) {
                found.insert(id, content.to_string());
            }
        }
    }
    if let Some(vocab) = tokenizer
        .get("model")
        .and_then(|model| model.get("vocab"))
        .and_then(Value::as_object)
    {
        for (text, id) in vocab {
            if let Some(id) = id.as_u64().and_then(|id| u32::try_from(id).ok()) {
                if wanted.contains(&id) {
                    found.entry(id).or_insert_with(|| text.clone());
                }
            }
        }
    }
    match wanted.iter().find(|id| !found.contains_key(id)) {
        Some(missing) => Err(unsupported(format!(
            "tokenizer.json has no token {missing}"
        ))),
        None => Ok(found),
    }
}

fn read_json(dir: &Path, relative: &str) -> Result<Value> {
    let path = dir.join(relative);
    let bytes = std::fs::read(&path)
        .map_err(|error| PumasError::io("reading sentence-transformers file", &path, error))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| unsupported(format!("{relative} is not valid JSON: {error}")))
}

fn optional_json(dir: &Path, relative: &str) -> Result<Option<Value>> {
    if dir.join(relative).is_file() {
        read_json(dir, relative).map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn package(files: &[(&str, Value)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, value) in files {
            let target = dir.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            let text = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            std::fs::write(target, text).unwrap();
        }
        dir
    }

    fn reranker(score: Value) -> tempfile::TempDir {
        package(&[
            (
                "modules.json",
                json!([
                    {"idx":0,"path":"","type":"sentence_transformers.base.modules.transformer.Transformer"},
                    {"idx":1,"path":"1_LogitScore","type":LOGIT_SCORE}
                ]),
            ),
            ("1_LogitScore/config.json", score),
            (
                "config_sentence_transformers.json",
                json!({"model_type":"CrossEncoder","activation_fn":"torch.nn.modules.linear.Identity"}),
            ),
            (
                "sentence_bert_config.json",
                json!({"processing_kwargs":{"chat_template":{"add_generation_prompt":true}}}),
            ),
            (
                "chat_template.jinja",
                Value::String("{{ messages }}".into()),
            ),
            (
                "tokenizer_config.json",
                json!({"eos_token":"<|im_end|>","pad_token":"<|endoftext|>","model_max_length":32768}),
            ),
            (
                "tokenizer.json",
                json!({"added_tokens":[{"id":151645,"content":"<|im_end|>"}],"model":{"vocab":{"Yes":9454,"No":2753}}}),
            ),
        ])
    }

    #[test]
    fn a_logit_score_cross_encoder_records_its_tokens_template_and_options() {
        let dir = reranker(json!({"true_token_id":9454,"false_token_id":null}));
        let declared = read_package(dir.path(), DeclarationSource::Package).unwrap();
        assert_eq!(declared.model_kind, SentenceTransformersKind::CrossEncoder);
        assert_eq!(
            declared.logit_score,
            Some(LogitScore {
                true_token_id: 9454,
                true_token: "Yes".into(),
                false_token_id: None,
                false_token: None
            })
        );
        assert_eq!(declared.chat_template.as_deref(), Some("{{ messages }}"));
        assert_eq!(
            declared.chat_template_options.get("add_generation_prompt"),
            Some(&true)
        );
        assert_eq!(
            declared.special_tokens.get("eos_token").map(String::as_str),
            Some("<|im_end|>")
        );
        assert!(!declared.special_tokens.contains_key("model_max_length"));
        assert_eq!(
            declared.activation.as_deref(),
            Some("torch.nn.modules.linear.Identity")
        );
    }

    #[test]
    fn pooling_is_read_from_the_named_mode_newer_packages_write() {
        // EmbeddingGemma 2 and Qwen3-VL-Embedding write these.
        assert_eq!(
            read_pooling(
                &json!({"embedding_dimension":768,"pooling_mode":"mean","include_prompt":true})
            )
            .unwrap(),
            Pooling::Mean
        );
        assert_eq!(
            read_pooling(&json!({"embedding_dimension":2048,"pooling_mode":"lasttoken"})).unwrap(),
            Pooling::LastToken
        );
        // Both forms present must agree.
        assert_eq!(
            read_pooling(&json!({"pooling_mode":"cls","pooling_mode_cls_token":true})).unwrap(),
            Pooling::Cls
        );
        let refusal = |config| read_pooling(&config).unwrap_err().to_string();
        assert!(
            refusal(json!({"pooling_mode":"mean","pooling_mode_cls_token":true}))
                .contains("disagree")
        );
        assert!(refusal(json!({"pooling_mode":"attention"})).contains("unknown mode"));
        assert!(refusal(json!({"pooling_mode":3})).contains("not a name"));
        assert!(refusal(json!({})).contains("enables no pooling mode"));
    }

    #[test]
    fn a_package_missing_a_module_file_reports_the_file_as_missing() {
        // modules.json names 1_Pooling, but a partial download lacks its config.
        let dir = package(&[(
            "modules.json",
            json!([
                {"path":"","type":"sentence_transformers.models.Transformer"},
                {"path":"1_Pooling","type":"sentence_transformers.models.Pooling"}
            ]),
        )]);
        let error = read_package(dir.path(), DeclarationSource::Package).unwrap_err();
        assert!(
            matches!(&error, PumasError::Io { source: Some(source), .. }
                if source.kind() == std::io::ErrorKind::NotFound),
            "{error:?}"
        );
    }

    #[test]
    fn an_embedder_records_its_pooling_normalization_and_prompts() {
        let dir = package(&[
            (
                "modules.json",
                json!([
                    {"path":"","type":"sentence_transformers.models.Transformer"},
                    {"path":"1_Pooling","type":"sentence_transformers.models.Pooling"},
                    {"path":"2_Normalize","type":"sentence_transformers.models.Normalize"}
                ]),
            ),
            (
                "1_Pooling/config.json",
                json!({"pooling_mode_lasttoken":true,"pooling_mode_mean_tokens":false,"include_prompt":true}),
            ),
            (
                "config_sentence_transformers.json",
                json!({"prompts":{"query":"Instruct: x\nQuery: "},"default_prompt_name":null}),
            ),
            ("sentence_bert_config.json", json!({"max_seq_length":32768})),
        ]);
        let declared = read_package(dir.path(), DeclarationSource::Package).unwrap();
        assert_eq!(
            declared.model_kind,
            SentenceTransformersKind::SentenceTransformer
        );
        assert_eq!(declared.pooling, Some(Pooling::LastToken));
        assert!(declared.normalize);
        assert_eq!(
            declared.prompts.get("query").map(String::as_str),
            Some("Instruct: x\nQuery: ")
        );
        assert_eq!(declared.max_seq_length, Some(32768));
        assert_eq!(declared.modules.len(), 3);
    }

    fn reason(error: PumasError) -> String {
        match error {
            PumasError::Validation { message, .. } => message,
            other => panic!("expected a typed refusal, got {other:?}"),
        }
    }

    #[test]
    fn invalid_declarations_are_refused_with_their_reason() {
        let extra = reranker(json!({"true_token_id":9454,"false_token_id":null,"scale":2}));
        assert!(
            reason(read_package(extra.path(), DeclarationSource::Package).unwrap_err())
                .contains("only true_token_id")
        );
        let same = reranker(json!({"true_token_id":9454,"false_token_id":9454}));
        assert!(
            reason(read_package(same.path(), DeclarationSource::Package).unwrap_err())
                .contains("the same")
        );
        let unknown = reranker(json!({"true_token_id":1,"false_token_id":null}));
        assert!(
            reason(read_package(unknown.path(), DeclarationSource::Package).unwrap_err())
                .contains("no token 1")
        );
        let escape = package(&[(
            "modules.json",
            json!([{"path":"","type":TRANSFORMER[0]},{"path":"../x","type":POOLING[0]}]),
        )]);
        assert!(
            reason(read_package(escape.path(), DeclarationSource::Package).unwrap_err())
                .contains("package-relative")
        );
        let combined = package(&[
            (
                "modules.json",
                json!([{"path":"","type":TRANSFORMER[0]},{"path":"1_Pooling","type":POOLING[0]}]),
            ),
            (
                "1_Pooling/config.json",
                json!({"pooling_mode_cls_token":true,"pooling_mode_mean_tokens":true}),
            ),
        ]);
        assert!(
            reason(read_package(combined.path(), DeclarationSource::Package).unwrap_err())
                .contains("combines")
        );
    }

    #[test]
    fn the_record_round_trips_through_metadata_json() {
        let dir = reranker(json!({"true_token_id":9454,"false_token_id":2753}));
        let declared = read_package(
            dir.path(),
            DeclarationSource::Repository {
                repo_id: "acme/reranker".into(),
                revision: "0".repeat(40),
            },
        )
        .unwrap();
        let value = serde_json::to_value(&declared).unwrap();
        assert_eq!(
            value["source"],
            json!({"kind":"repository","repo_id":"acme/reranker","revision":"0".repeat(40)})
        );
        assert_eq!(
            serde_json::from_value::<SentenceTransformersDeclarations>(value).unwrap(),
            declared
        );
    }
}
