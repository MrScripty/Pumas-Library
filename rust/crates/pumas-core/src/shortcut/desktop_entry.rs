//! Desktop entry (.desktop file) generation.
//!
//! Implements the XDG Desktop Entry Specification.

use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::error::{PumasError, Result};
use tracing::debug;

/// A desktop entry representation.
#[derive(Debug, Clone)]
pub struct DesktopEntry {
    /// Entry name (shown in menus).
    pub name: String,
    /// Comment/description.
    pub comment: Option<String>,
    /// Executable command.
    pub exec: String,
    /// Icon name or path.
    pub icon: String,
    /// Whether to run in a terminal.
    pub terminal: bool,
    /// Entry type (usually "Application").
    pub entry_type: String,
    /// Categories (semicolon-separated).
    pub categories: Vec<String>,
    /// Keywords for search.
    pub keywords: Vec<String>,
    /// Whether this is a hidden entry.
    pub hidden: bool,
    /// Whether this entry should not be displayed.
    pub no_display: bool,
    /// StartupWMClass for window matching.
    pub startup_wm_class: Option<String>,
}

impl Default for DesktopEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            comment: None,
            exec: String::new(),
            icon: String::new(),
            terminal: false,
            entry_type: "Application".to_string(),
            categories: vec!["Graphics".to_string(), "ArtificialIntelligence".to_string()],
            keywords: Vec::new(),
            hidden: false,
            no_display: false,
            startup_wm_class: None,
        }
    }
}

impl DesktopEntry {
    /// Create a new desktop entry builder.
    pub fn builder() -> DesktopEntryBuilder {
        DesktopEntryBuilder::new()
    }

    /// Generate the .desktop file content.
    pub fn to_string(&self) -> String {
        let mut content = String::new();

        writeln!(content, "[Desktop Entry]").unwrap();
        writeln!(content, "Name={}", self.name).unwrap();

        if let Some(ref comment) = self.comment {
            writeln!(content, "Comment={}", comment).unwrap();
        }

        writeln!(content, "Exec={}", self.exec).unwrap();
        writeln!(content, "Icon={}", self.icon).unwrap();
        writeln!(content, "Terminal={}", if self.terminal { "true" } else { "false" }).unwrap();
        writeln!(content, "Type={}", self.entry_type).unwrap();

        if !self.categories.is_empty() {
            writeln!(content, "Categories={};", self.categories.join(";")).unwrap();
        }

        if !self.keywords.is_empty() {
            writeln!(content, "Keywords={};", self.keywords.join(";")).unwrap();
        }

        if self.hidden {
            writeln!(content, "Hidden=true").unwrap();
        }

        if self.no_display {
            writeln!(content, "NoDisplay=true").unwrap();
        }

        if let Some(ref wm_class) = self.startup_wm_class {
            writeln!(content, "StartupWMClass={}", wm_class).unwrap();
        }

        content
    }

    /// Write the desktop entry to a file.
    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                message: "create directory".to_string(),
                path: Some(parent.to_path_buf()),
                source: Some(e),
            })?;
        }

        // Write content
        let content = self.to_string();
        let mut file = fs::File::create(path).map_err(|e| PumasError::Io {
            message: "create desktop file".to_string(),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;

        file.write_all(content.as_bytes()).map_err(|e| PumasError::Io {
            message: "write desktop file".to_string(),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;

        // Make executable (required for desktop files to be trusted)
        let metadata = fs::metadata(path).map_err(|e| PumasError::Io {
            message: "get file metadata".to_string(),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;

        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);

        fs::set_permissions(path, permissions).map_err(|e| PumasError::Io {
            message: "set permissions".to_string(),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;

        debug!("Wrote desktop entry to {:?}", path);

        Ok(())
    }
}

/// Builder for desktop entries.
pub struct DesktopEntryBuilder {
    entry: DesktopEntry,
}

impl DesktopEntryBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            entry: DesktopEntry::default(),
        }
    }

    /// Set the entry name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.entry.name = name.into();
        self
    }

    /// Set the comment.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.entry.comment = Some(comment.into());
        self
    }

    /// Set the executable command.
    pub fn exec(mut self, exec: impl Into<String>) -> Self {
        self.entry.exec = exec.into();
        self
    }

    /// Set the icon.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.entry.icon = icon.into();
        self
    }

    /// Set whether to run in terminal.
    pub fn terminal(mut self, terminal: bool) -> Self {
        self.entry.terminal = terminal;
        self
    }

    /// Set the entry type.
    pub fn entry_type(mut self, entry_type: impl Into<String>) -> Self {
        self.entry.entry_type = entry_type.into();
        self
    }

    /// Set categories.
    pub fn categories(mut self, categories: Vec<String>) -> Self {
        self.entry.categories = categories;
        self
    }

    /// Add a category.
    pub fn add_category(mut self, category: impl Into<String>) -> Self {
        self.entry.categories.push(category.into());
        self
    }

    /// Set keywords.
    pub fn keywords(mut self, keywords: Vec<String>) -> Self {
        self.entry.keywords = keywords;
        self
    }

    /// Set whether hidden.
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.entry.hidden = hidden;
        self
    }

    /// Set whether to not display.
    pub fn no_display(mut self, no_display: bool) -> Self {
        self.entry.no_display = no_display;
        self
    }

    /// Set the StartupWMClass.
    pub fn startup_wm_class(mut self, wm_class: impl Into<String>) -> Self {
        self.entry.startup_wm_class = Some(wm_class.into());
        self
    }

    /// Build the desktop entry.
    pub fn build(self) -> DesktopEntry {
        self.entry
    }
}

impl Default for DesktopEntryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_desktop_entry_builder() {
        let entry = DesktopEntry::builder()
            .name("My App")
            .comment("A test application")
            .exec("/usr/bin/myapp")
            .icon("myapp")
            .terminal(false)
            .add_category("Utility")
            .build();

        assert_eq!(entry.name, "My App");
        assert_eq!(entry.comment, Some("A test application".to_string()));
        assert_eq!(entry.exec, "/usr/bin/myapp");
        assert_eq!(entry.icon, "myapp");
        assert!(!entry.terminal);
        assert!(entry.categories.contains(&"Utility".to_string()));
    }

    #[test]
    fn test_desktop_entry_to_string() {
        let entry = DesktopEntry::builder()
            .name("Test App")
            .exec("/bin/test")
            .icon("test-icon")
            .build();

        let content = entry.to_string();

        assert!(content.contains("[Desktop Entry]"));
        assert!(content.contains("Name=Test App"));
        assert!(content.contains("Exec=/bin/test"));
        assert!(content.contains("Icon=test-icon"));
        assert!(content.contains("Type=Application"));
    }

    #[test]
    fn test_write_desktop_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.desktop");

        let entry = DesktopEntry::builder()
            .name("Test")
            .exec("/bin/test")
            .icon("test")
            .build();

        entry.write_to_file(&file_path).unwrap();

        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("Name=Test"));

        // Check permissions
        let metadata = fs::metadata(&file_path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o755, 0o755);
    }
}
