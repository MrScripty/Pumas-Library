//! Version dependency handlers.

use crate::handlers::{path_exists, read_utf8_file, require_version_manager};
use crate::server::AppState;

pub async fn check_version_dependencies(
    state: &AppState,
    app_id: &str,
    tag: &str,
) -> pumas_library::Result<crate::contract::CheckVersionDependenciesOutcome> {
    let vm = require_version_manager(state, app_id).await?;
    let status = vm.check_dependencies(tag).await?;
    Ok(crate::contract::CheckVersionDependenciesOutcome::new(
        status,
    ))
}

pub async fn install_version_dependencies(
    state: &AppState,
    app_id: &str,
    tag: &str,
) -> pumas_library::Result<crate::contract::InstallVersionDependenciesOutcome> {
    let vm = require_version_manager(state, app_id).await?;
    vm.install_dependencies(tag, None)
        .await
        .map(crate::contract::InstallVersionDependenciesOutcome::new)
}

pub async fn get_release_dependencies(
    state: &AppState,
    app_id: &str,
    tag: &str,
) -> pumas_library::Result<crate::contract::GetReleaseDependenciesOutcome> {
    let vm = require_version_manager(state, app_id).await?;
    let requirements_path = vm.version_path(tag).join("requirements.txt");
    read_release_dependencies(&requirements_path)
        .await
        .map(crate::contract::GetReleaseDependenciesOutcome::new)
}

async fn read_release_dependencies(
    requirements_path: &std::path::Path,
) -> pumas_library::Result<Vec<String>> {
    if !path_exists(requirements_path).await? {
        return Ok(vec![]);
    }
    let content = read_utf8_file(requirements_path).await?;

    // Preserve the existing simple extraction; this is not requirements validation.
    Ok(content
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with('#') && !line.starts_with('-')
        })
        .filter_map(|line| {
            let name = line.split(['=', '>', '<', '[', ';']).next()?.trim();
            if !name.is_empty() {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn install_version_dependencies_preserves_safe_producer_branches() {
        use pumas_app_manager::version_manager::{ConstraintsManager, DependencyManager};
        use pumas_library::config::AppId;
        use pumas_library::PumasError;

        let temp = tempfile::TempDir::new().unwrap();
        let manager = DependencyManager::new(
            temp.path().to_path_buf(),
            AppId::Torch,
            temp.path().join("pip-cache"),
        );
        let constraints = ConstraintsManager::new(temp.path().join("constraints"));
        assert!(matches!(
            manager.install_dependencies("missing", &constraints, None).await,
            Err(PumasError::VersionNotFound { tag }) if tag == "missing"
        ));

        let version_path = temp
            .path()
            .join(AppId::Torch.versions_dir_name())
            .join("fixture");
        let python = version_path.join("venv/bin/python");
        tokio::fs::create_dir_all(python.parent().unwrap())
            .await
            .unwrap();
        // Inert, non-executable marker admits only branches before process creation.
        let marker = b"inert fixture: never execute";
        tokio::fs::write(&python, marker).await.unwrap();
        let raw = manager
            .install_dependencies("fixture", &constraints, None)
            .await
            .unwrap();
        assert!(raw);
        assert_eq!(
            serde_json::to_value(crate::contract::InstallVersionDependenciesOutcome::new(raw))
                .unwrap(),
            serde_json::json!({"success":true})
        );
        let requirements = version_path.join("requirements.txt");
        tokio::fs::write(&requirements, [0xff]).await.unwrap();
        assert!(matches!(
            manager.install_dependencies("fixture", &constraints, None).await,
            Err(PumasError::Io { path: Some(path), .. }) if path == requirements
        ));
        assert_eq!(tokio::fs::read(&python).await.unwrap(), marker);
        assert!(!temp.path().join("pip-cache").exists());
        assert!(!temp.path().join("constraints").exists());
    }

    #[tokio::test]
    async fn get_release_dependencies_preserves_filesystem_and_parser_semantics() {
        let temp = tempfile::TempDir::new().unwrap();
        let absent = temp.path().join("missing-version/requirements.txt");
        assert_eq!(
            read_release_dependencies(&absent).await.unwrap(),
            Vec::<String>::new()
        );
        let path = temp.path().join("requirements.txt");
        assert!(read_release_dependencies(&path).await.unwrap().is_empty());
        for content in [
            "",
            "  # comment\n\n-r other.txt\n--extra-index-url ignored\n",
        ] {
            tokio::fs::write(&path, content).await.unwrap();
            assert!(read_release_dependencies(&path).await.unwrap().is_empty());
        }
        tokio::fs::write(&path, "# Essential\n torch==2\nnumpy>=1\ntorch==3\n# Non essential\n λ-package==1\n Pillow[extra]; marker\nfoo!=1\nbar~=2\npkg @ https://example.invalid/pkg.whl\nname # retained inline comment\n; omitted\n").await.unwrap();
        assert_eq!(
            read_release_dependencies(&path).await.unwrap(),
            vec![
                "torch",
                "numpy",
                "torch",
                "λ-package",
                "Pillow",
                "foo!",
                "bar~",
                "pkg @ https://example.invalid/pkg.whl",
                "name # retained inline comment",
            ]
        );
        tokio::fs::write(&path, [0xff]).await.unwrap();
        assert!(matches!(
            read_release_dependencies(&path).await,
            Err(pumas_library::PumasError::Io { .. })
        ));
        assert!(matches!(
            read_release_dependencies(temp.path()).await,
            Err(pumas_library::PumasError::Io { .. })
        ));
    }
}
