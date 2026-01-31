//! Process detection for ComfyUI and other managed applications.

use crate::platform;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// How the process was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSource {
    /// Detected via PID file.
    PidFile,
    /// Detected via process table scan.
    ProcessScan,
}

/// Information about a detected process.
#[derive(Debug, Clone)]
pub struct DetectedProcess {
    /// Process ID.
    pub pid: u32,
    /// How the process was detected.
    pub source: ProcessSource,
    /// Version tag (if identified).
    pub tag: Option<String>,
    /// Path to the PID file (if detected via PID file).
    pub pid_file: Option<PathBuf>,
    /// Command line (if detected via process scan).
    pub cmdline: Option<String>,
}

/// Process detector for managed applications.
pub struct ProcessDetector {
    /// Root directory (e.g., ComfyUI root or launcher root).
    root_dir: PathBuf,
    /// Known version paths: tag -> version directory.
    version_paths: HashMap<String, PathBuf>,
}

impl ProcessDetector {
    /// Create a new process detector.
    ///
    /// # Arguments
    ///
    /// * `root_dir` - Root directory for the application
    /// * `version_paths` - Map of version tags to their installation directories
    pub fn new(root_dir: impl AsRef<Path>, version_paths: HashMap<String, PathBuf>) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
            version_paths,
        }
    }

    /// Update the known version paths.
    pub fn set_version_paths(&mut self, version_paths: HashMap<String, PathBuf>) {
        self.version_paths = version_paths;
    }

    /// Detect all running ComfyUI processes.
    ///
    /// Uses multiple detection methods and deduplicates by PID.
    pub fn detect_processes(&self) -> Vec<DetectedProcess> {
        let mut processes = Vec::new();
        let mut seen_pids = HashSet::new();

        // 1. Check PID files
        self.detect_from_pid_files(&mut processes, &mut seen_pids);

        // 2. Scan process table
        self.detect_from_process_scan(&mut processes, &mut seen_pids);

        debug!(
            "detect_processes: found {} processes via PID files and process scan",
            processes.len()
        );

        processes
    }

    /// Check if any managed process is running.
    pub fn is_any_running(&self) -> bool {
        !self.detect_processes().is_empty()
    }

    /// Detect processes from PID files.
    fn detect_from_pid_files(
        &self,
        processes: &mut Vec<DetectedProcess>,
        seen_pids: &mut HashSet<u32>,
    ) {
        // Check root-level PID file (legacy)
        let root_pid_file = self.root_dir.join("comfyui.pid");
        self.check_pid_file(&root_pid_file, None, processes, seen_pids);

        // Check per-version PID files
        for (tag, version_path) in &self.version_paths {
            let pid_file = version_path.join("comfyui.pid");
            self.check_pid_file(&pid_file, Some(tag.clone()), processes, seen_pids);
        }
    }

    /// Check a single PID file.
    fn check_pid_file(
        &self,
        pid_file: &Path,
        tag: Option<String>,
        processes: &mut Vec<DetectedProcess>,
        seen_pids: &mut HashSet<u32>,
    ) {
        if !pid_file.exists() {
            return;
        }

        // Read PID from file
        let pid_str = match fs::read_to_string(pid_file) {
            Ok(s) => s,
            Err(e) => {
                debug!("Failed to read PID file {:?}: {}", pid_file, e);
                return;
            }
        };

        let pid: u32 = match pid_str.trim().parse() {
            Ok(p) => p,
            Err(e) => {
                debug!("Invalid PID in {:?}: {}", pid_file, e);
                return;
            }
        };

        // Check if process is alive
        if !self.is_process_alive(pid) {
            debug!(
                "Stale PID file {:?}: process {} not running",
                pid_file, pid
            );
            return;
        }

        if seen_pids.insert(pid) {
            processes.push(DetectedProcess {
                pid,
                source: ProcessSource::PidFile,
                tag,
                pid_file: Some(pid_file.to_path_buf()),
                cmdline: None,
            });
        }
    }

    /// Detect processes by scanning the process table.
    /// Uses the centralized platform module for cross-platform process scanning.
    fn detect_from_process_scan(
        &self,
        processes: &mut Vec<DetectedProcess>,
        seen_pids: &mut HashSet<u32>,
    ) {
        // Use platform module to find processes matching ComfyUI patterns
        let comfyui_processes = platform::process::find_processes_by_cmdline("comfyui");

        for (pid, cmdline) in comfyui_processes {
            if seen_pids.contains(&pid) {
                continue;
            }

            let cmdline_lower = cmdline.to_lowercase();

            // Additional validation: check for specific ComfyUI indicators
            let is_comfyui = cmdline_lower.contains("comfyui server")
                || (cmdline.contains("main.py") && cmdline_lower.contains("comfyui"));

            if !is_comfyui {
                continue;
            }

            // Try to infer version tag from command line
            let inferred_tag = self.infer_tag_from_cmdline(&cmdline);

            seen_pids.insert(pid);
            processes.push(DetectedProcess {
                pid,
                source: ProcessSource::ProcessScan,
                tag: inferred_tag,
                pid_file: None,
                cmdline: Some(cmdline),
            });
        }
    }

    /// Try to infer the version tag from a command line.
    fn infer_tag_from_cmdline(&self, cmdline: &str) -> Option<String> {
        for (tag, path) in &self.version_paths {
            let path_str = path.to_string_lossy();
            if cmdline.contains(path_str.as_ref()) {
                return Some(tag.clone());
            }
        }
        None
    }

    /// Check if a process is alive.
    /// Uses the centralized platform module for cross-platform implementation.
    fn is_process_alive(&self, pid: u32) -> bool {
        platform::is_process_alive(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_process_detector_creation() {
        let temp_dir = TempDir::new().unwrap();
        let detector = ProcessDetector::new(temp_dir.path(), HashMap::new());

        // Should detect nothing in an empty directory
        let processes = detector.detect_processes();
        assert!(processes.is_empty());
    }

    #[test]
    fn test_stale_pid_file() {
        let temp_dir = TempDir::new().unwrap();
        let pid_file = temp_dir.path().join("comfyui.pid");

        // Write a PID that definitely doesn't exist
        fs::write(&pid_file, "999999999").unwrap();

        let detector = ProcessDetector::new(temp_dir.path(), HashMap::new());
        let processes = detector.detect_processes();

        // Stale PID file should be ignored
        assert!(processes.is_empty());
    }

    #[test]
    fn test_infer_tag_from_cmdline() {
        let temp_dir = TempDir::new().unwrap();
        let mut version_paths = HashMap::new();
        version_paths.insert(
            "v1.0.0".to_string(),
            temp_dir.path().join("versions").join("v1.0.0"),
        );

        let detector = ProcessDetector::new(temp_dir.path(), version_paths);

        let cmdline = format!(
            "python {}/versions/v1.0.0/main.py",
            temp_dir.path().display()
        );
        let tag = detector.infer_tag_from_cmdline(&cmdline);

        assert_eq!(tag, Some("v1.0.0".to_string()));
    }
}
