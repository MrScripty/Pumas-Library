//! Version dependency handlers.

use crate::handlers::{path_exists, read_utf8_file, require_str_param, require_version_manager};
use crate::server::AppState;
use serde_json::Value;

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
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    let vm = require_version_manager(state, app_id_str).await?;
    let result = vm.install_dependencies(&tag, None).await?;
    Ok(serde_json::to_value(result)?)
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
