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
    let setup_snapshots = [
        ConversionSetupStatus::InProgress,
        ConversionSetupStatus::Completed,
        ConversionSetupStatus::Failed,
        ConversionSetupStatus::Cancelled,
    ]
    .into_iter()
    .map(|status| ConversionSetupSnapshot {
        operation_id: "2e038924-e0e3-4266-95ef-f7a02997b7b6".into(),
        status,
        error: (status == ConversionSetupStatus::Failed)
            .then(|| "private setup diagnostic /secret/path".into()),
    })
    .collect::<Vec<_>>();
    let setup_started = setup_snapshots
        .iter()
        .cloned()
        .map(ConversionSetupStartedOutcome::new)
        .collect::<Result<Vec<_>, _>>()?;
    let setup_status = setup_snapshots
        .into_iter()
        .map(|snapshot| ConversionSetupStatusOutcome::new(Some(snapshot)))
        .collect::<Result<Vec<_>, _>>()?;
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
    let backend_setup_request_probes: Vec<Value> = backend_setup_requests()
        .into_iter()
        .map(|(method, params, _)| {
            let accepted = parse_command(method, Some(&params)).is_ok();
            serde_json::json!({"method":method,"params":params,"accepted":accepted})
        })
        .collect();
    let hf_details = HfDownloadDetailsOutcome::found(
        "example/model",
        pumas_library::models::HfDownloadDetails {
            repo_id: "example/model".into(),
            total_size_bytes: Some(MAX_JS_SAFE_INTEGER),
            download_options: vec![
                pumas_library::models::DownloadOption {
                    quant: "Q4_K_M".into(),
                    size_bytes: Some(42),
                    file_group: None,
                },
                pumas_library::models::DownloadOption {
                    quant: "Original precision".into(),
                    size_bytes: None,
                    file_group: Some(pumas_library::models::FileGroup {
                        filenames: vec![
                            "nested/model-00001.safetensors".into(),
                            "nested/model-00002.safetensors".into(),
                        ],
                        shard_count: 2,
                        label: "nested/model.safetensors".into(),
                    }),
                },
            ],
        },
    )?;
    let hf_empty = HfDownloadDetailsOutcome::found(
        "example/empty",
        pumas_library::models::HfDownloadDetails {
            repo_id: "example/empty".into(),
            download_options: vec![],
            total_size_bytes: None,
        },
    )?;
    let hf_download_details_request_probes: Vec<Value> = hf_download_details_requests()
        .into_iter()
        .map(|(params, _)| {
            let parsed = parse_command("get_hf_download_details", Some(&params));
            let normalized = match &parsed {
                Ok(RpcCommand::GetHfDownloadDetails { repo_id, quants }) => serde_json::json!({"repo_id":repo_id,"quants":quants}),
                _ => Value::Null,
            };
            serde_json::json!({"method":"get_hf_download_details","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
        }).collect();
    let metadata = library_model_metadata_fixture();
    let notes_outcome = |success, notes: Option<&str>, error: Option<&str>| {
        UpdateModelNotesOutcome::new(
            "llm/Exact Model",
            pumas_library::models::UpdateModelNotesResponse {
                success,
                model_id: "llm/Exact Model".into(),
                notes: notes.map(str::to_owned),
                error: error.map(str::to_owned),
            },
        )
    };
    let update_model_notes_request_probes:Vec<Value> = update_model_notes_requests().into_iter().map(|(params,_)| {
        let parsed = parse_command("update_model_notes",Some(&params));
        let normalized = match &parsed {
            Ok(RpcCommand::UpdateModelNotes { model_id,notes }) => serde_json::json!({"model_id":model_id,"notes":notes}),
            _ => Value::Null,
        };
        serde_json::json!({"method":"update_model_notes","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
    }).collect();
    let update_inference_settings_request_probes:Vec<Value> = update_inference_settings_requests().into_iter().map(|(params,_)| {
        let parsed = parse_command("update_inference_settings",Some(&params));
        let normalized = match &parsed {
            Ok(RpcCommand::UpdateInferenceSettings { model_id,settings }) => serde_json::json!({"model_id":model_id,"settings":settings}),
            _ => Value::Null,
        };
        serde_json::json!({"method":"update_inference_settings","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
    }).collect();
    let mut metadata_gguf = library_model_metadata_fixture();
    metadata_gguf.embedded_metadata = Some(pumas_library::EmbeddedMetadataResponse {
        file_type: "gguf".into(),
        metadata: serde_json::json!({
            "general.name":"Exact GGUF",
            "custom":{"nested":[null,{"value":"λ"}]},
            "general.base_model.0.repo_url":{"not":"a URL"},
            "general.basename":"Exact base",
        }),
    });
    let metadata_empty = pumas_library::LibraryModelMetadataResponse {
        success: true,
        model_id: "llm/Exact Model".into(),
        stored_metadata: None,
        effective_metadata: None,
        embedded_metadata: None,
        primary_file: None,
        component_manifest: None,
    };
    let mut fixtures = serde_json::json!({
        "library_model_metadata":LibraryModelMetadataOutcome::new("llm/Exact Model",metadata)?,
        "update_inference_settings_request_probes":update_inference_settings_request_probes,
        "update_model_notes_request_probes":update_model_notes_request_probes,
        "library_model_metadata_gguf":LibraryModelMetadataOutcome::new("llm/Exact Model",metadata_gguf)?,
        "library_model_metadata_empty":LibraryModelMetadataOutcome::new("llm/Exact Model",metadata_empty)?,
        "hf_download_details_request_probes":hf_download_details_request_probes,
        "inference_settings":InferenceSettingsOutcome::new("llm/Exact Model".into(), inference_settings_fixture())?,
        "inference_settings_empty":InferenceSettingsOutcome::new("llm/empty".into(), vec![])?,
        "hf_download_details_success":hf_details,
        "hf_download_details_empty":hf_empty,
        "hf_download_details_failure":HfDownloadDetailsOutcome::failed(&PumasError::Other("private upstream detail".into())),
        "models":models, "search":search, "recovery_request":recovery_request,
        "link_health_healthy":link_health_healthy, "link_health_degraded":link_health_degraded,
        "conversion_progress":conversion_progress, "conversion_missing":conversion_missing,
        "conversion_list":conversion_list,
        "conversion_started":ConversionStartedOutcome::new("fixture-conversion".into())?,
        "conversion_cancelled":[ConversionCancelledOutcome::new(false),ConversionCancelledOutcome::new(true)],
        "conversion_environment":[ConversionEnvironmentOutcome::new(false),ConversionEnvironmentOutcome::new(true)],
        "conversion_setup_success":SuccessOutcome::new(),
        "conversion_setup_started":setup_started,
        "conversion_setup_status":setup_status,
        "conversion_setup_idle":ConversionSetupStatusOutcome::new(None)?,
        "backend_setup_request_probes":backend_setup_request_probes,
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
    });
    fixtures["update_inference_settings"] =
        serde_json::to_value(UpdateInferenceSettingsOutcome::new("llm/Exact Model"))?;
    for (key, status) in [
        (
            "github_cache_status_populated",
            pumas_library::models::CacheStatus {
                has_cache: true,
                is_valid: true,
                is_fetching: false,
                age_seconds: Some(MAX_JS_SAFE_INTEGER),
                last_fetched: Some(" 2026-09-08T00:00:00Z λ ".into()),
                releases_count: Some(u32::MAX),
            },
        ),
        (
            "github_cache_status_empty",
            pumas_library::models::CacheStatus {
                has_cache: false,
                is_valid: false,
                is_fetching: false,
                age_seconds: None,
                last_fetched: None,
                releases_count: None,
            },
        ),
        (
            "github_cache_status_fetching",
            pumas_library::models::CacheStatus {
                has_cache: true,
                is_valid: false,
                is_fetching: true,
                age_seconds: Some(42),
                last_fetched: Some("2026-09-08T00:00:00Z".into()),
                releases_count: Some(3),
            },
        ),
    ] {
        fixtures[key] = serde_json::to_value(GithubCacheStatusOutcome::snapshot(status)?)?;
    }
    fixtures["version_status"] =
        serde_json::to_value(VersionStatusOutcome::new(version_status_fixture())?)?;
    for key in ["version_status_empty", "version_status_no_manager"] {
        fixtures[key] =
            serde_json::to_value(VersionStatusOutcome::new(RuntimeVersionStatus::default())?)?;
    }
    fixtures["version_info_installed"] =
        serde_json::to_value(VersionInfoOutcome::new(" vλ.1 ".into(), true))?;
    fixtures["cancel_installation_true"] =
        serde_json::to_value(CancelInstallationOutcome::new(true))?;
    fixtures["cancel_installation_false"] =
        serde_json::to_value(CancelInstallationOutcome::new(false))?;
    fixtures["remove_version_true"] = serde_json::to_value(RemoveVersionOutcome::new(true))?;
    fixtures["remove_version_false"] = serde_json::to_value(RemoveVersionOutcome::new(false))?;
    fixtures["switch_version_true"] = serde_json::to_value(SwitchVersionOutcome::new(true))?;
    fixtures["switch_version_false"] = serde_json::to_value(SwitchVersionOutcome::new(false))?;
    fixtures["set_default_version_true"] =
        serde_json::to_value(SetDefaultVersionOutcome::new(true))?;
    fixtures["set_default_version_false"] =
        serde_json::to_value(SetDefaultVersionOutcome::new(false))?;
    fixtures["set_default_version_request_probes"] = set_default_version_requests().into_iter().map(|(params, _)| {
        let parsed = parse_params::<SetDefaultVersionParams>(Some(&params));
        let normalized = match &parsed {
            Ok(value) => serde_json::json!({"app_id":value.app_id,"tag":value.tag}),
            Err(_) => Value::Null,
        };
        serde_json::json!({"method":"set_default_version","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
    }).collect();
    for (key, status) in [
        (
            "check_version_dependencies_null",
            pumas_library::models::DependencyStatus::default(),
        ),
        (
            "check_version_dependencies_populated",
            pumas_library::models::DependencyStatus {
                installed: vec!["torch==2.0".into()],
                missing: vec!["numpy>=1".into()],
                requirements_file: Some("fixture/requirements.txt".into()),
            },
        ),
    ] {
        fixtures[key] = serde_json::to_value(CheckVersionDependenciesOutcome::new(status))?;
    }
    fixtures["check_version_dependencies_request_probes"] = install_version_requests().into_iter().map(|(params, _)| {
        let parsed = parse_params::<CheckVersionDependenciesParams>(Some(&params));
        let normalized = match &parsed {
            Ok(value) => serde_json::json!({"app_id":value.app_id,"tag":value.tag}),
            Err(_) => Value::Null,
        };
        serde_json::json!({"method":"check_version_dependencies","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
    }).collect();
    fixtures["get_release_dependencies_request_probes"] = install_version_requests().into_iter().map(|(params, _)| {
        let parsed = parse_params::<GetReleaseDependenciesParams>(Some(&params));
        let normalized = match &parsed {
            Ok(value) => serde_json::json!({"app_id":value.app_id,"tag":value.tag}),
            Err(_) => Value::Null,
        };
        serde_json::json!({"method":"get_release_dependencies","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
    }).collect();
    fixtures["install_version_dependencies_request_probes"] = install_version_requests().into_iter().map(|(params, _)| {
        let parsed = parse_params::<InstallVersionDependenciesParams>(Some(&params));
        let normalized = match &parsed {
            Ok(value) => serde_json::json!({"app_id":value.app_id,"tag":value.tag}),
            Err(_) => Value::Null,
        };
        serde_json::json!({"method":"install_version_dependencies","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
    }).collect();
    fixtures["switch_version_request_probes"] = install_version_requests().into_iter().map(|(params, _)| {
        let parsed = parse_params::<SwitchVersionParams>(Some(&params));
        let normalized = match &parsed {
            Ok(value) => serde_json::json!({"app_id":value.app_id,"tag":value.tag}),
            Err(_) => Value::Null,
        };
        serde_json::json!({"method":"switch_version","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
    }).collect();
    for (key, value) in runtime_launch_responses() {
        fixtures[key] = serde_json::to_value(RuntimeLaunchOutcome::from(value))?;
    }
    fixtures["runtime_launch_request_probes"] = runtime_launch_requests()
        .into_iter()
        .map(|(params, _)| {
            let accepted = parse_params::<RuntimeLaunchParams>(params.as_ref()).is_ok();
            serde_json::json!({"params":params,"accepted":accepted,"omitted":params.is_none()})
        })
        .collect();
    fixtures["runtime_stop_true"] = serde_json::to_value(RuntimeStopOutcome::new(true))?;
    fixtures["runtime_stop_false"] = serde_json::to_value(RuntimeStopOutcome::new(false))?;
    fixtures["install_version_dependencies_true"] =
        serde_json::to_value(InstallVersionDependenciesOutcome::new(true))?;
    fixtures["install_version_dependencies_false"] =
        serde_json::to_value(InstallVersionDependenciesOutcome::new(false))?;
    fixtures["get_release_dependencies_empty"] =
        serde_json::to_value(GetReleaseDependenciesOutcome::new(vec![]))?;
    fixtures["get_release_dependencies_populated"] =
        serde_json::to_value(GetReleaseDependenciesOutcome::new(vec![
            " torch λ ".into(),
            "numpy".into(),
            "numpy".into(),
            "".into(),
        ]))?;
    fixtures["install_version_started"] =
        serde_json::to_value(InstallVersionOutcome::started("fixture-version"))?;
    fixtures["install_version_failed"] =
        serde_json::to_value(InstallVersionOutcome::failed(&PumasError::Config {
            message: "private diagnostic".into(),
        }))?;
    fixtures["install_version_no_manager"] = serde_json::to_value(
        InstallVersionOutcome::missing_manager("unregistered-runtime"),
    )?;
    fixtures["install_version_request_probes"] = install_version_requests().into_iter().map(|(params, _)| {
        let parsed = parse_params::<InstallVersionParams>(Some(&params));
        let normalized = match &parsed {
            Ok(value) => serde_json::json!({"app_id":value.app_id,"tag":value.tag}),
            Err(_) => Value::Null,
        };
        serde_json::json!({"method":"install_version","params":params,"accepted":parsed.is_ok(),"normalized":normalized})
    }).collect();
    fixtures["validate_installations_populated"] =
        serde_json::to_value(validate_installations_fixture())?;
    fixtures["installation_progress_populated"] = serde_json::to_value(
        InstallationProgressOutcome::new(Some(installation_progress_fixture()))?,
    )?;
    for key in [
        "installation_progress_null",
        "installation_progress_no_manager",
    ] {
        fixtures[key] = serde_json::to_value(InstallationProgressOutcome::new(None)?)?;
    }
    for (key, success) in [
        ("installation_progress_success", true),
        ("installation_progress_failure", false),
    ] {
        let mut progress = installation_progress_fixture();
        progress.completed_at = Some("done".into());
        progress.success = Some(success);
        progress.error = (!success).then(|| "failed".into());
        fixtures[key] = serde_json::to_value(InstallationProgressOutcome::new(Some(progress))?)?;
    }
    for key in [
        "validate_installations_empty",
        "validate_installations_no_manager",
    ] {
        fixtures[key] =
            serde_json::to_value(ValidateInstallationsOutcome::new(vec![], vec![], 0)?)?;
    }
    fixtures["version_info_uninstalled"] =
        serde_json::to_value(VersionInfoOutcome::new("v2".into(), false))?;
    fixtures["installed_versions"] = serde_json::to_value(InstalledVersionsOutcome::new(vec![
        " vλ.1 ".into(),
        "v2".into(),
        "v2".into(),
        String::new(),
    ]))?;
    fixtures["selected_version"] =
        serde_json::to_value(SelectedVersionOutcome::new(Some(" vλ.1 ".into())))?;
    fixtures["selected_version_empty"] = serde_json::to_value(SelectedVersionOutcome::new(None))?;
    fixtures["installed_versions_empty"] =
        serde_json::to_value(InstalledVersionsOutcome::new(vec![]))?;
    fixtures["github_cache_status_no_manager"] =
        serde_json::to_value(GithubCacheStatusOutcome::no_manager())?;
    fixtures["available_versions"] = serde_json::to_value(AvailableVersionsOutcome::available(
        available_versions_fixture(),
    )?)?;
    fixtures["available_versions_empty"] =
        serde_json::to_value(AvailableVersionsOutcome::available(vec![])?)?;
    fixtures["available_versions_rate_limited"] = serde_json::to_value(
        AvailableVersionsOutcome::rate_limited(Some(MAX_JS_SAFE_INTEGER))?,
    )?;
    fixtures["available_versions_rate_limited_unknown"] =
        serde_json::to_value(AvailableVersionsOutcome::rate_limited(None)?)?;
    fixtures["update_model_notes_text"] = serde_json::to_value(notes_outcome(
        true,
        Some("  # Exact λ\n\n**notes**  "),
        None,
    )?)?;
    fixtures["update_model_notes_clear"] = serde_json::to_value(notes_outcome(true, None, None)?)?;
    fixtures["update_model_notes_missing"] = serde_json::to_value(notes_outcome(
        false,
        None,
        Some("Model not found: private model details"),
    )?)?;
    Ok(fixtures)
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
        HfDownloadDetailsOutcome,
        InferenceSettingsOutcome,
        UpdateInferenceSettingsOutcome,
        AvailableVersionsOutcome,
        GithubCacheStatusOutcome,
        InstalledVersionsOutcome,
        SelectedVersionOutcome,
        VersionStatusOutcome,
        VersionInfoOutcome,
        ValidateInstallationsOutcome,
        InstallationProgressOutcome,
        UpdateModelNotesOutcome,
        LibraryModelMetadataOutcome,
        GetHfDownloadDetailsParams,
        UpdateInferenceSettingsParams,
        UpdateModelNotesParams,
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
        ConversionSetupStartedOutcome,
        ConversionSetupStatusOutcome,
        StartConversionSetupParams,
        StartBackendSetupParams,
        GetBackendSetupParams,
        SupportedQuantTypesOutcome,
        BackendStatusOutcome,
        SuccessOutcome,
        CancelInstallationOutcome,
        RemoveVersionOutcome,
        SwitchVersionOutcome,
        SetDefaultVersionOutcome,
        SetDefaultVersionParams,
        InstallVersionParams,
        InstallVersionOutcome,
        RuntimeLaunchParams,
        RuntimeLaunchOutcome,
        RuntimeStopOutcome,
        SwitchVersionParams,
        InstallVersionDependenciesParams,
        InstallVersionDependenciesOutcome,
        GetReleaseDependenciesParams,
        GetReleaseDependenciesOutcome,
        CheckVersionDependenciesParams,
        CheckVersionDependenciesOutcome,
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
    if matches!(
        T::schema_name().as_ref(),
        "InferenceSettingsOutcome"
            | "LibraryModelMetadataOutcome"
            | "LibraryModelMetadataResponse"
            | "UpdateInferenceSettingsParams"
    ) {
        schema["definitions"]["DesktopJsonValue"] = serde_json::json!({
            "anyOf":[
                {"type":"null"}, {"type":"boolean"}, {"type":"string"},
                {"type":"number","minimum":-(MAX_JS_SAFE_INTEGER as i64),"maximum":MAX_JS_SAFE_INTEGER},
                {"type":"array","items":{"$ref":"#/definitions/DesktopJsonValue"}},
                {"type":"object","additionalProperties":{"$ref":"#/definitions/DesktopJsonValue"}},
            ]
        });
    }
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
    if matches!(
        name,
        "SetDefaultVersionParams"
            | "InstallVersionParams"
            | "CheckVersionDependenciesParams"
            | "GetReleaseDependenciesParams"
            | "InstallVersionDependenciesParams"
            | "SwitchVersionParams"
    ) {
        let mut canonical = schema.clone();
        let object = canonical.as_object_mut().expect("request object schema");
        object.remove("$schema");
        object.remove("title");
        let mut alias = canonical.clone();
        let property = alias["properties"]
            .as_object_mut()
            .unwrap()
            .remove("app_id")
            .unwrap();
        alias["properties"]["appId"] = property;
        for required in alias["required"].as_array_mut().unwrap() {
            if required == "app_id" {
                *required = "appId".into();
            }
        }
        *schema = serde_json::json!({"anyOf":[canonical, alias]});
        return;
    }
    if matches!(
        name,
        "UpdateInferenceSettingsParams" | "UpdateModelNotesParams"
    ) {
        let (canonical_field, field_aliases): (&str, &[&str]) = if name == "UpdateModelNotesParams"
        {
            ("notes", &["notes", "model_notes"])
        } else {
            (
                "settings",
                &["settings", "inference_settings", "inferenceSettings"],
            )
        };
        let mut canonical = schema.clone();
        let object = canonical.as_object_mut().expect("request object schema");
        let definitions = object.remove("definitions");
        object.remove("$schema");
        object.remove("title");
        let mut variants = Vec::new();
        for model_key in ["model_id", "modelId"] {
            for field_key in field_aliases {
                let mut branch = canonical.clone();
                for (from, to) in [("model_id", model_key), (canonical_field, *field_key)] {
                    let properties = branch["properties"]
                        .as_object_mut()
                        .expect("request properties");
                    let property = properties.remove(from).expect("canonical request field");
                    properties.insert(to.into(), property);
                    for required in branch["required"]
                        .as_array_mut()
                        .expect("required request fields")
                    {
                        if required == from {
                            *required = to.into();
                        }
                    }
                }
                variants.push(branch);
            }
        }
        *schema = serde_json::json!({"anyOf":variants});
        if let Some(definitions) = definitions {
            schema["definitions"] = definitions;
        }
        return;
    }
    if name == "GetHfDownloadDetailsParams" {
        // Serde accepts either spelling but rejects duplicate aliases. Both
        // closed branches project that existing parser policy without a keyword.
        let mut canonical = schema.clone();
        if let Some(object) = canonical.as_object_mut() {
            object.remove("$schema");
            object.remove("title");
        }
        let mut alias = canonical.clone();
        if let Some(properties) = alias.get_mut("properties").and_then(Value::as_object_mut) {
            if let Some(repo) = properties.remove("repo_id") {
                properties.insert("repoId".into(), repo);
            }
        }
        if let Some(required) = alias.get_mut("required").and_then(Value::as_array_mut) {
            for field in required {
                if field == "repo_id" {
                    *field = "repoId".into();
                }
            }
        }
        *schema = serde_json::json!({"anyOf":[canonical,alias]});
        return;
    }
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    if name == "GithubCacheStatusSnapshot" {
        object.insert(
            "required".into(),
            serde_json::json!([
                "has_cache",
                "is_valid",
                "is_fetching",
                "age_seconds",
                "last_fetched",
                "releases_count"
            ]),
        );
    }
    if name == "GithubCacheStatusNoManager" {
        for field in ["has_cache", "is_valid", "is_fetching"] {
            object["properties"][field]["const"] = false.into();
        }
    }
    if name == "VersionReleaseInfo" {
        object.insert(
            "required".into(),
            serde_json::json!([
                "tagName",
                "name",
                "publishedAt",
                "prerelease",
                "body",
                "htmlUrl",
                "assets",
                "totalSize",
                "archiveSize",
                "dependenciesSize",
                "installing"
            ]),
        );
    }
    if name == "RuntimeVersionStatus" {
        object.insert(
            "required".into(),
            serde_json::json!([
                "installedCount",
                "activeVersion",
                "defaultVersion",
                "versions"
            ]),
        );
    }
    if name == "AvailableVersionsRateLimited" {
        object.insert(
            "required".into(),
            serde_json::json!(["success", "error", "rate_limited", "retry_after_secs"]),
        );
        object["properties"]["rate_limited"]["const"] = true.into();
    }
    if name == "InferenceSettingInput" {
        object.insert(
            "required".into(),
            serde_json::json!(["key", "label", "param_type", "default"]),
        );
        object["properties"]["default"] =
            serde_json::json!({"$ref":"#/definitions/DesktopJsonValue"});
    }
    if name == "InferenceConstraintsInput" {
        object["properties"]["allowed_values"] = serde_json::json!({"anyOf":[
            {"type":"null"},{"type":"array","items":{"$ref":"#/definitions/DesktopJsonValue"}},
        ]});
    }
    if name == "InferenceParamSchema" {
        object.insert(
            "required".into(),
            serde_json::json!([
                "key",
                "label",
                "param_type",
                "default",
                "description",
                "constraints"
            ]),
        );
        if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
            properties.insert(
                "default".into(),
                serde_json::json!({"$ref":"#/definitions/DesktopJsonValue"}),
            );
        }
    }
    if name == "ParamConstraints" {
        object.insert(
            "required".into(),
            serde_json::json!(["min", "max", "allowed_values"]),
        );
        if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
            properties.insert("allowed_values".into(), serde_json::json!({"anyOf":[
                {"type":"null"}, {"type":"array","items":{"$ref":"#/definitions/DesktopJsonValue"}},
            ]}));
        }
    }
    if name == "HfDownloadDetails" {
        object.insert(
            "required".into(),
            serde_json::json!(["repoId", "downloadOptions", "totalSizeBytes"]),
        );
    }
    if matches!(
        name,
        "LibraryModelMetadataOutcome" | "LibraryModelMetadataResponse"
    ) {
        object.insert(
            "required".into(),
            serde_json::json!(["success", "model_id"]),
        );
        if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
            for field in ["stored_metadata", "effective_metadata"] {
                properties.insert(field.into(),serde_json::json!({"type":"object","additionalProperties":{"$ref":"#/definitions/DesktopJsonValue"}}));
            }
            properties.insert(
                "embedded_metadata".into(),
                serde_json::json!({"$ref":"#/definitions/EmbeddedMetadataResponse"}),
            );
            properties.insert("primary_file".into(), serde_json::json!({"type":"string"}));
            properties.insert("component_manifest".into(),serde_json::json!({"type":"array","items":{"$ref":"#/definitions/BundleComponentManifestEntry"}}));
        }
    }
    if name == "EmbeddedMetadataResponse" {
        object.insert(
            "required".into(),
            serde_json::json!(["file_type", "metadata"]),
        );
        if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
            properties.insert("metadata".into(),serde_json::json!({"type":"object","additionalProperties":{"$ref":"#/definitions/DesktopJsonValue"}}));
        }
    }
    if name == "BundleComponentManifestEntry" {
        object.insert(
            "required".into(),
            serde_json::json!([
                "name",
                "relative_path",
                "source_library",
                "class_name",
                "state"
            ]),
        );
    }
    if name == "DownloadOption" {
        object.insert("required".into(), serde_json::json!(["quant", "sizeBytes"]));
        // Serde omits None; the producer never emits a null fileGroup.
        if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
            properties.insert(
                "fileGroup".into(),
                serde_json::json!({"$ref":"#/definitions/FileGroup"}),
            );
        }
    }
    if name == "UpdateModelNotesSuccess" {
        object.insert(
            "required".into(),
            serde_json::json!(["success", "model_id"]),
        );
        object["properties"]["notes"] = serde_json::json!({"type":"string"});
    }
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
        "ConversionSetupSnapshotOutcome" => {
            object.insert("pumasConversionSetup".into(), true.into());
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
            | "ConversionSetupStartedOutcome"
            | "ConversionSetupStatusOutcome"
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
            "ConversionSetupSnapshotOutcome" => {
                properties["operationId"]["pattern"] =
                    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$".into();
                properties["operationId"]["minLength"] = 36.into();
                properties["operationId"]["maxLength"] = 36.into();
                properties["error"]["enum"] = serde_json::json!([null, SETUP_FAILURE_MESSAGE]);
            }
            "StartConversionSetupParams" | "StartBackendSetupParams" => {
                properties["expected_previous_operation_id"]["pattern"] =
                    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$".into();
                properties["expected_previous_operation_id"]["minLength"] = 36.into();
                properties["expected_previous_operation_id"]["maxLength"] = 36.into();
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
            "GetReleaseDependenciesOutcome"
            | "CheckVersionDependenciesOutcome"
            | "InstallVersionStarted"
            | "ModelsOutcome"
            | "CatalogSearchOutcome"
            | "DownloadListOutcome"
            | "DownloadStartedSuccess"
            | "DownloadStatusFoundOutcome"
            | "HfDownloadDetailsSuccess"
            | "InferenceSettingsOutcome"
            | "UpdateInferenceSettingsOutcome"
            | "AvailableVersionsSuccess"
            | "InstalledVersionsOutcome"
            | "SelectedVersionOutcome"
            | "VersionStatusOutcome"
            | "VersionInfoOutcome"
            | "UpdateModelNotesSuccess"
            | "LibraryModelMetadataOutcome"
            | "LibraryModelMetadataResponse"
            | "ModelIndexRefreshOutcome" => Some(true),
            "InstallVersionFailed"
            | "DownloadStartedFailure"
            | "DownloadStatusMissingOutcome"
            | "HfDownloadDetailsFailure"
            | "UpdateModelNotesFailure"
            | "AvailableVersionsRateLimited" => Some(false),
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
