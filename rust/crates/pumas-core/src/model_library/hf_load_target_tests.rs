// Synthetic package observations only: these tests never load model tensors.

use super::*;
use crate::models::PumasArtifactLoadPathKind;

const MODEL_ID: &str = "audio/cohere/synthetic-asr";
const ARTIFACT_ID: &str = "cohere--synthetic-asr__hf";
const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const TOKEN_PREFIX: &str = "pumas-package-observation-v1:sha256:";

async fn synthetic_cohere_library() -> (TempDir, ModelLibrary, PathBuf) {
    let (temp, library) = setup_library().await;
    let model_dir = seed_synthetic_cohere(&library).await;
    (temp, library, model_dir)
}

async fn seed_synthetic_cohere(library: &ModelLibrary) -> PathBuf {
    let model_dir = library.build_model_path("audio", "cohere", "synthetic-asr");
    std::fs::create_dir_all(&model_dir).unwrap();
    write_min_safetensors(&model_dir.join("model.safetensors"));
    for (name, contents) in [
        (
            "config.json",
            r#"{"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"]}"#,
        ),
        (
            "tokenizer.json",
            r#"{"version":"1.0","model":{"type":"BPE","vocab":{}}}"#,
        ),
        (
            "tokenizer_config.json",
            r#"{"tokenizer_class":"CohereAsrTokenizer"}"#,
        ),
        (
            "preprocessor_config.json",
            r#"{"feature_extractor_type":"CohereAsrFeatureExtractor","sampling_rate":16000}"#,
        ),
    ] {
        std::fs::write(model_dir.join(name), contents).unwrap();
    }
    let metadata = ModelMetadata {
        schema_version: Some(2),
        model_id: Some(MODEL_ID.into()),
        family: Some("cohere".into()),
        model_type: Some("audio".into()),
        official_name: Some("Synthetic Cohere ASR".into()),
        cleaned_name: Some("synthetic-asr".into()),
        repo_id: Some("cohere/synthetic-asr".into()),
        upstream_revision: Some(REVISION.into()),
        selected_artifact_id: Some(ARTIFACT_ID.into()),
        selected_artifact_files: Some(vec![
            "model.safetensors".into(),
            "config.json".into(),
            "tokenizer.json".into(),
            "tokenizer_config.json".into(),
            "preprocessor_config.json".into(),
        ]),
        storage_kind: Some(StorageKind::LibraryOwned),
        validation_state: Some(AssetValidationState::Valid),
        task_type_primary: Some("automatic-speech-recognition".into()),
        input_modalities: Some(vec!["audio".into()]),
        output_modalities: Some(vec!["text".into()]),
        recommended_backend: Some("transformers".into()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();
    library.index_model_dir(&model_dir).await.unwrap();
    model_dir
}

fn hf_request(
    mode: PumasArtifactLoadTargetResolutionMode,
) -> ResolveModelArtifactLoadTargetRequest {
    ResolveModelArtifactLoadTargetRequest {
        model_ref: PumasModelRef {
            model_id: MODEL_ID.into(),
            selected_artifact_id: Some(ARTIFACT_ID.into()),
            revision: Some(REVISION.into()),
            ..Default::default()
        },
        expected_artifact_kind: Some(PackageArtifactKind::HfCompatibleDirectory),
        caller_observed_entry_path: None,
        caller_observed_package_facts_contract_version: Some(PACKAGE_FACTS_CONTRACT_VERSION),
        resolution_mode: mode,
        consumer: PumasArtifactConsumer {
            consumer_name: "pantograph-synthetic-asr-test".into(),
            task_kind: Some("audio_transcription".into()),
            runtime_family: Some("pytorch.transformers".into()),
        },
    }
}

async fn ready_target(
    library: &ModelLibrary,
    mode: PumasArtifactLoadTargetResolutionMode,
) -> crate::models::PumasArtifactLoadTarget {
    let response = library
        .resolve_model_artifact_load_target(hf_request(mode))
        .await
        .unwrap();
    assert!(response.is_ready(), "{response:?}");
    response.target.unwrap()
}

fn cached(
    library: &ModelLibrary,
    scope: ModelPackageFactsCacheScope,
) -> ModelPackageFactsCacheRecord {
    library
        .index
        .get_model_package_facts_cache(MODEL_ID, Some(ARTIFACT_ID), scope)
        .unwrap()
        .unwrap()
}

fn remove_summary(library: &ModelLibrary) {
    rusqlite::Connection::open(library.index.db_path()).unwrap().execute(
        "DELETE FROM model_package_facts_cache WHERE model_id = ?1 AND selected_artifact_id = ?2 AND cache_scope = 'summary'",
        rusqlite::params![MODEL_ID, ARTIFACT_ID],
    ).unwrap();
}

async fn assert_read_only_alias_snapshot(
    library: &ModelLibrary,
    model_dir: &Path,
    alias_root: &Path,
    scope: ModelPackageFactsCacheScope,
) {
    let owner_target =
        ready_target(library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    if scope == ModelPackageFactsCacheScope::Detail {
        remove_summary(library);
    }
    let before_summary = library
        .index
        .list_model_package_facts_cache(MODEL_ID, ModelPackageFactsCacheScope::Summary)
        .unwrap();
    let before_detail = library
        .index
        .list_model_package_facts_cache(MODEL_ID, ModelPackageFactsCacheScope::Detail)
        .unwrap();
    let before_model = serde_json::to_value(library.index.get(MODEL_ID).unwrap()).unwrap();
    let before_effective = library.index.get_effective_metadata_json(MODEL_ID).unwrap();
    let before_cursor = library.index.current_model_library_update_cursor().unwrap();
    // Keep models.db and the canonical library root, but remove the entire
    // observed package. Indexed resolution must not canonicalize or scan it.
    std::fs::remove_dir_all(model_dir).unwrap();
    let read_only = crate::model_library::PumasReadOnlyLibrary::open(alias_root).unwrap();
    assert_eq!(read_only.library_root(), library.library_root());
    let response = read_only
        .resolve_model_artifact_load_target(hf_request(
            PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
        ))
        .unwrap();
    assert!(
        response.is_ready(),
        "{scope:?} root alias lost the indexed target: {response:?}"
    );
    let target = response.target.unwrap();
    assert_eq!(
        target, owner_target,
        "root alias changed canonical path, reference or observation token"
    );
    assert_eq!(
        target.local_load_path,
        MODEL_ID
            .split('/')
            .fold(library.library_display_root.clone(), |root, part| {
                root.join(part)
            })
            .display()
            .to_string()
    );
    assert_eq!(
        target.model_ref.selected_artifact_path.as_deref(),
        Some(target.local_load_path.as_str())
    );
    assert_eq!(
        library
            .index
            .list_model_package_facts_cache(MODEL_ID, ModelPackageFactsCacheScope::Summary)
            .unwrap(),
        before_summary
    );
    assert_eq!(
        library
            .index
            .list_model_package_facts_cache(MODEL_ID, ModelPackageFactsCacheScope::Detail)
            .unwrap(),
        before_detail
    );
    assert_eq!(
        serde_json::to_value(library.index.get(MODEL_ID).unwrap()).unwrap(),
        before_model
    );
    assert_eq!(
        library.index.get_effective_metadata_json(MODEL_ID).unwrap(),
        before_effective
    );
    assert_eq!(
        library.index.current_model_library_update_cursor().unwrap(),
        before_cursor
    );
}

#[tokio::test]
async fn hf_read_only_relative_root_preserves_summary_and_detail_snapshot() {
    let cwd = std::env::current_dir().unwrap();
    for scope in [
        ModelPackageFactsCacheScope::Summary,
        ModelPackageFactsCacheScope::Detail,
    ] {
        let (temp, library) = setup_library_relative_to_cwd().await;
        let model_dir = seed_synthetic_cohere(&library).await;
        let relative_root = temp.path().strip_prefix(&cwd).unwrap();
        assert!(!relative_root.is_absolute());
        assert_read_only_alias_snapshot(&library, &model_dir, relative_root, scope).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn hf_read_only_symlink_root_preserves_summary_and_detail_snapshot() {
    for scope in [
        ModelPackageFactsCacheScope::Summary,
        ModelPackageFactsCacheScope::Detail,
    ] {
        let temp = TempDir::new().unwrap();
        let library = ModelLibrary::new(temp.path().join("library"))
            .await
            .unwrap();
        let model_dir = seed_synthetic_cohere(&library).await;
        let alias_root = temp.path().join("library-alias");
        std::os::unix::fs::symlink(library.library_root(), &alias_root).unwrap();
        assert_read_only_alias_snapshot(&library, &model_dir, &alias_root, scope).await;
    }
}

#[cfg(windows)]
#[tokio::test]
async fn hf_windows_native_owner_target_round_trips_canonical_display_path() {
    let (_temp, library, model_dir) = synthetic_cohere_library().await;
    let canonical_package = model_dir.canonicalize().unwrap();
    let displayed_package = crate::platform::platform_display_path(&canonical_package);
    assert_ne!(
        Path::new(&displayed_package),
        canonical_package.as_path(),
        "fixture must exercise verbatim canonical path versus display path"
    );
    let target = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_eq!(target.local_load_path, displayed_package);
    assert_eq!(
        target.model_ref.selected_artifact_path.as_deref(),
        Some(displayed_package.as_str())
    );
    assert_eq!(target.load_path_kind, PumasArtifactLoadPathKind::Directory);
    assert_eq!(
        target.content_fingerprint.as_deref(),
        Some(
            cached(&library, ModelPackageFactsCacheScope::Detail)
                .source_fingerprint
                .as_str()
        )
    );
}

#[tokio::test]
async fn hf_owner_target_projects_canonical_package_identity_revision_and_observation() {
    let (_temp, library, model_dir) = synthetic_cohere_library().await;
    let target = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let root = crate::platform::platform_display_path(&std::fs::canonicalize(&model_dir).unwrap());
    assert_eq!(target.local_load_path, root);
    assert_eq!(target.load_path_kind, PumasArtifactLoadPathKind::Directory);
    assert_eq!(target.storage_kind, StorageKind::LibraryOwned);
    assert_eq!(target.model_ref.model_id, MODEL_ID);
    assert_eq!(
        target.model_ref.selected_artifact_id.as_deref(),
        Some(ARTIFACT_ID)
    );
    assert_eq!(
        target.model_ref.selected_artifact_path.as_deref(),
        Some(root.as_str())
    );
    assert_eq!(target.model_ref.revision.as_deref(), Some(REVISION));
    assert_eq!(
        target.package_facts_contract_version,
        Some(PACKAGE_FACTS_CONTRACT_VERSION)
    );
    let detail = cached(&library, ModelPackageFactsCacheScope::Detail);
    assert!(detail.source_fingerprint.starts_with(TOKEN_PREFIX));
    assert_eq!(detail.source_fingerprint.len(), TOKEN_PREFIX.len() + 64);
    assert_eq!(
        target.content_fingerprint.as_deref(),
        Some(detail.source_fingerprint.as_str())
    );
    let facts: ResolvedModelPackageFacts = serde_json::from_str(&detail.facts_json).unwrap();
    assert_eq!(facts.artifact.entry_path, root);
    assert_eq!(
        facts.transformers.unwrap().source_revision.as_deref(),
        Some(REVISION)
    );
    let descriptor = library
        .resolve_model_execution_descriptor(MODEL_ID)
        .await
        .unwrap();
    assert_eq!(descriptor.entry_path, target.local_load_path);
}

#[tokio::test]
async fn hf_summary_and_detail_targets_have_identical_observation_tokens() {
    let (_temp, library, _model_dir) = synthetic_cohere_library().await;
    let summary_target =
        ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let summary_row = cached(&library, ModelPackageFactsCacheScope::Summary);
    let detail_row = cached(&library, ModelPackageFactsCacheScope::Detail);
    assert_eq!(
        summary_row.source_fingerprint,
        detail_row.source_fingerprint
    );
    remove_summary(&library);
    let detail_target = ready_target(
        &library,
        PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
    )
    .await;
    assert_eq!(summary_target, detail_target);
    assert!(library
        .index
        .get_model_package_facts_cache(
            MODEL_ID,
            Some(ARTIFACT_ID),
            ModelPackageFactsCacheScope::Summary
        )
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn hf_expected_owner_observation_is_enforced_for_summary_and_detail() {
    let (_temp, library, _model_dir) = synthetic_cohere_library().await;
    let target = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let current_token = target.content_fingerprint.unwrap();
    let mismatched_token = format!("{TOKEN_PREFIX}{}", "f".repeat(64));
    for scope in [
        ModelPackageFactsCacheScope::Summary,
        ModelPackageFactsCacheScope::Detail,
    ] {
        if scope == ModelPackageFactsCacheScope::Detail {
            remove_summary(&library);
        }
        let row_before = cached(&library, scope);
        let resolve = |expected| {
            resolve_artifact_load_target_from_index(
                &library.index,
                library.library_root(),
                &library.library_display_root,
                hf_request(PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed),
                expected,
            )
            .unwrap()
        };
        assert!(resolve(Some(current_token.as_str())).is_ready());
        let refused = resolve(Some(mismatched_token.as_str()));
        assert!(
            !refused.is_ready(),
            "{scope:?} ignored owner observation mismatch"
        );
        assert_eq!(
            refused.diagnostics[0].code,
            PumasArtifactLoadTargetDiagnosticCode::StalePackageFacts
        );
        assert!(
            resolve(None).is_ready(),
            "indexed snapshot must not perform an implicit observation"
        );
        assert_eq!(cached(&library, scope), row_before);
    }
}

#[tokio::test]
async fn hf_indexed_resolution_is_an_immutable_snapshot_after_package_and_metadata_drift() {
    let (_temp, library, model_dir) = synthetic_cohere_library().await;
    let original = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    std::fs::write(
        model_dir.join("config.json"),
        r#"{"model_type":"changed","new_field":true}"#,
    )
    .unwrap();
    library
        .index
        .apply_metadata_overlay(
            MODEL_ID,
            "test-drift",
            &serde_json::json!({"recommended_backend":"changed-backend"}),
            "test",
            None,
        )
        .unwrap();
    let detail = cached(&library, ModelPackageFactsCacheScope::Detail);
    let summary = cached(&library, ModelPackageFactsCacheScope::Summary);
    let cursor = library.index.current_model_library_update_cursor().unwrap();
    let metadata = std::fs::read(model_dir.join("metadata.json")).unwrap();
    let indexed = ready_target(
        &library,
        PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
    )
    .await;
    assert_eq!(indexed, original);
    assert_eq!(
        cached(&library, ModelPackageFactsCacheScope::Detail),
        detail
    );
    assert_eq!(
        cached(&library, ModelPackageFactsCacheScope::Summary),
        summary
    );
    assert_eq!(
        library.index.current_model_library_update_cursor().unwrap(),
        cursor
    );
    assert_eq!(
        std::fs::read(model_dir.join("metadata.json")).unwrap(),
        metadata
    );
    let refreshed = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_ne!(refreshed.content_fingerprint, original.content_fingerprint);
}

#[tokio::test]
async fn hf_owner_reobserves_metadata_file_mtime_and_upstream_revision() {
    let (_temp, library, model_dir) = synthetic_cohere_library().await;
    let first = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    library
        .index
        .apply_metadata_overlay(
            MODEL_ID,
            "test-backend",
            &serde_json::json!({"recommended_backend":"different-backend"}),
            "test",
            None,
        )
        .unwrap();
    let metadata_changed =
        ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_ne!(
        first.content_fingerprint,
        metadata_changed.content_fingerprint
    );
    let weight = std::fs::File::options()
        .write(true)
        .open(model_dir.join("model.safetensors"))
        .unwrap();
    let modified =
        weight.metadata().unwrap().modified().unwrap() + std::time::Duration::from_secs(10);
    weight
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let mtime_changed =
        ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_ne!(
        metadata_changed.content_fingerprint,
        mtime_changed.content_fingerprint
    );
    let new_revision = "89abcdef0123456789abcdef0123456789abcdef01";
    library
        .index
        .apply_metadata_overlay(
            MODEL_ID,
            "test-revision",
            &serde_json::json!({"upstream_revision":new_revision}),
            "test",
            None,
        )
        .unwrap();
    let mut request = hf_request(PumasArtifactLoadTargetResolutionMode::OwnerFresh);
    request.model_ref.revision = Some(new_revision.into());
    let response = library
        .resolve_model_artifact_load_target(request)
        .await
        .unwrap();
    assert!(response.is_ready(), "{response:?}");
    let target = response.target.unwrap();
    assert_eq!(target.model_ref.revision.as_deref(), Some(new_revision));
    assert_ne!(
        target.content_fingerprint,
        mtime_changed.content_fingerprint
    );
    let mismatch = library
        .resolve_model_artifact_load_target(hf_request(
            PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
        ))
        .await
        .unwrap();
    assert!(
        !mismatch.is_ready(),
        "old pinned revision must not authorize a new revision"
    );
}

#[tokio::test]
async fn hf_owner_reobserves_cache_source_fingerprint_mismatch() {
    let (_temp, library, _model_dir) = synthetic_cohere_library().await;
    let original = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    for scope in [
        ModelPackageFactsCacheScope::Summary,
        ModelPackageFactsCacheScope::Detail,
    ] {
        let mut row = cached(&library, scope);
        row.source_fingerprint = format!("{TOKEN_PREFIX}{}", "f".repeat(64));
        library
            .index
            .upsert_model_package_facts_cache(&row)
            .unwrap();
    }
    let refreshed = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_eq!(refreshed.content_fingerprint, original.content_fingerprint);
    assert_ne!(
        cached(&library, ModelPackageFactsCacheScope::Detail).source_fingerprint,
        format!("{TOKEN_PREFIX}{}", "f".repeat(64))
    );
}

#[tokio::test]
async fn hf_currentness_refuses_dropped_revision_with_matching_observation() {
    let (_temp, library, _model_dir) = synthetic_cohere_library().await;
    ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let mut row = cached(&library, ModelPackageFactsCacheScope::Detail);
    assert!(library
        .cached_model_package_facts_are_current(&row, None, None)
        .await
        .unwrap());
    let mut facts: ResolvedModelPackageFacts = serde_json::from_str(&row.facts_json).unwrap();
    facts.model_ref.revision = None;
    row.facts_json = serde_json::to_string(&facts).unwrap();
    assert!(!library
        .cached_model_package_facts_are_current(&row, None, None)
        .await
        .unwrap());
    library
        .index
        .upsert_model_package_facts_cache(&row)
        .unwrap();
    let target = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_eq!(target.model_ref.revision.as_deref(), Some(REVISION));
}

#[tokio::test]
async fn hf_owner_reobserves_incoherent_cached_source_and_revision_evidence() {
    let (_temp, library, _model_dir) = synthetic_cohere_library().await;
    ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    for changed_field in ["source_repo_id", "source_revision"] {
        let mut row = cached(&library, ModelPackageFactsCacheScope::Detail);
        let mut facts: ResolvedModelPackageFacts = serde_json::from_str(&row.facts_json).unwrap();
        let evidence = facts.transformers.as_mut().unwrap();
        if changed_field == "source_repo_id" {
            evidence.source_repo_id = Some("cohere/unrelated-source".into());
        } else {
            evidence.source_revision = Some("unrelated-revision".into());
        }
        row.facts_json = serde_json::to_string(&facts).unwrap();
        library
            .index
            .upsert_model_package_facts_cache(&row)
            .unwrap();
        remove_summary(&library);
        let refused = library
            .resolve_model_artifact_load_target(hf_request(
                PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
            ))
            .await
            .unwrap();
        assert!(
            !refused.is_ready(),
            "indexed detail ignored mismatched {changed_field}"
        );
        assert_eq!(cached(&library, ModelPackageFactsCacheScope::Detail), row);
        ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
        let repaired: ResolvedModelPackageFacts =
            serde_json::from_str(&cached(&library, ModelPackageFactsCacheScope::Detail).facts_json)
                .unwrap();
        let evidence = repaired.transformers.unwrap();
        assert_eq!(
            evidence.source_repo_id.as_deref(),
            Some("cohere/synthetic-asr")
        );
        assert_eq!(evidence.source_revision.as_deref(), Some(REVISION));
    }
}

#[tokio::test]
async fn hf_observation_token_does_not_claim_a_full_package_content_digest() {
    let (_temp, library, model_dir) = synthetic_cohere_library().await;
    let original = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let weight_path = model_dir.join("model.safetensors");
    let before = std::fs::read(&weight_path).unwrap();
    let modified_at = std::fs::metadata(&weight_path).unwrap().modified().unwrap();
    let mut changed = before.clone();
    *changed.last_mut().unwrap() = 1;
    std::fs::write(&weight_path, &changed).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&weight_path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified_at))
        .unwrap();
    assert_ne!(std::fs::read(&weight_path).unwrap(), before);
    // This is deliberately the manifest metadata observation contract: equal
    // lengths and restored mtimes cannot attest whether all file bytes agree.
    let same_observation =
        ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_eq!(
        same_observation.content_fingerprint,
        original.content_fingerprint
    );
}

#[tokio::test]
async fn hf_owner_refuses_a_package_with_missing_selected_weight() {
    let (_temp, library, model_dir) = synthetic_cohere_library().await;
    ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    std::fs::remove_file(model_dir.join("model.safetensors")).unwrap();
    let response = library
        .resolve_model_artifact_load_target(hf_request(
            PumasArtifactLoadTargetResolutionMode::OwnerFresh,
        ))
        .await
        .unwrap();
    assert!(
        !response.is_ready(),
        "missing selected weight authorized a load target: {response:?}"
    );
}

#[tokio::test]
async fn hf_owner_reobserves_wrong_kind_cache_instead_of_reusing_valid_token() {
    let (_temp, library, _model_dir) = synthetic_cohere_library().await;
    let original = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let mut row = cached(&library, ModelPackageFactsCacheScope::Detail);
    let mut facts: ResolvedModelPackageFacts = serde_json::from_str(&row.facts_json).unwrap();
    facts.artifact.artifact_kind = PackageArtifactKind::Safetensors;
    row.facts_json = serde_json::to_string(&facts).unwrap();
    library
        .index
        .upsert_model_package_facts_cache(&row)
        .unwrap();
    let repaired = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_eq!(repaired, original);
}

#[tokio::test]
async fn hf_indexed_refuses_storage_relabeling_in_summary_and_detail() {
    let (temp, library, _model_dir) = synthetic_cohere_library().await;
    let original = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let outside_path = temp
        .path()
        .parent()
        .unwrap()
        .join("pumas-synthetic-hf-unowned-package")
        .display()
        .to_string();
    for scope in [
        ModelPackageFactsCacheScope::Summary,
        ModelPackageFactsCacheScope::Detail,
    ] {
        let mut row = cached(&library, scope);
        if scope == ModelPackageFactsCacheScope::Summary {
            let mut facts: ResolvedModelPackageFactsSummary =
                serde_json::from_str(&row.facts_json).unwrap();
            facts.storage_kind = StorageKind::ExternalReference;
            facts.entry_path = outside_path.clone();
            facts.model_ref.selected_artifact_path = Some(outside_path.clone());
            row.facts_json = serde_json::to_string(&facts).unwrap();
        } else {
            remove_summary(&library);
            let mut facts: ResolvedModelPackageFacts =
                serde_json::from_str(&row.facts_json).unwrap();
            facts.artifact.storage_kind = StorageKind::ExternalReference;
            facts.artifact.entry_path = outside_path.clone();
            facts.model_ref.selected_artifact_path = Some(outside_path.clone());
            row.facts_json = serde_json::to_string(&facts).unwrap();
        }
        library
            .index
            .upsert_model_package_facts_cache(&row)
            .unwrap();
        let refused = library
            .resolve_model_artifact_load_target(hf_request(
                PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
            ))
            .await
            .unwrap();
        assert!(
            !refused.is_ready(),
            "{scope:?} relabelled a managed package as an external reference"
        );
        assert_eq!(cached(&library, scope), row);
        let repaired =
            ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
        assert_eq!(repaired, original);
    }
}

#[tokio::test]
async fn hf_indexed_refuses_unqualified_external_weight_entry_in_summary_and_detail() {
    for scope in [
        ModelPackageFactsCacheScope::Summary,
        ModelPackageFactsCacheScope::Detail,
    ] {
        let (_temp, library, model_dir) = synthetic_cohere_library().await;
        ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
        let weight_path = model_dir.join("model.safetensors").display().to_string();
        // Matching metadata/cache labels do not establish an external HF
        // directory root. The current producer observes managed packages only.
        let mut record = library.index.get(MODEL_ID).unwrap().unwrap();
        record.metadata["storage_kind"] = serde_json::json!("external_reference");
        library.index.upsert(&record).unwrap();
        let mut row = cached(&library, scope);
        if scope == ModelPackageFactsCacheScope::Summary {
            let mut facts: ResolvedModelPackageFactsSummary =
                serde_json::from_str(&row.facts_json).unwrap();
            facts.storage_kind = StorageKind::ExternalReference;
            facts.entry_path = weight_path.clone();
            facts.model_ref.selected_artifact_path = Some(weight_path.clone());
            row.facts_json = serde_json::to_string(&facts).unwrap();
        } else {
            remove_summary(&library);
            let mut facts: ResolvedModelPackageFacts =
                serde_json::from_str(&row.facts_json).unwrap();
            facts.artifact.storage_kind = StorageKind::ExternalReference;
            facts.artifact.entry_path = weight_path.clone();
            facts.model_ref.selected_artifact_path = Some(weight_path.clone());
            row.facts_json = serde_json::to_string(&facts).unwrap();
        }
        library
            .index
            .upsert_model_package_facts_cache(&row)
            .unwrap();
        let record_before = library.index.get(MODEL_ID).unwrap();
        let cursor_before = library.index.current_model_library_update_cursor().unwrap();
        let response = library
            .resolve_model_artifact_load_target(hf_request(
                PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
            ))
            .await
            .unwrap();
        assert!(
            !response.is_ready(),
            "{scope:?} advertised a weights file as an external HF directory: {response:?}"
        );
        assert!(response.target.is_none());
        assert_eq!(response.artifact_state, ModelArtifactState::Invalid);
        assert_eq!(response.entry_path_state, ModelEntryPathState::Invalid);
        assert_eq!(
            response.diagnostics[0].code,
            PumasArtifactLoadTargetDiagnosticCode::InvalidArtifact
        );
        assert_eq!(
            response.diagnostics[0].field_path.as_deref(),
            Some("target.storage_kind")
        );
        assert_eq!(cached(&library, scope), row);
        assert_eq!(
            serde_json::to_value(library.index.get(MODEL_ID).unwrap()).unwrap(),
            serde_json::to_value(record_before).unwrap()
        );
        assert_eq!(
            library.index.current_model_library_update_cursor().unwrap(),
            cursor_before
        );
    }
}

#[tokio::test]
async fn hf_producer_refuses_unqualified_external_directory_claims() {
    let (_temp, library, model_dir) = synthetic_cohere_library().await;
    let mut metadata: ModelMetadata =
        serde_json::from_slice(&std::fs::read(model_dir.join("metadata.json")).unwrap()).unwrap();
    metadata.storage_kind = Some(StorageKind::ExternalReference);
    library.save_metadata(&model_dir, &metadata).await.unwrap();
    library.index_model_dir(&model_dir).await.unwrap();
    // Exercise the actual public producer, not a hand-written target/cache row.
    let facts = library.resolve_model_package_facts(MODEL_ID).await.unwrap();
    assert_eq!(
        facts.artifact.artifact_kind,
        PackageArtifactKind::HfCompatibleDirectory
    );
    assert_eq!(facts.artifact.storage_kind, StorageKind::ExternalReference);
    assert_eq!(
        facts.artifact.entry_path,
        model_dir.join("model.safetensors").display().to_string()
    );
    for mode in [
        PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
        PumasArtifactLoadTargetResolutionMode::OwnerFresh,
    ] {
        let response = library
            .resolve_model_artifact_load_target(hf_request(mode))
            .await
            .unwrap();
        assert!(
            !response.is_ready(),
            "{mode:?} approved the producer's weight entry as an HF directory: {response:?}"
        );
        assert!(response.target.is_none());
        assert_eq!(response.artifact_state, ModelArtifactState::Invalid);
        assert_eq!(response.entry_path_state, ModelEntryPathState::Invalid);
        assert_eq!(
            response.diagnostics[0].code,
            PumasArtifactLoadTargetDiagnosticCode::InvalidArtifact
        );
        assert_eq!(
            response.diagnostics[0].field_path.as_deref(),
            Some("target.storage_kind")
        );
    }
}

#[tokio::test]
async fn hf_legacy_weight_shaped_facts_are_refused_then_owner_reobserves() {
    let (_temp, library, model_dir) = synthetic_cohere_library().await;
    let original = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let weight_path = model_dir.join("model.safetensors").display().to_string();
    let mut summary = cached(&library, ModelPackageFactsCacheScope::Summary);
    let mut summary_facts: ResolvedModelPackageFactsSummary =
        serde_json::from_str(&summary.facts_json).unwrap();
    summary_facts.entry_path = weight_path.clone();
    summary_facts.model_ref.selected_artifact_path = Some(weight_path.clone());
    summary_facts.model_ref.revision = None;
    summary.facts_json = serde_json::to_string(&summary_facts).unwrap();
    library
        .index
        .upsert_model_package_facts_cache(&summary)
        .unwrap();
    let mut detail = cached(&library, ModelPackageFactsCacheScope::Detail);
    let mut detail_facts: ResolvedModelPackageFacts =
        serde_json::from_str(&detail.facts_json).unwrap();
    detail_facts.artifact.entry_path = weight_path.clone();
    detail_facts.model_ref.selected_artifact_path = Some(weight_path);
    detail_facts.model_ref.revision = None;
    detail.facts_json = serde_json::to_string(&detail_facts).unwrap();
    library
        .index
        .upsert_model_package_facts_cache(&detail)
        .unwrap();
    let refused = library
        .resolve_model_artifact_load_target(hf_request(
            PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
        ))
        .await
        .unwrap();
    assert!(
        !refused.is_ready(),
        "legacy facts were relabelled: {refused:?}"
    );
    assert_eq!(
        cached(&library, ModelPackageFactsCacheScope::Summary),
        summary
    );
    assert_eq!(
        cached(&library, ModelPackageFactsCacheScope::Detail),
        detail
    );
    let refreshed = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_eq!(refreshed, original);
}

#[tokio::test]
async fn hf_target_requires_nonempty_source_fingerprint_and_matching_selected_identity() {
    let (_temp, library, _model_dir) = synthetic_cohere_library().await;
    ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    for invalid_token in [
        "  ".to_string(),
        "f".repeat(64),
        format!("pumas-package-observation-v99:sha256:{}", "f".repeat(64)),
    ] {
        let mut row = cached(&library, ModelPackageFactsCacheScope::Summary);
        row.source_fingerprint = invalid_token;
        library
            .index
            .upsert_model_package_facts_cache(&row)
            .unwrap();
        let response = library
            .resolve_model_artifact_load_target(hf_request(
                PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
            ))
            .await
            .unwrap();
        assert!(
            !response.is_ready(),
            "unversioned, empty or unknown observation must not authorize a target: {response:?}"
        );
        assert_eq!(cached(&library, ModelPackageFactsCacheScope::Summary), row);
        ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    }
    let mut row = cached(&library, ModelPackageFactsCacheScope::Summary);
    let mut facts: ResolvedModelPackageFactsSummary =
        serde_json::from_str(&row.facts_json).unwrap();
    facts.model_ref.model_id = "audio/cohere/other".into();
    row.facts_json = serde_json::to_string(&facts).unwrap();
    library
        .index
        .upsert_model_package_facts_cache(&row)
        .unwrap();
    let response = library
        .resolve_model_artifact_load_target(hf_request(
            PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
        ))
        .await
        .unwrap();
    assert!(
        !response.is_ready(),
        "cached identity mismatch must not authorize a target"
    );
}

#[tokio::test]
async fn hf_package_observation_is_stable_across_fresh_processes() {
    let (_temp, library, _model_dir) = synthetic_cohere_library().await;
    let original = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    let token = original.content_fingerprint.unwrap();
    for _ in 0..3 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg(
                "model_library::library::tests::hf_load_target_tests::hf_package_observation_child",
            )
            .arg("--exact")
            .arg("--nocapture")
            .env("PUMAS_SYNTHETIC_HF_ROOT", library.library_root())
            .env("PUMAS_SYNTHETIC_HF_TOKEN", &token)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child observation failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("running 1 test"),
            "child filter failed"
        );
    }
}

#[tokio::test]
async fn hf_package_observation_child() {
    let Ok(root) = std::env::var("PUMAS_SYNTHETIC_HF_ROOT") else {
        return;
    };
    let expected = std::env::var("PUMAS_SYNTHETIC_HF_TOKEN").unwrap();
    let library = ModelLibrary::new(root).await.unwrap();
    let target = ready_target(&library, PumasArtifactLoadTargetResolutionMode::OwnerFresh).await;
    assert_eq!(
        target.content_fingerprint.as_deref(),
        Some(expected.as_str())
    );
}
