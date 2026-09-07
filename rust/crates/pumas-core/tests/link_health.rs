//! Public headless link-health contract; no application or GUI startup.

use pumas_library::model_library::{LinkEntry, LinkRegistry, LinkType};
use pumas_library::PumasError;
use std::path::Path;

fn entry(root: &Path, target: &str) -> LinkEntry {
    LinkEntry {
        model_id: "llm/example/model".into(),
        source: root.join("source"),
        target: root.join(target),
        link_type: LinkType::Copy,
        created_at: "2026-09-06T00:00:00Z".into(),
        app_id: "embedded-consumer".into(),
        app_version: None,
    }
}

#[tokio::test]
async fn public_registry_health_is_read_only_and_reports_registered_links() {
    let temp = tempfile::TempDir::new().unwrap();
    let registry_path = temp.path().join("links.json");
    let registry = LinkRegistry::new(&registry_path);
    assert_eq!(registry.health().await.unwrap().status, "healthy");
    std::fs::write(temp.path().join("healthy"), b"model").unwrap();
    std::fs::write(temp.path().join("unregistered"), b"unrelated").unwrap();
    registry
        .register(entry(temp.path(), "healthy"))
        .await
        .unwrap();
    registry
        .register(entry(temp.path(), "missing"))
        .await
        .unwrap();
    let before = std::fs::read(&registry_path).unwrap();

    let reopened = LinkRegistry::new(&registry_path);
    reopened.load().await.unwrap();
    let health = reopened.health().await.unwrap();
    assert!(health.success);
    assert!(health.error.is_none());
    assert_eq!(health.status, "degraded");
    assert_eq!((health.total_links, health.healthy_links), (2, 1));
    assert_eq!(
        health.broken_links,
        vec![temp.path().join("missing").to_string_lossy().into_owned()]
    );
    assert!(health.orphaned_links.is_empty());
    assert_eq!(std::fs::read(&registry_path).unwrap(), before);
    assert_eq!(
        std::fs::read(temp.path().join("healthy")).unwrap(),
        b"model"
    );
    assert_eq!(
        std::fs::read(temp.path().join("unregistered")).unwrap(),
        b"unrelated"
    );
    assert!(!temp.path().join("missing").exists());
}

#[tokio::test]
async fn public_registry_health_preserves_inspection_failure_without_cleanup() {
    let temp = tempfile::TempDir::new().unwrap();
    let registry_path = temp.path().join("links.json");
    let registry = LinkRegistry::new(&registry_path);
    std::fs::write(temp.path().join("not-a-directory"), b"preserve").unwrap();
    registry
        .register(entry(temp.path(), "not-a-directory/child"))
        .await
        .unwrap();
    let before = std::fs::read(&registry_path).unwrap();
    assert!(matches!(
        registry.health().await,
        Err(PumasError::Io { .. })
    ));
    assert_eq!(std::fs::read(&registry_path).unwrap(), before);
    assert_eq!(registry.get_all().await.len(), 1);
    assert_eq!(
        std::fs::read(temp.path().join("not-a-directory")).unwrap(),
        b"preserve"
    );
}
