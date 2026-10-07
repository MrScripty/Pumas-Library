use super::*;
use crate::build_info::ProtocolAdvertisement;
use crate::registry::LibraryRegistry;
use crate::PumasApi;
use std::path::PathBuf;
use tempfile::TempDir;

async fn fixture() -> (TempDir, LibraryRegistry, PathBuf, PumasApi) {
    let temp = TempDir::new().unwrap();
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let api = PumasApi::builder(&root)
        .with_registry(registry.clone())
        .with_hf_client(false)
        .with_process_manager(false)
        .with_connectivity_probe(false)
        .build()
        .await
        .unwrap();
    api.start_ipc_server().await.unwrap();
    (temp, registry, root, api)
}
fn http_build() -> crate::PumasBuildInfo {
    let mut info = crate::PumasBuildInfo::library();
    info.protocols.push(ProtocolAdvertisement {
        name: LOCAL_HTTP_PROTOCOL.into(),
        versions: vec![1],
    });
    info.schemas.push(crate::build_info::SchemaAdvertisement {
        name: "pumas.http-advertisement".into(),
        version: HTTP_ADVERTISEMENT_SCHEMA_VERSION,
    });
    info
}
fn endpoint(port: u16) -> LoopbackHttpEndpoint {
    LoopbackHttpEndpoint::parse(format!("http://127.0.0.1:{port}")).unwrap()
}
#[test]
fn endpoints_reject_remote_hosts_credentials_and_ambiguous_context() {
    for url in [
        "https://127.0.0.1:1",
        "http://localhost:1",
        "http://192.0.2.1:1",
        "http://127.0.0.1:0",
        "http://user@127.0.0.1:1",
        "http://127.0.0.1:1/path",
        "http://127.0.0.1:1?secret",
        "http://127.0.0.1:1#fragment",
    ] {
        assert!(LoopbackHttpEndpoint::parse(url).is_err(), "{url}");
        assert!(serde_json::from_value::<LoopbackHttpEndpoint>(serde_json::json!(url)).is_err());
    }
    assert_eq!(
        LoopbackHttpEndpoint::parse("http://[::1]:8000/")
            .unwrap()
            .as_str(),
        "http://[::1]:8000"
    );
}
#[tokio::test]
async fn publication_is_explicit_and_same_owner_cannot_replace_live_service() {
    let (temp, registry, root, api) = fixture().await;
    let observer = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
    let mut first = api.prepare_http_service(endpoint(1), http_build()).unwrap();
    assert!(observer
        .snapshot()
        .unwrap()
        .advertised_http_services
        .is_empty());
    first.publish().unwrap();
    let description = observer
        .snapshot()
        .unwrap()
        .advertised_http_services
        .remove(0);
    assert_eq!(description, *first.description());
    assert!(!serde_json::to_string(&description)
        .unwrap()
        .contains("connection_token"));
    let mut second = api.prepare_http_service(endpoint(2), http_build()).unwrap();
    assert!(second.publish().is_err());
    assert_eq!(registry.list_http_services().unwrap()[0], description);
    assert!(first.revoke().unwrap());
    first.complete_shutdown(Ok(())).unwrap();
    second.publish().unwrap();
    assert!(!registry
        .revoke_http_service(
            api.primary().ready_instance.get().unwrap(),
            &description.service_generation
        )
        .unwrap());
    assert_eq!(
        registry.list_http_services().unwrap()[0],
        *second.description()
    );
    second.revoke().unwrap();
    second.complete_shutdown(Ok(())).unwrap();
    api.shutdown_instance().await.unwrap();
    assert!(registry.get_instance(&root).unwrap().is_none());
}
#[tokio::test]
async fn completed_registration_cannot_republish_and_fresh_registration_retains_custody() {
    for published_before_completion in [false, true] {
        let (_temp, registry, root, api) = fixture().await;
        let mut completed = api.prepare_http_service(endpoint(1), http_build()).unwrap();
        if published_before_completion {
            completed.publish().unwrap();
            assert!(completed.revoke().unwrap());
        }
        completed.complete_shutdown(Ok(())).unwrap();
        assert!(completed.publish().is_err());
        assert!(registry.list_http_services().unwrap().is_empty());

        let mut fresh = api.prepare_http_service(endpoint(2), http_build()).unwrap();
        assert_ne!(
            fresh.description().service_generation,
            completed.description().service_generation
        );
        fresh.publish().unwrap();
        let receipt = crate::api::instance_shutdown::begin(api.primary());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), receipt.clone())
                .await
                .is_err()
        );
        assert!(registry.get_instance(&root).unwrap().is_some());
        fresh.revoke().unwrap();
        fresh.complete_shutdown(Ok(())).unwrap();
        receipt.await.unwrap();
        assert!(registry.get_instance(&root).unwrap().is_none());
    }
}

#[tokio::test]
async fn stalled_second_core_description_expires_without_reclaiming_owner() {
    use crate::ipc::server::{IpcDispatch, IpcServer};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Notify;

    struct Reply {
        identity: Mutex<Option<(InstanceDescription, String)>>,
        requests: AtomicUsize,
        stalled: Notify,
        release: Notify,
    }
    #[async_trait::async_trait]
    impl IpcDispatch for Reply {
        async fn dispatch(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> Result<serde_json::Value> {
            assert_eq!(method, "describe_instance");
            let (description, token) = self.identity.lock().unwrap().as_ref().unwrap().clone();
            assert_eq!(params["connection_token"], token);
            if self.requests.fetch_add(1, Ordering::SeqCst) > 0 {
                self.stalled.notify_one();
                self.release.notified().await;
            }
            Ok(serde_json::to_value(description)?)
        }
    }
    let temp = TempDir::new().unwrap();
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    registry.register(&root, "test").unwrap();
    let reply = Arc::new(Reply {
        identity: Mutex::new(None),
        requests: AtomicUsize::new(0),
        stalled: Notify::new(),
        release: Notify::new(),
    });
    let ipc = IpcServer::start(reply.clone()).await.unwrap();
    registry
        .register_instance(&root, std::process::id(), ipc.port)
        .unwrap();
    let instance = registry.get_instance(&root).unwrap().unwrap();
    let library = registry.get_by_path(&root).unwrap().unwrap();
    let description = InstanceDescription::local(&library, &instance);
    *reply.identity.lock().unwrap() = Some((
        description.clone(),
        instance.connection_token.clone().unwrap(),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let advertised = HttpServiceDescription {
        advertisement_schema_version: HTTP_ADVERTISEMENT_SCHEMA_VERSION,
        service_generation: uuid::Uuid::new_v4().to_string(),
        instance: description,
        endpoint: endpoint(listener.local_addr().unwrap().port()),
        build_info: http_build(),
    };
    registry
        .publish_http_service(&instance, &advertised)
        .unwrap();
    let body = serde_json::to_string(&advertised).unwrap();
    let http = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    });
    let observer = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
    let selected_root = root.clone();
    let borrower = tokio::spawn(async move {
        observer
            .borrow_http_service(&selected_root, &CompatibilityRequirements::default())
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), reply.stalled.notified())
        .await
        .expect("the final authenticated describe request must reach the peer");
    http.await.unwrap();
    assert_eq!(reply.requests.load(Ordering::SeqCst), 2);

    // Pause only after real sockets reach the stalled stage, avoiding simulated
    // time racing ahead of the first IPC/HTTP response on a busy test runner.
    tokio::time::pause();
    tokio::time::advance(crate::config::RegistryConfig::PRIMARY_READY_TIMEOUT).await;
    let result = tokio::time::timeout(Duration::from_secs(1), borrower).await;
    reply.release.notify_one();
    ipc.shutdown_and_wait().await.unwrap();
    assert!(result
        .expect("final reauthentication must be bounded")
        .unwrap()
        .is_err());
    let retained = registry.get_instance(&root).unwrap().unwrap();
    assert_eq!(retained.connection_token, instance.connection_token);
    assert_eq!(retained.started_at, instance.started_at);
    assert_eq!(retained.status, instance.status);
    assert_eq!(registry.list_http_services().unwrap(), vec![advertised]);
    assert!(PumasApi::builder(&root)
        .with_registry(registry.clone())
        .build()
        .await
        .is_err());
    assert!(!root.join("shared-resources").exists());
}

#[tokio::test]
async fn stale_publisher_and_revoke_cannot_touch_successor_advertisement() {
    let (_temp, registry, root, first) = fixture().await;
    let mut stale = first
        .prepare_http_service(endpoint(1), http_build())
        .unwrap();
    stale.publish().unwrap();
    let mut pending = first
        .prepare_http_service(endpoint(2), http_build())
        .unwrap();
    // Administrative replacement is solely a deterministic hostile-race fixture.
    registry.unregister_instance(&root).unwrap();
    let second = PumasApi::builder(&root)
        .with_registry(registry.clone())
        .with_hf_client(false)
        .with_process_manager(false)
        .with_connectivity_probe(false)
        .build()
        .await
        .unwrap();
    second.start_ipc_server().await.unwrap();
    let mut current = second
        .prepare_http_service(endpoint(3), http_build())
        .unwrap();
    current.publish().unwrap();
    assert!(pending.publish().is_err());
    assert!(!stale.revoke().unwrap());
    assert_eq!(
        registry.list_http_services().unwrap()[0],
        *current.description()
    );
    stale.complete_shutdown(Ok(())).unwrap();
    pending.complete_shutdown(Ok(())).unwrap();
    first.shutdown_instance().await.unwrap();
    assert_eq!(
        registry.list_http_services().unwrap()[0],
        *current.description()
    );
    current.revoke().unwrap();
    current.complete_shutdown(Ok(())).unwrap();
    second.shutdown_instance().await.unwrap();
}
#[tokio::test]
async fn owner_shutdown_waits_for_external_receipt_and_blocks_late_publication() {
    let (_temp, registry, root, api) = fixture().await;
    let mut service = api.prepare_http_service(endpoint(1), http_build()).unwrap();
    let receipt = crate::api::instance_shutdown::begin(api.primary());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), receipt.clone())
            .await
            .is_err()
    );
    assert!(registry.get_instance(&root).unwrap().is_some());
    assert!(service.publish().is_err());
    service.complete_shutdown(Ok(())).unwrap();
    receipt.await.unwrap();
    assert!(registry.get_instance(&root).unwrap().is_none());
}
#[tokio::test]
async fn abandoned_external_receipt_retains_owner_even_after_listener_is_gone() {
    let (_temp, registry, root, api) = fixture().await;
    let service = api.prepare_http_service(endpoint(1), http_build()).unwrap();
    drop(service);
    assert!(api.shutdown_instance().await.is_err());
    assert!(registry.get_instance(&root).unwrap().is_some());
}
#[tokio::test]
async fn changed_http_context_and_redirect_are_unresolved_without_registry_mutation() {
    for redirect in [false, true] {
        let (temp, registry, root, api) = fixture().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut registration = api
            .prepare_http_service(
                endpoint(listener.local_addr().unwrap().port()),
                http_build(),
            )
            .unwrap();
        registration.publish().unwrap();
        let before = registry.get_instance(&root).unwrap().unwrap();
        let mut wrong = registration.description().clone();
        wrong.instance.capabilities.clear();
        let body = serde_json::to_string(&wrong).unwrap();
        let server = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            let response = if redirect {
                "HTTP/1.1 302 Found\r\nLocation: http://192.0.2.1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            };
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let observer = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
        assert!(observer
            .borrow_http_service(&root, &CompatibilityRequirements::default())
            .await
            .is_err());
        server.await.unwrap();
        assert_eq!(
            registry
                .get_instance(&root)
                .unwrap()
                .unwrap()
                .connection_token,
            before.connection_token
        );
        assert_eq!(
            registry.list_http_services().unwrap()[0],
            *registration.description()
        );
        registration.revoke().unwrap();
        registration.complete_shutdown(Ok(())).unwrap();
        api.shutdown_instance().await.unwrap();
    }
}
#[test]
fn legacy_http_observation_does_not_migrate_registry() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("registry.db");
    drop(LibraryRegistry::open_at(&path).unwrap());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute("DROP TABLE http_services", []).unwrap();
    let observer = LocalDiscovery::open_at(&path).unwrap();
    assert!(observer
        .snapshot()
        .unwrap()
        .advertised_http_services
        .is_empty());
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='http_services')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!exists);
}
