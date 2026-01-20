//! Launch script generation for version-specific shortcuts.

use crate::error::{PumasError, Result};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Generator for version-specific launch scripts.
pub struct LaunchScriptGenerator {
    /// Directory to store launch scripts.
    scripts_dir: PathBuf,
    /// Directory to store browser profiles.
    profiles_dir: PathBuf,
    /// Server start delay in seconds.
    server_start_delay: u32,
}

impl LaunchScriptGenerator {
    /// Create a new launch script generator.
    ///
    /// # Arguments
    ///
    /// * `scripts_dir` - Directory to store generated scripts
    /// * `profiles_dir` - Directory for browser profiles
    pub fn new(scripts_dir: impl AsRef<Path>, profiles_dir: impl AsRef<Path>) -> Self {
        Self {
            scripts_dir: scripts_dir.as_ref().to_path_buf(),
            profiles_dir: profiles_dir.as_ref().to_path_buf(),
            server_start_delay: 5,
        }
    }

    /// Set the server start delay.
    pub fn with_server_start_delay(mut self, delay: u32) -> Self {
        self.server_start_delay = delay;
        self
    }

    /// Generate a launch script for a version.
    ///
    /// # Arguments
    ///
    /// * `tag` - Version tag
    /// * `version_dir` - Path to the version installation
    /// * `slug` - Filesystem-safe version identifier
    ///
    /// # Returns
    ///
    /// Path to the generated script.
    pub fn generate(&self, tag: &str, version_dir: &Path, slug: &str) -> Result<PathBuf> {
        // Ensure directories exist
        fs::create_dir_all(&self.scripts_dir).map_err(|e| PumasError::Io {
            message: "create scripts directory".to_string(),
            path: Some(self.scripts_dir.clone()),
            source: Some(e),
        })?;

        let profile_dir = self.profiles_dir.join(slug);
        fs::create_dir_all(&profile_dir).map_err(|e| PumasError::Io {
            message: "create profile directory".to_string(),
            path: Some(profile_dir.clone()),
            source: Some(e),
        })?;

        let script_path = self.scripts_dir.join(format!("launch-{}.sh", slug));
        let content = self.generate_script_content(tag, version_dir, slug, &profile_dir);

        // Write script
        let mut file = fs::File::create(&script_path).map_err(|e| PumasError::Io {
            message: "create launch script".to_string(),
            path: Some(script_path.clone()),
            source: Some(e),
        })?;

        file.write_all(content.as_bytes()).map_err(|e| PumasError::Io {
            message: "write launch script".to_string(),
            path: Some(script_path.clone()),
            source: Some(e),
        })?;

        // Make executable
        let metadata = fs::metadata(&script_path).map_err(|e| PumasError::Io {
            message: "get script metadata".to_string(),
            path: Some(script_path.clone()),
            source: Some(e),
        })?;

        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);

        fs::set_permissions(&script_path, permissions).map_err(|e| PumasError::Io {
            message: "set script permissions".to_string(),
            path: Some(script_path.clone()),
            source: Some(e),
        })?;

        debug!("Generated launch script at {:?}", script_path);

        Ok(script_path)
    }

    /// Generate the bash script content.
    fn generate_script_content(
        &self,
        tag: &str,
        version_dir: &Path,
        slug: &str,
        profile_dir: &Path,
    ) -> String {
        let version_dir_str = version_dir.display();
        let profile_dir_str = profile_dir.display();

        format!(
            r#"#!/bin/bash
set -euo pipefail

VERSION_DIR="{version_dir_str}"
VENV_PATH="$VERSION_DIR/venv"
MAIN_PY="$VERSION_DIR/main.py"
PID_FILE="$VERSION_DIR/comfyui.pid"
URL="http://127.0.0.1:8188"
WINDOW_CLASS="ComfyUI-{slug}"
PROFILE_DIR="{profile_dir_str}"
SERVER_START_DELAY={delay}
SERVER_PID=""

log() {{
    echo "[\$(date +'%H:%M:%S')] $*"
}}

stop_previous_instance() {{
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || echo "")
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            log "Stopping previous server (PID: $pid)..."
            kill "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        fi
        rm -f "$PID_FILE"
    fi
}}

close_existing_app_window() {{
    if command -v wmctrl >/dev/null 2>&1; then
        local wins
        wins=$(wmctrl -l -x 2>/dev/null | grep -i "$WINDOW_CLASS" | awk '{{print $1}}' || true)
        if [[ -n "$wins" ]]; then
            for win_id in $wins; do
                wmctrl -i -c "$win_id" || true
            done
            sleep 1
        fi
    fi
}}

start_comfyui() {{
    if [[ ! -x "$VENV_PATH/bin/python" ]]; then
        echo "Missing virtual environment for {tag}"
        exit 1
    fi

    cd "$VERSION_DIR"
    log "Starting ComfyUI {tag}..."
    "$VENV_PATH/bin/python" "$MAIN_PY" --enable-manager &
    SERVER_PID=$!
    echo "$SERVER_PID" > "$PID_FILE"
}}

open_app() {{
    if command -v brave-browser >/dev/null 2>&1; then
        mkdir -p "$PROFILE_DIR"
        log "Opening Brave window for {tag}..."
        brave-browser --app="$URL" --new-window --user-data-dir="$PROFILE_DIR" --class="$WINDOW_CLASS" >/dev/null 2>&1 &
    else
        log "Opening default browser..."
        xdg-open "$URL" >/dev/null 2>&1 &
    fi
}}

cleanup() {{
    if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
}}

trap cleanup EXIT

stop_previous_instance
close_existing_app_window
start_comfyui

log "Waiting $SERVER_START_DELAY seconds for server to start..."
sleep "$SERVER_START_DELAY"
open_app

wait $SERVER_PID
"#,
            version_dir_str = version_dir_str,
            slug = slug,
            profile_dir_str = profile_dir_str,
            delay = self.server_start_delay,
            tag = tag,
        )
    }

    /// Remove a launch script.
    pub fn remove(&self, slug: &str) -> Result<()> {
        let script_path = self.scripts_dir.join(format!("launch-{}.sh", slug));

        if script_path.exists() {
            fs::remove_file(&script_path).map_err(|e| PumasError::Io {
                message: "remove launch script".to_string(),
                path: Some(script_path.to_path_buf()),
                source: Some(e),
            })?;
        }

        Ok(())
    }

    /// Get the path to a launch script.
    pub fn script_path(&self, slug: &str) -> PathBuf {
        self.scripts_dir.join(format!("launch-{}.sh", slug))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_script() {
        let temp_dir = TempDir::new().unwrap();
        let scripts_dir = temp_dir.path().join("scripts");
        let profiles_dir = temp_dir.path().join("profiles");
        let version_dir = temp_dir.path().join("versions").join("v1.0.0");

        fs::create_dir_all(&version_dir).unwrap();

        let generator = LaunchScriptGenerator::new(&scripts_dir, &profiles_dir)
            .with_server_start_delay(3);

        let script_path = generator.generate("v1.0.0", &version_dir, "v1-0-0").unwrap();

        assert!(script_path.exists());

        let content = fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("#!/bin/bash"));
        assert!(content.contains("ComfyUI v1.0.0"));
        assert!(content.contains("SERVER_START_DELAY=3"));

        // Check permissions
        let metadata = fs::metadata(&script_path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o755, 0o755);
    }

    #[test]
    fn test_remove_script() {
        let temp_dir = TempDir::new().unwrap();
        let scripts_dir = temp_dir.path().join("scripts");
        let profiles_dir = temp_dir.path().join("profiles");
        let version_dir = temp_dir.path().join("versions").join("v1.0.0");

        fs::create_dir_all(&version_dir).unwrap();

        let generator = LaunchScriptGenerator::new(&scripts_dir, &profiles_dir);
        let script_path = generator.generate("v1.0.0", &version_dir, "v1-0-0").unwrap();

        assert!(script_path.exists());

        generator.remove("v1-0-0").unwrap();

        assert!(!script_path.exists());
    }
}
