//! Test the public live/cache result and actual retained files under budget pressure.
use super::super::budget;
use super::*;
use chrono::Timelike;

fn observation(client: &HuggingFaceClient, repo: &str, age: i64) -> Observation {
    Observation {
        version: 1,
        url: format!("{}/models/{repo}", client.api_base_url()),
        body: Some(RawValue::from_string(body(repo, 'a', 1)).unwrap()),
        etag: Some("\"owned\"".into()),
        last_modified: None,
        validated_at: Some(
            Utc::now().with_nanosecond(0).unwrap() - chrono::TimeDelta::seconds(age),
        ),
        ttl_seconds: REPO_CACHE_TTL_SECS,
        blocked_until: None,
    }
}

fn seed(client: &HuggingFaceClient, observation: &Observation) -> PathBuf {
    let path = cache_path(client, &observation.url);
    atomic_write_json(&path, observation, false).unwrap();
    path
}

fn snapshot(root: &std::path::Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), std::fs::read(path).unwrap())
        })
        .collect()
}

#[tokio::test]
async fn actual_public_lookup_evicts_oldest_at_default_record_limit_and_reopens() {
    let fixture = Fixture::new(vec![("200 OK", "", body("acme/new", 'b', 2))]).await;
    let root = tempfile::tempdir().unwrap();
    let client = fixture.client(root.path()).await;
    let mut seeded = Vec::new();
    for index in 0..HuggingFaceClient::MODEL_DETAIL_CACHE_RECORD_LIMIT {
        seeded.push(seed(
            &client,
            &observation(&client, &format!("acme/seed{index}"), 1000 - index as i64),
        ));
    }
    let foreign = root.path().join("hf_acme_model_files.json");
    std::fs::write(&foreign, b"owned foreign tree sentinel").unwrap();
    assert_eq!(client.model_detail_cache_usage().await.unwrap().0, 256);
    assert_eq!(
        client
            .get_model_info_cached("acme/new")
            .await
            .unwrap()
            .downloads,
        Some(2)
    );
    assert!(!seeded[0].exists());
    assert!(seeded[1..].iter().all(|path| path.exists()));
    assert_eq!(
        std::fs::read(&foreign).unwrap(),
        b"owned foreign tree sentinel"
    );
    let (records, bytes) = client.model_detail_cache_usage().await.unwrap();
    assert_eq!(records, 256);
    assert!(bytes <= HuggingFaceClient::MODEL_DETAIL_CACHE_BYTE_LIMIT);
    drop(client);
    let reopened = fixture.client(root.path()).await;
    assert_eq!(
        reopened
            .get_model_info_cached("acme/new")
            .await
            .unwrap()
            .downloads,
        Some(2)
    );
    assert_eq!(fixture.count(), 1);
    assert_eq!(
        reopened.model_detail_cache_usage().await.unwrap(),
        (records, bytes)
    );
}

#[tokio::test]
async fn byte_pressure_evicts_oldest_and_replacement_counts_existing_bytes_once() {
    let root = tempfile::tempdir().unwrap();
    let client = HuggingFaceClient::new(root.path()).unwrap();
    let first = observation(&client, "acme/aaa", 30);
    let second = observation(&client, "acme/bbb", 20);
    let mut third = observation(&client, "acme/ccc", 10);
    let size = serde_json::to_vec_pretty(&first).unwrap().len() as u64;
    let first_path = seed(&client, &first);
    let second_path = seed(&client, &second);
    let third_path = cache_path(&client, &third.url);
    budget::publish_with_limits(&third_path, &third, 10, size * 2).unwrap();
    assert!(!first_path.exists());
    assert!(second_path.exists());
    assert_eq!(
        client.model_detail_cache_usage().await.unwrap(),
        (2, size * 2)
    );
    third.etag = None;
    budget::publish_with_limits(&third_path, &third, 2, size * 2).unwrap();
    assert!(second_path.exists());
    assert_eq!(
        std::fs::read(&third_path).unwrap(),
        serde_json::to_vec_pretty(&third).unwrap()
    );
    assert_eq!(
        client.model_detail_cache_usage().await.unwrap().1,
        std::fs::metadata(second_path).unwrap().len()
            + std::fs::metadata(third_path).unwrap().len()
    );
}

#[tokio::test]
async fn active_cooldown_is_protected_but_expired_cooldown_is_eligible() {
    let root = tempfile::tempdir().unwrap();
    let client = HuggingFaceClient::new(root.path()).unwrap();
    let mut active = observation(&client, "acme/active", 100);
    active.blocked_until = Some(Utc::now() + chrono::TimeDelta::minutes(5));
    let active_path = seed(&client, &active);
    let before = std::fs::read(&active_path).unwrap();
    let eligible = seed(&client, &observation(&client, "acme/eligible", 10));
    let new = observation(&client, "acme/new", 0);
    budget::publish_with_limits(&cache_path(&client, &new.url), &new, 2, 8192).unwrap();
    assert_eq!(std::fs::read(&active_path).unwrap(), before);
    assert!(!eligible.exists());
    active.blocked_until = Some(Utc::now() - chrono::TimeDelta::seconds(1));
    seed(&client, &active);
    let next = observation(&client, "acme/next", 0);
    budget::publish_with_limits(&cache_path(&client, &next.url), &next, 2, 8192).unwrap();
    assert!(!active_path.exists());
}

#[tokio::test]
async fn all_protected_capacity_preserves_disk_and_live_lookup_success() {
    let fixture = Fixture::new(vec![
        ("200 OK", "", body("acme/new", 'a', 1)),
        ("200 OK", "", body("acme/new", 'b', 2)),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    let client = fixture.client(root.path()).await;
    for index in 0..256 {
        let mut blocked = observation(&client, &format!("acme/protected{index}"), 100);
        blocked.body = None;
        blocked.blocked_until = Some(Utc::now() + chrono::TimeDelta::minutes(5));
        seed(&client, &blocked);
    }
    let before = snapshot(root.path());
    assert_eq!(
        client
            .get_model_info_cached("acme/new")
            .await
            .unwrap()
            .downloads,
        Some(1)
    );
    assert_eq!(
        client
            .get_model_info_cached("acme/new")
            .await
            .unwrap()
            .downloads,
        Some(2)
    );
    assert!(matches!(
        client.get_model_info_cached("acme/protected0").await,
        Err(PumasError::RateLimited { .. })
    ));
    assert_eq!(fixture.count(), 2);
    assert_eq!(snapshot(root.path()), before);
}

#[tokio::test]
async fn corrupt_future_and_wrong_identity_records_are_counted_but_not_evicted() {
    let root = tempfile::tempdir().unwrap();
    let client = HuggingFaceClient::new(root.path()).unwrap();
    let mut future = observation(&client, "acme/future", 1000);
    future.version = 2;
    let future_path = seed(&client, &future);
    let corrupt = seed(&client, &observation(&client, "acme/corrupt", 1000));
    std::fs::write(&corrupt, b"owned malformed JSON").unwrap();
    let wrong = seed(&client, &observation(&client, "acme/wrong", 1000));
    std::fs::write(&wrong, serde_json::to_vec_pretty(&future).unwrap()).unwrap();
    let valid = seed(&client, &observation(&client, "acme/valid", 10));
    let saved: Vec<_> = [&future_path, &corrupt, &wrong]
        .into_iter()
        .map(|path| (path, std::fs::read(path).unwrap()))
        .collect();
    let next = observation(&client, "acme/next", 0);
    budget::publish_with_limits(&cache_path(&client, &next.url), &next, 4, 8192).unwrap();
    assert!(!valid.exists());
    assert_eq!(client.model_detail_cache_usage().await.unwrap().0, 4);
    for (path, bytes) in saved {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}

#[tokio::test]
async fn successful_live_lookup_preserves_an_inadmissible_existing_target() {
    for fault in [
        "json",
        "version",
        "etag",
        "last-modified",
        "last-modified-size",
        "future",
        "body-size",
        "source",
    ] {
        let fixture = Fixture::new(vec![("200 OK", "", body("acme/model", 'b', 2))]).await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        let mut old = observation(&client, "acme/model", 60);
        match fault {
            "version" => old.version = 2,
            "etag" => old.etag = Some("invalid-unquoted-validator".into()),
            "last-modified" => old.last_modified = Some("invalid-date".into()),
            "last-modified-size" => {
                old.last_modified = Some(format!(
                    "Wed, 01 Jan 2025 00:00:00 +0000{}",
                    " ".repeat(1100)
                ))
            }
            "future" => old.validated_at = Some(Utc::now() + chrono::TimeDelta::days(1)),
            "body-size" => {
                old.body = Some(
                    RawValue::from_string(format!(
                        "{{\"modelId\":\"acme/model\",\"owned_padding\":\"{}\"}}",
                        "x".repeat(BODY_LIMIT)
                    ))
                    .unwrap(),
                )
            }
            "source" => old.url = "https://other.invalid/models/acme/model".into(),
            _ => {}
        }
        let path = fixture.path(&client, "acme/model");
        atomic_write_json(&path, &old, false).unwrap();
        if fault == "json" {
            std::fs::write(&path, b"owned corrupt record").unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            client.get_model_info("acme/model").await.unwrap().downloads,
            Some(2),
            "{fault}"
        );
        assert_eq!(std::fs::read(path).unwrap(), before, "{fault}");
        assert_eq!(fixture.count(), 1);
    }
}

#[tokio::test]
async fn overbudget_legacy_classification_refuses_growth_before_eviction() {
    let root = tempfile::tempdir().unwrap();
    let client = HuggingFaceClient::new(root.path()).unwrap();
    let mut old_paths = Vec::new();
    for index in 0..17 {
        let repo = format!("acme/large{index}");
        let mut old = observation(&client, &repo, 60);
        let json = format!(
            "{{\"modelId\":\"{repo}\",\"owned_padding\":\"{}\"}}",
            "x".repeat(BODY_LIMIT - 1024)
        );
        old.body = Some(RawValue::from_string(json).unwrap());
        let path = seed(&client, &old);
        old_paths.push((path.clone(), std::fs::metadata(path).unwrap().len()));
    }
    assert!(
        client.model_detail_cache_usage().await.unwrap().1
            > HuggingFaceClient::MODEL_DETAIL_CACHE_BYTE_LIMIT
    );
    let next = observation(&client, "acme/next", 0);
    let next_path = cache_path(&client, &next.url);
    let error = budget::publish(&next_path, &next).unwrap_err();
    assert!(
        matches!(error, PumasError::Validation { message, .. } if message.contains("scan byte capacity"))
    );
    assert!(!next_path.exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 17);
    for (path, bytes) in old_paths {
        assert_eq!(std::fs::metadata(path).unwrap().len(), bytes);
    }
}

#[tokio::test]
async fn scan_capacity_refuses_persistence_without_erasing_foreign_files() {
    let fixture = Fixture::new(vec![("200 OK", "", body("acme/new", 'a', 1))]).await;
    let root = tempfile::tempdir().unwrap();
    let client = fixture.client(root.path()).await;
    for index in 0..4097 {
        std::fs::write(root.path().join(format!("foreign-{index}")), b"x").unwrap();
    }
    assert!(client.model_detail_cache_usage().await.is_err());
    assert_eq!(
        client
            .get_model_info_cached("acme/new")
            .await
            .unwrap()
            .downloads,
        Some(1)
    );
    assert!(!fixture.path(&client, "acme/new").exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 4097);
}

#[tokio::test]
async fn no_store_still_invalidates_exact_known_target_when_inventory_is_incomplete() {
    let fixture = Fixture::new(vec![(
        "200 OK",
        "Cache-Control: no-store\r\n",
        body("acme/model", 'b', 2),
    )])
    .await;
    let root = tempfile::tempdir().unwrap();
    let client = fixture.client(root.path()).await;
    let target = seed(&client, &observation(&client, "acme/model", 100));
    for index in 0..4097 {
        std::fs::write(root.path().join(format!("foreign-{index}")), b"unchanged").unwrap();
    }
    assert!(client.model_detail_cache_usage().await.is_err());
    assert_eq!(
        client.get_model_info("acme/model").await.unwrap().downloads,
        Some(2)
    );
    assert!(!target.exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 4097);
    assert_eq!(
        std::fs::read(root.path().join("foreign-0")).unwrap(),
        b"unchanged"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn nonregular_namespace_members_never_block_or_touch_external_targets() {
    for kind in ["symlink", "fifo", "directory"] {
        let fixture = Fixture::new(vec![("200 OK", "", body("acme/new", 'a', 1))]).await;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let external = outside.path().join("owned-sentinel");
        std::fs::write(&external, b"external unchanged").unwrap();
        let client = fixture.client(root.path()).await;
        let path = cache_path(&client, &observation(&client, "acme/nonregular", 0).url);
        match kind {
            "symlink" => std::os::unix::fs::symlink(&external, &path).unwrap(),
            "fifo" => nix::unistd::mkfifo(
                &path,
                nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
            )
            .unwrap(),
            _ => std::fs::create_dir(&path).unwrap(),
        }
        assert!(
            tokio::time::timeout(Duration::from_secs(3), client.model_detail_cache_usage())
                .await
                .unwrap()
                .is_err()
        );
        assert_eq!(
            client
                .get_model_info_cached("acme/new")
                .await
                .unwrap()
                .downloads,
            Some(1)
        );
        assert!(std::fs::symlink_metadata(&path).is_ok());
        assert_eq!(std::fs::read(external).unwrap(), b"external unchanged");
        assert!(!fixture.path(&client, "acme/new").exists());
    }
}

#[tokio::test]
async fn concurrent_public_misses_share_the_directory_budget() {
    let fixture = Fixture::new(vec![
        ("200 OK", "", body("acme/one", 'a', 1)),
        ("200 OK", "", body("acme/two", 'b', 2)),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    let client = Arc::new(fixture.client(root.path()).await);
    for index in 0..255 {
        seed(
            &client,
            &observation(&client, &format!("acme/seed{index}"), 1000 - index as i64),
        );
    }
    // Deterministic request order, overlapping owned persistence via an actual effect gate.
    let gate = client.hold_metadata_fixture_effect("acme/one");
    let first = {
        let client = client.clone();
        tokio::spawn(async move { client.get_model_info_cached("acme/one").await })
    };
    tokio::time::timeout(Duration::from_secs(3), gate.entered())
        .await
        .unwrap();
    let second = {
        let client = client.clone();
        tokio::spawn(async move { client.get_model_info_cached("acme/two").await })
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.count() < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    gate.release();
    assert!(tokio::time::timeout(Duration::from_secs(3), first)
        .await
        .unwrap()
        .unwrap()
        .is_ok());
    assert!(tokio::time::timeout(Duration::from_secs(3), second)
        .await
        .unwrap()
        .unwrap()
        .is_ok());
    assert_eq!(client.model_detail_cache_usage().await.unwrap().0, 256);
    assert!(fixture.path(&client, "acme/one").exists());
    assert!(fixture.path(&client, "acme/two").exists());
}

#[tokio::test]
async fn canceled_writer_retains_directory_admission_until_actual_effect_settles() {
    let fixture = Fixture::new(vec![("200 OK", "", body("acme/one", 'a', 1))]).await;
    let root = tempfile::tempdir().unwrap();
    let client = Arc::new(fixture.client(root.path()).await);
    let gate = client.hold_metadata_fixture_effect("acme/one");
    let writer = {
        let client = client.clone();
        tokio::spawn(async move { client.get_model_info_cached("acme/one").await })
    };
    tokio::time::timeout(Duration::from_secs(3), gate.entered())
        .await
        .unwrap();
    let mut inventory = {
        let client = client.clone();
        tokio::spawn(async move { client.model_detail_cache_usage().await })
    };
    writer.abort();
    assert!(writer.await.unwrap_err().is_cancelled());
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut inventory)
            .await
            .is_err()
    );
    assert!(!fixture.path(&client, "acme/one").exists());
    gate.release();
    let usage = tokio::time::timeout(Duration::from_secs(3), inventory)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(usage.0, 1);
    assert_eq!(
        usage.1,
        std::fs::metadata(fixture.path(&client, "acme/one"))
            .unwrap()
            .len()
    );
    assert_eq!(fixture.count(), 1);
}
