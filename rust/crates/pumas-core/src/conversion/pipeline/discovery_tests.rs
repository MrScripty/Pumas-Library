//! Filesystem discovery contracts, not model-content or retained-read proofs.

use super::list_files_with_extension;
use crate::PumasError;

#[tokio::test]
async fn discovery_returns_sorted_regular_files_with_exact_extensions_only() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "z.gguf",
        "a.gguf",
        "other.safetensors",
        "uppercase.GGUF",
        "gguf",
    ] {
        std::fs::write(root.path().join(name), "fixture").unwrap();
    }
    std::fs::create_dir(root.path().join("directory.gguf")).unwrap();
    std::fs::write(root.path().join("directory.gguf/nested.gguf"), "nested").unwrap();
    #[cfg(unix)]
    let _socket = std::os::unix::net::UnixListener::bind(root.path().join("socket.gguf")).unwrap();
    assert_eq!(
        list_files_with_extension(root.path(), "gguf")
            .await
            .unwrap(),
        vec![root.path().join("a.gguf"), root.path().join("z.gguf")]
    );
    assert_eq!(
        list_files_with_extension(root.path(), "safetensors")
            .await
            .unwrap(),
        vec![root.path().join("other.safetensors")]
    );
}

#[tokio::test]
async fn discovery_preserves_missing_and_nondirectory_root_io_context() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("not-a-directory");
    std::fs::write(&file, "fixture").unwrap();
    for path in [root.path().join("missing"), file] {
        let error = list_files_with_extension(&path, "gguf").await.unwrap_err();
        assert!(
            matches!(&error, PumasError::Io { message, path: Some(failed_path), source: Some(_), .. } if failed_path == &path && message.starts_with("reading model directory:")),
            "{error}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn discovery_follows_matching_symlinks_and_preserves_dangling_link_error() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("source");
    std::fs::create_dir(&directory).unwrap();
    let file = root.path().join("real-model");
    std::fs::write(&file, "fixture").unwrap();
    let regular_link = directory.join("regular.gguf");
    std::os::unix::fs::symlink(&file, &regular_link).unwrap();
    std::os::unix::fs::symlink(root.path(), directory.join("directory.gguf")).unwrap();
    std::os::unix::fs::symlink(root.path().join("missing"), directory.join("ignored.bin")).unwrap();
    assert_eq!(
        list_files_with_extension(&directory, "gguf").await.unwrap(),
        vec![regular_link]
    );
    let dangling = directory.join("dangling.gguf");
    std::os::unix::fs::symlink(root.path().join("missing"), &dangling).unwrap();
    let error = list_files_with_extension(&directory, "gguf")
        .await
        .unwrap_err();
    assert!(
        matches!(&error, PumasError::Io { message, path: Some(path), source: Some(source), .. } if path == &dangling && message.starts_with("inspecting model file:") && source.kind() == std::io::ErrorKind::NotFound),
        "{error}"
    );
    assert!(std::fs::symlink_metadata(dangling)
        .unwrap()
        .file_type()
        .is_symlink());
}
