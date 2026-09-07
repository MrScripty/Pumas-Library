//! Build-time projection of the selected desktop wire declarations.
//!
//! Serde DTOs own representation. Their constructors own domain authorization;
//! this export preserves the receiving process's representation constraints.
//! It does not issue recovery authority or make cached tickets current.

use super::*;
use schemars::{generate::SchemaSettings, JsonSchema};

/// Conformance values are made by the production constructors and real ticket
/// issuer. Temporary filesystem identity is deliberately not normalized.
pub(crate) fn desktop_contract_fixtures() -> anyhow::Result<Value> {
    let root = tempfile::TempDir::new()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (link_health_healthy, link_health_degraded) = runtime.block_on(async {
        use pumas_library::model_library::{LinkEntry, LinkRegistry, LinkType};
        let registry = LinkRegistry::new(root.path().join("link-health.json"));
        let target = root.path().join("linked-model");
        std::fs::write(&target, b"model")?;
        let entry = LinkEntry {
            model_id: "llm/example/model".into(),
            source: target.clone(),
            target,
            link_type: LinkType::Copy,
            created_at: "2026-09-06T00:00:00Z".into(),
            app_id: "embedded-consumer".into(),
            app_version: None,
        };
        registry.register(entry.clone()).await?;
        let healthy = LinkHealthOutcome::try_from(registry.health().await?)?;
        registry
            .register(LinkEntry {
                target: root.path().join("missing-link"),
                ..entry
            })
            .await?;
        let degraded = LinkHealthOutcome::try_from(registry.health().await?)?;
        Ok::<_, PumasError>((healthy, degraded))
    })?;
    let mut records = Vec::new();
    for (name, partial, duplicate) in [
        ("complete", false, false),
        ("partial", true, false),
        ("duplicate", false, true),
        ("duplicate-peer", false, true),
    ] {
        let id = format!("llm/example/{name}");
        let directory = root.path().join(&id);
        std::fs::create_dir_all(&directory)?;
        let mut metadata = serde_json::json!({
            "dependency_bindings": [], "primary_format": "gguf",
            "related_available": name == "complete",
            "size_bytes": 10, "download_incomplete": partial,
            "download_has_part_files": partial,
            "download_missing_expected_files": if partial {1} else {0},
            "download_progress": if partial {Some(0.5)} else {None},
        });
        if partial {
            std::fs::write(directory.join("weights.gguf.part"), b"12345")?;
            metadata["repo_id"] = "example/model".into();
            metadata["selected_artifact_id"] = "example/model::Q4".into();
            metadata["selected_artifact_files"] = serde_json::json!(["weights.gguf"]);
        }
        if duplicate {
            let peer = if name == "duplicate" {
                "duplicate-peer"
            } else {
                "duplicate"
            };
            metadata["integrity_issue_duplicate_repo_id"] = true.into();
            metadata["integrity_issue_duplicate_repo_id_count"] = 2.into();
            metadata["integrity_issue_duplicate_repo_id_others"] =
                serde_json::json!([format!("llm/example/{peer}")]);
        }
        records.push(ModelRecord {
            id,
            path: directory.display().to_string(),
            official_name: name.into(),
            cleaned_name: name.into(),
            model_type: "llm".into(),
            tags: Vec::new(),
            hashes: Default::default(),
            metadata,
            updated_at: "2026-09-05T00:00:00Z".into(),
        });
    }
    let mut catalog_text_probes = Vec::new();
    for input in [
        "   ",
        "\u{0085}",
        "\u{feff}",
        "\u{0085}Name\u{0085}",
        "\u{feff}Name\u{feff}",
        "Name",
        " Name ",
    ] {
        let mut record = records[0].clone();
        record.official_name = input.into();
        record.cleaned_name = input.into();
        let projected = ModelsOutcome::from_records(vec![record], root.path())?;
        catalog_text_probes.push(serde_json::json!({"input":input,"emitted":projected.models["llm/example/complete"].display_name}));
    }
    let models = ModelsOutcome::from_records(records.clone(), root.path())?;
    let search = CatalogSearchOutcome::from_search(
        pumas_library::index::SearchResult {
            total_count: records.len(),
            models: records,
            query_time_ms: 0.5,
            query: String::new(),
        },
        root.path(),
    )?;
    let CatalogArtifactState::Partial {
        recovery: Some(recovery),
        ..
    } = &models.models["llm/example/partial"].artifact
    else {
        anyhow::bail!("The actual producer did not issue the fixture recovery ticket");
    };
    let recovery_request = serde_json::json!({"modelId":"llm/example/partial", "recoveryToken":recovery.recovery_token});
    if !matches!(
        parse_command("resume_partial_download", Some(&recovery_request)),
        Ok(RpcCommand::ResumePartialDownload { .. })
    ) {
        anyhow::bail!("The actual producer rejected its fixture recovery request");
    }
    let recovery_outcome = PartialDownloadOutcome::try_from(PartialDownloadAction {
        action: "attach".into(),
        download_id: Some("fixture-download".into()),
        status: Some(DownloadStatus::Queued),
        reason_code: None,
        message: None,
    })?;
    let recovery_busy_outcome = PartialDownloadOutcome::try_from(PartialDownloadAction {
        action: "none".into(),
        download_id: None,
        status: None,
        reason_code: Some("download_root_busy".into()),
        message: Some("Download library root is busy".into()),
    })?;
    let recovery_request_probes = [
        "llm/example/model".to_string(),
        String::new(),
        "../escape".to_string(),
        "folder/CON.gguf".to_string(),
        "folder\\file".to_string(),
        format!("folder/{}", "é".repeat(127)),
        format!("folder/{}", "é".repeat(128)),
    ]
    .into_iter()
    .map(|model_id| {
        let request =
            serde_json::json!({"modelId":model_id,"recoveryToken":recovery.recovery_token});
        let accepted = matches!(
            parse_command("resume_partial_download", Some(&request)),
            Ok(RpcCommand::ResumePartialDownload { .. })
        );
        serde_json::json!({"request":request,"accepted":accepted})
    })
    .collect::<Vec<_>>();
    let progress = ModelDownloadProgress {
        download_id: "fixture-download".into(),
        library_model_id: Some("llm/example/partial".into()),
        repo_id: Some("example/model".into()),
        selected_artifact_id: Some("example/model::Q4".into()),
        model_name: Some("partial".into()),
        model_type: Some("llm".into()),
        status: DownloadStatus::Paused,
        progress: Some(0.5),
        downloaded_bytes: Some(5),
        total_bytes: Some(10),
        speed: None,
        eta_seconds: None,
        retry_attempt: None,
        retry_limit: None,
        retrying: None,
        next_retry_delay_seconds: None,
        error: None,
    };
    let download_push =
        project_download_notification(&pumas_library::models::ModelDownloadUpdateNotification {
            cursor: "download:1".into(),
            snapshot: pumas_library::models::ModelDownloadSnapshot {
                cursor: "download:1".into(),
                revision: 1,
                downloads: vec![progress.clone()],
            },
            stale_cursor: false,
            snapshot_required: true,
        })?;
    let mut conversions = Vec::new();
    for direction in [
        ConversionDirection::GgufToSafetensors,
        ConversionDirection::SafetensorsToGguf,
        ConversionDirection::SafetensorsToQuantizedGguf,
        ConversionDirection::GgufToQuantizedGguf,
        ConversionDirection::SafetensorsToNvfp4,
        ConversionDirection::SafetensorsToSherryQat,
    ] {
        for status in [
            ConversionStatus::SettingUp,
            ConversionStatus::Validating,
            ConversionStatus::Converting,
            ConversionStatus::Writing,
            ConversionStatus::Importing,
            ConversionStatus::Completed,
            ConversionStatus::Cancelled,
            ConversionStatus::Error,
            ConversionStatus::BuildingToolchain,
            ConversionStatus::GeneratingF16Gguf,
            ConversionStatus::ComputingImatrix,
            ConversionStatus::Quantizing,
            ConversionStatus::Calibrating,
            ConversionStatus::Training,
        ] {
            conversions.push(ConversionProgress {
                conversion_id: format!("fixture-conversion-{}", conversions.len()),
                source_model_id: "llm/example/complete".into(),
                direction,
                status,
                progress: Some(0.5),
                current_tensor: None,
                tensors_completed: Some(1),
                tensors_total: Some(2),
                bytes_written: Some(5),
                estimated_output_size: Some(10),
                target_quant: None,
                error: (status == ConversionStatus::Error)
                    .then(|| "private conversion diagnostic".into()),
                output_model_id: None,
                pipeline_step: None,
                pipeline_steps_total: None,
                pipeline_step_label: None,
            });
        }
    }
    let conversion_progress = conversions
        .iter()
        .cloned()
        .map(|progress| ConversionProgressResponse::new(Some(progress)))
        .collect::<Result<Vec<_>, _>>()?;
    let conversion_list = ConversionListOutcome::new(conversions)?;
    let conversion_missing = ConversionProgressResponse::new(None)?;
    use pumas_library::conversion::{
        LlamaCppBackend, Nvfp4Backend, QuantizationBackend, SherryBackend,
    };
    let backends: Vec<Box<dyn QuantizationBackend>> = vec![
        Box::new(LlamaCppBackend::new(root.path())),
        Box::new(Nvfp4Backend::new(root.path())),
        Box::new(SherryBackend::new(root.path())),
    ];
    let quant_options = backends[0].supported_quant_types();
    let conversion_quant_types = SupportedQuantTypesOutcome::new(quant_options.clone())?;
    let mut nullable_quant = quant_options[0].clone();
    nullable_quant.backend = None;
    let conversion_quant_types_nullable_backend =
        SupportedQuantTypesOutcome::new(vec![nullable_quant])?;
    let conversion_backend_status = BackendStatusOutcome::new(
        backends
            .iter()
            .map(|backend| BackendStatus {
                backend: backend.backend_id(),
                name: backend.name().into(),
                ready: backend.is_ready(),
            })
            .collect(),
    );
    Ok(serde_json::json!({
        "models":models, "search":search, "recovery_request":recovery_request,
        "link_health_healthy":link_health_healthy, "link_health_degraded":link_health_degraded,
        "conversion_progress":conversion_progress, "conversion_missing":conversion_missing,
        "conversion_list":conversion_list,
        "conversion_started":ConversionStartedOutcome::new("fixture-conversion".into())?,
        "conversion_cancelled":[ConversionCancelledOutcome::new(false),ConversionCancelledOutcome::new(true)],
        "conversion_environment":[ConversionEnvironmentOutcome::new(false),ConversionEnvironmentOutcome::new(true)],
        "conversion_setup_success":SuccessOutcome::new(),
        "conversion_quant_types":conversion_quant_types,
        "conversion_quant_types_nullable_backend":conversion_quant_types_nullable_backend,
        "conversion_backend_status":conversion_backend_status,
        "recovery_outcome":recovery_outcome,
        "recovery_busy_outcome":recovery_busy_outcome,
        "recovery_request_probes":recovery_request_probes,
        "catalog_text_probes":catalog_text_probes,
        "download_status":DownloadStatusOutcome::new(Some(progress.clone()))?,
        "download_push":download_push,
        "download_list":DownloadListOutcome::new(vec![progress])?,
        "download_started":DownloadStartedOutcome::started("fixture-download".into(),Some("example/model::Q4".into())),
        "download_mutation":DownloadMutationOutcome::completed(true),
    }))
}

pub(crate) fn desktop_contract_schema() -> Result<Value, serde_json::Error> {
    let mut schemas = Map::new();
    macro_rules! export {
        ($($ty:ty),+ $(,)?) => { $(
            schemas.insert(stringify!($ty).to_string(), schema::<$ty>()?);
        )+ };
    }
    export!(
        ModelsOutcome,
        CatalogSearchOutcome,
        SearchCatalogParams,
        DownloadListOutcome,
        DownloadStatusOutcome,
        DownloadStartedOutcome,
        DownloadMutationOutcome,
        PartialDownloadOutcome,
        ModelIndexRefreshOutcome,
        RecoverDownloadParams,
        DownloadIdParams,
        PublicError,
        LinkHealthOutcome,
        ConversionProgressResponse,
        ConversionListOutcome,
        ConversionStartedOutcome,
        ConversionCancelledOutcome,
        ConversionEnvironmentOutcome,
        SupportedQuantTypesOutcome,
        BackendStatusOutcome,
        SuccessOutcome,
    );
    Ok(serde_json::json!({
        "format": "pumas-desktop-contract-1",
        "dialect": "http://json-schema.org/draft-07/schema#",
        "schemas": schemas,
    }))
}

fn schema<T: JsonSchema>() -> Result<Value, serde_json::Error> {
    let settings = SchemaSettings::draft07();
    let settings = if T::schema_name().ends_with("Params") {
        settings.for_deserialize()
    } else {
        settings.for_serialize()
    };
    let mut schema = serde_json::to_value(settings.into_generator().into_root_schema_for::<T>())?;
    refine_named(&T::schema_name(), &mut schema);
    if let Some(definitions) = schema.get_mut("definitions").and_then(Value::as_object_mut) {
        for (name, definition) in definitions {
            refine_named(name, definition);
        }
    }
    constrain_representation(&mut schema);
    Ok(schema)
}

// These named wire refinements project existing constructor invariants, not
// authorization. The generator owns their executable TypeScript projection.
fn refine_named(name: &str, schema: &mut Value) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    match name {
        "LinkHealthOutcome" | "LinkHealthResponse" => {
            object.insert("pumasLinkHealth".into(), true.into());
        }
        "CatalogModel" => {
            object.insert("pumasCatalogRow".into(), true.into());
        }
        "PartialDownloadOutcome" => {
            object.insert("pumasPartialOutcome".into(), true.into());
        }
        "CatalogSearchOutcome" => {
            object.insert("pumasCatalogSearch".into(), true.into());
        }
        "DownloadMutationOutcome" => {
            object.insert("pumasMutation".into(), true.into());
        }
        "DownloadStartedSuccess" => {
            object.insert("pumasStarted".into(), true.into());
        }
        _ => {}
    }
    if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
        match name {
            "ConversionProgressResponse"
            | "ConversionListOutcome"
            | "ConversionStartedOutcome"
            | "ConversionCancelledOutcome"
            | "ConversionEnvironmentOutcome"
            | "SupportedQuantTypesOutcome"
            | "BackendStatusOutcome"
            | "SuccessOutcome" => {
                properties["success"]["const"] = true.into();
                if name == "ConversionStartedOutcome" {
                    properties["conversion_id"]["pumasUtf8Max"] = MAX_IDENTIFIER_BYTES.into();
                    properties["conversion_id"]["pattern"] = r"[^\t\n\v\f\r \u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]".into();
                }
            }
            "QuantOption" => {
                properties["bitsPerWeight"]["minimum"] = 0.into();
            }
            "ConversionProgressOutcome" => {
                properties["progress"]["minimum"] = 0.into();
                properties["progress"]["maximum"] = 1.into();
                properties["error"]["enum"] = serde_json::json!([
                    null,
                    "The model conversion did not complete successfully."
                ]);
            }
            "LinkHealthOutcome" | "LinkHealthResponse" => {
                properties["success"]["const"] = true.into();
                properties["error"]["type"] = "null".into();
                properties["status"]["enum"] = serde_json::json!(["healthy", "degraded"]);
                for field in ["total_links", "healthy_links"] {
                    properties[field]["minimum"] = 0.into();
                    properties[field]["maximum"] = MAX_JS_SAFE_INTEGER.into();
                }
            }
            "ModelsOutcome" => {
                properties["models"]["pumasCatalogMap"] = true.into();
            }
            "CatalogSearchOutcome" => {
                properties["query"]["pumasUtf8Max"] = MAX_IDENTIFIER_BYTES.into();
                properties["query_time_ms"]["minimum"] = 0.into();
            }
            "SearchCatalogParams" => {
                properties["query"]["pumasUtf8Max"] = MAX_IDENTIFIER_BYTES.into();
                properties["limit"]["minimum"] = 1.into();
                properties["limit"]["maximum"] = MAX_COLLECTION_ITEMS.into();
            }
            "CatalogModel" => {
                for field in [
                    "id",
                    "modelDir",
                    "displayName",
                    "modelType",
                    "format",
                    "quantization",
                    "displayDate",
                ] {
                    properties[field]["pumasUtf8Max"] = MAX_IDENTIFIER_BYTES.into();
                    properties[field]["minLength"] = 1.into();
                    properties[field]["pumasCanonicalText"] = true.into();
                }
                properties["dependencyCount"]["maximum"] = MAX_COLLECTION_ITEMS.into();
            }
            "CatalogRecoveryIdentity" => {
                properties["recoveryToken"]["pattern"] = "^v1:[0-9a-f]{64}$".into();
                // Mirrors the selected recovery wire representation checked by
                // download_recovery::{optional_text, validate_repo_id,
                // optional_file_set}; these checks do not issue authority.
                for field in ["repoId", "selectedArtifactId", "selectedArtifactQuant"] {
                    properties[field]["pumasCanonicalText"] = true.into();
                    properties[field]["pumasUtf8Max"] = MAX_IDENTIFIER_BYTES.into();
                }
                properties["repoId"]["maxLength"] = 96.into();
                properties["repoId"]["pattern"] = r"^(?!.*(?:--|\.\.))(?!.*\.[gG][iI][tT]$)[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?/[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?$".into();
                properties["selectedArtifactFiles"]["maxItems"] = MAX_COLLECTION_ITEMS.into();
                properties["selectedArtifactFiles"]["minItems"] = 1.into();
                properties["selectedArtifactFiles"]["uniqueItems"] = true.into();
                properties["selectedArtifactFiles"]["items"]["pumasPortablePath"] = true.into();
                properties["selectedArtifactFiles"]["items"]["pumasUtf8Max"] =
                    MAX_IDENTIFIER_BYTES.into();
            }
            "RecoverDownloadParams" => {
                properties["recoveryToken"]["pattern"] = "^v1:[0-9a-f]{64}$".into();
                properties["modelId"]["pumasUtf8Max"] = MAX_IDENTIFIER_BYTES.into();
                properties["modelId"]["pumasPortablePath"] = true.into();
            }
            "DownloadIdParams" => {
                properties["download_id"]["pumasUtf8Max"] = MAX_IDENTIFIER_BYTES.into();
                properties["download_id"]["minLength"] = 1.into();
            }
            "DownloadProgressOutcome" | "DownloadStatusFoundOutcome" => {
                properties["libraryModelId"]["pumasPortablePath"] = true.into();
                properties["libraryModelId"]["pumasUtf8Max"] = MAX_IDENTIFIER_BYTES.into();
                for field in ["progress", "speed", "etaSeconds", "nextRetryDelaySeconds"] {
                    properties[field]["minimum"] = 0.into();
                }
                properties["progress"]["maximum"] = 1.into();
            }
            _ => {}
        }
        let success = match name {
            "ModelsOutcome"
            | "CatalogSearchOutcome"
            | "DownloadListOutcome"
            | "DownloadStartedSuccess"
            | "DownloadStatusFoundOutcome"
            | "ModelIndexRefreshOutcome" => Some(true),
            "DownloadStartedFailure" | "DownloadStatusMissingOutcome" => Some(false),
            _ => None,
        };
        if let Some(success) = success {
            properties["success"]["const"] = success.into();
        }
    }
    if name == "CatalogArtifactState" {
        if let Some(variants) = object.get_mut("oneOf").and_then(Value::as_array_mut) {
            for variant in variants {
                if let Some(properties) =
                    variant.get_mut("properties").and_then(Value::as_object_mut)
                {
                    if let Some(reasons) = properties.get_mut("reasons") {
                        reasons["minItems"] = 1.into();
                        reasons["maxItems"] = 2.into();
                        reasons["uniqueItems"] = true.into();
                    }
                    if let Some(progress) = properties.get_mut("downloadProgressFraction") {
                        progress["minimum"] = 0.into();
                        progress["exclusiveMaximum"] = 1.into();
                    }
                }
            }
        }
    }
}

fn constrain_representation(value: &mut Value) {
    match value {
        Value::Object(object) => {
            // Preserve Rust storage domains as standard numeric constraints
            // before removing those recognized annotation-only formats.
            let format = object
                .get("format")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let bounds = match format.as_deref() {
                Some("uint") => Some((
                    0.0,
                    (usize::MAX as u128).min(MAX_JS_SAFE_INTEGER as u128) as f64,
                )),
                Some("uint32") => Some((0.0, f64::from(u32::MAX))),
                Some("uint64") => Some((0.0, MAX_JS_SAFE_INTEGER as f64)),
                Some("int32") => Some((f64::from(i32::MIN), f64::from(i32::MAX))),
                Some("float") => Some((-f64::from(f32::MAX), f64::from(f32::MAX))),
                Some("double") => Some((-f64::MAX, f64::MAX)),
                _ => None,
            };
            if let Some((minimum, maximum)) = bounds {
                tighten_bounds(object, minimum, maximum);
                object.remove("format");
            }
            // All selected wire integers must survive JavaScript exactly.
            if object.get("type") == Some(&Value::String("integer".into()))
                || object
                    .get("type")
                    .and_then(Value::as_array)
                    .is_some_and(|types| types.iter().any(|ty| ty == "integer"))
            {
                tighten_bounds(
                    object,
                    -(MAX_JS_SAFE_INTEGER as f64),
                    MAX_JS_SAFE_INTEGER as f64,
                );
            }
            // DTOs are closed. Maps explicitly declare additionalProperties.
            if object.contains_key("properties") && !object.contains_key("additionalProperties") {
                object.insert("additionalProperties".into(), false.into());
            }
            for child in object.values_mut() {
                constrain_representation(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                constrain_representation(child);
            }
        }
        _ => {}
    }
}

fn tighten_bounds(object: &mut Map<String, Value>, minimum: f64, maximum: f64) {
    let minimum = object
        .get("minimum")
        .and_then(Value::as_f64)
        .map_or(minimum, |bound| bound.max(minimum));
    let maximum = object
        .get("maximum")
        .and_then(Value::as_f64)
        .map_or(maximum, |bound| bound.min(maximum));
    object.insert("minimum".into(), minimum.into());
    object.insert("maximum".into(), maximum.into());
}
