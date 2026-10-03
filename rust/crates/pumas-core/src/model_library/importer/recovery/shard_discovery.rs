//! Read-only shard observations. Paths in the report are labels, never grants.
#![deny(unsafe_code)]

use super::super::{
    IncompleteShardRecovery, ShardModelDiscovery, ShardRecoveryDiagnostic,
    ShardRecoveryDiagnosticKind as Kind, ShardRecoveryDiscovery, ShardSetDiscoveryStatus,
    TEMP_IMPORT_PREFIX,
};
use crate::model_library::download_recovery::{
    directory_identity, open_directory_chain, FilesystemIdentity,
};
use crate::model_library::{normalize_artifact_path_slug, normalize_name};
use crate::platform::capability_fs::open_directory;
use cap_std::fs::{Dir, Metadata};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

mod evidence;
#[cfg(test)]
mod tests;

// Discovery is advisory. Exceeding these observation budgets returns an explicit
// incomplete report, never a smaller "complete" sample or an acquisition request.
const MAX_ENTRIES: usize = 100_000;
const MAX_MODEL_DEPTH: usize = 64;

// Per-scan test instrumentation counts owned HeldDirectory capabilities, not
// process-global descriptors or Arc references to the same capability.
#[cfg(test)]
#[derive(Default)]
struct HeldDirectoryCounts {
    live: std::sync::atomic::AtomicUsize,
    high_water: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl HeldDirectoryCounts {
    fn live(&self) -> usize {
        self.live.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn high_water(&self) -> usize {
        self.high_water.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
struct HeldDirectoryCount {
    counts: Arc<HeldDirectoryCounts>,
}

#[cfg(test)]
impl HeldDirectoryCount {
    fn acquire(counts: Arc<HeldDirectoryCounts>) -> Self {
        use std::sync::atomic::Ordering::Relaxed;
        let live = counts.live.fetch_add(1, Relaxed) + 1;
        counts.high_water.fetch_max(live, Relaxed);
        Self { counts }
    }
}

#[cfg(test)]
impl Drop for HeldDirectoryCount {
    fn drop(&mut self) {
        self.counts
            .live
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

struct HeldDirectory {
    directory: Dir,
    identity: FilesystemIdentity,
    parent: Option<Arc<HeldDirectory>>,
    name: OsString,
    #[cfg(test)]
    handle_count: HeldDirectoryCount,
}

impl HeldDirectory {
    fn anchor(path: &Path) -> io::Result<Arc<Self>> {
        let directory = open_directory(path)?;
        Ok(Arc::new(Self {
            identity: directory_identity(&directory)?,
            directory,
            parent: None,
            name: OsString::new(),
            #[cfg(test)]
            handle_count: HeldDirectoryCount::acquire(Arc::new(HeldDirectoryCounts::default())),
        }))
    }

    fn child(self: &Arc<Self>, name: &std::ffi::OsStr) -> io::Result<Arc<Self>> {
        self.verify()?;
        let directory = open_directory_chain(&self.directory, Path::new(name), false)?;
        let child = Arc::new(Self {
            identity: directory_identity(&directory)?,
            directory,
            parent: Some(self.clone()),
            name: name.to_owned(),
            #[cfg(test)]
            handle_count: HeldDirectoryCount::acquire(self.handle_count.counts.clone()),
        });
        child.verify()?;
        Ok(child)
    }

    fn verify(&self) -> io::Result<()> {
        if let Some(parent) = &self.parent {
            parent.verify()?;
            let current = open_directory_chain(&parent.directory, Path::new(&self.name), false)?;
            if directory_identity(&current)? != self.identity {
                return Err(binding_changed());
            }
        }
        if directory_identity(&self.directory)? != self.identity {
            return Err(binding_changed());
        }
        Ok(())
    }
}

struct RootBinding {
    source: PathBuf,
    canonical: PathBuf,
    held: Arc<HeldDirectory>,
}

impl RootBinding {
    fn open(source: &Path) -> io::Result<Self> {
        // The configured root may use a platform alias (/var on macOS). Bind
        // that explicit selection, then traverse only its canonical no-follow
        // chain; aliases below the selected root are never followed.
        let configured = open_directory(source)?;
        let canonical = std::fs::canonicalize(source)?;
        let mut anchor = PathBuf::new();
        let mut names = Vec::new();
        for component in canonical.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => anchor.push(component.as_os_str()),
                Component::Normal(name) => names.push(name.to_owned()),
                _ => return Err(binding_changed()),
            }
        }
        let mut held = HeldDirectory::anchor(&anchor)?;
        for name in names {
            held = held.child(&name)?;
        }
        if directory_identity(&configured)? != held.identity {
            return Err(binding_changed());
        }
        let root = Self {
            source: source.to_owned(),
            canonical,
            held,
        };
        root.verify()?;
        Ok(root)
    }

    fn verify(&self) -> io::Result<()> {
        self.held.verify()?;
        if std::fs::canonicalize(&self.source)? != self.canonical
            || directory_identity(&open_directory(&self.source)?)? != self.held.identity
        {
            return Err(binding_changed());
        }
        Ok(())
    }
}

fn binding_changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "directory binding changed during discovery",
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    RootBound,
    DirectoryBound,
    BeforeEnumeration,
    EnumerationEntry,
    AfterEnumeration,
    BeforeFinish,
}

/// An observation to recheck, never a retained directory capability. Completed
/// subtrees must release their handles before unrelated traversal continues.
struct ObservedBinding {
    identity: FilesystemIdentity,
    relative: PathBuf,
}

struct Scanner<F> {
    root: RootBinding,
    report: ShardRecoveryDiscovery,
    remaining_entries: usize,
    bindings: Vec<ObservedBinding>,
    // Private synchronous fault seam; production supplies a no-op. No global
    // state, additional scan path, or filesystem behavior is substituted.
    checkpoint: F,
}

impl<F: FnMut(&Path, Phase) -> io::Result<()>> Scanner<F> {
    fn checkpoint(&mut self, path: &Path, phase: Phase) -> io::Result<()> {
        (self.checkpoint)(path, phase)
    }

    fn diagnostic(&mut self, path: &Path, kind: Kind, message: impl ToString, incomplete: bool) {
        if incomplete {
            self.report.enumeration_complete = false;
        }
        self.report.diagnostics.push(ShardRecoveryDiagnostic {
            path: path.to_owned(),
            kind,
            message: message.to_string(),
        });
    }

    fn verify(&mut self, held: &HeldDirectory, path: &Path) -> bool {
        match self.root.verify().and_then(|()| held.verify()) {
            Ok(()) => true,
            Err(error) => {
                self.diagnostic(path, Kind::BindingChanged, error, true);
                false
            }
        }
    }

    fn verify_observed(&mut self, observed: &ObservedBinding) -> bool {
        let result = self.root.verify().and_then(|()| {
            // Reopen one saved path at a time from the held root. The existing
            // helper refuses symlinks and releases each intermediate handle;
            // neither tree width nor visited-directory count retains handles.
            let current =
                open_directory_chain(&self.root.held.directory, &observed.relative, false)?;
            if directory_identity(&current)? != observed.identity {
                return Err(binding_changed());
            }
            // Do not overlap this reopened directory with the root verifier's
            // temporary parent/child handles or retain it for the next binding.
            drop(current);
            self.root.verify()
        });
        match result {
            Ok(()) => true,
            Err(error) => {
                self.diagnostic(&observed.relative, Kind::BindingChanged, error, true);
                false
            }
        }
    }

    fn entries(
        &mut self,
        held: &HeldDirectory,
        relative: &Path,
    ) -> Option<Vec<(OsString, Metadata)>> {
        if !self.verify(held, relative) {
            return None;
        }
        let reader = self
            .checkpoint(relative, Phase::BeforeEnumeration)
            .and_then(|()| held.directory.entries());
        let reader = match reader {
            Ok(reader) => reader,
            Err(error) => {
                self.diagnostic(relative, Kind::EnumerationFailed, error, true);
                return None;
            }
        };
        let mut entries = Vec::new();
        for entry in reader {
            if self.remaining_entries == 0 {
                self.diagnostic(
                    relative,
                    Kind::ResourceLimit,
                    "discovery entry budget exhausted",
                    true,
                );
                return None;
            }
            self.remaining_entries -= 1;
            let entry = self
                .checkpoint(relative, Phase::EnumerationEntry)
                .and(entry);
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    self.diagnostic(relative, Kind::EnumerationFailed, error, true);
                    return None;
                }
            };
            let name = entry.file_name();
            match held.directory.symlink_metadata(&name) {
                Ok(metadata) => entries.push((name, metadata)),
                Err(error) => {
                    self.diagnostic(&relative.join(name), Kind::MetadataFailed, error, true);
                    return None;
                }
            }
        }
        if let Err(error) = self.checkpoint(relative, Phase::AfterEnumeration) {
            self.diagnostic(relative, Kind::EnumerationFailed, error, true);
            return None;
        }
        if !self.verify(held, relative) {
            return None;
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Some(entries)
    }

    fn child(
        &mut self,
        parent: &Arc<HeldDirectory>,
        name: &std::ffi::OsStr,
        path: &Path,
    ) -> Option<Arc<HeldDirectory>> {
        let child = match parent.child(name) {
            Ok(child) => child,
            Err(error) => {
                self.diagnostic(path, Kind::BindingChanged, error, true);
                return None;
            }
        };
        if let Err(error) = self.checkpoint(path, Phase::DirectoryBound) {
            self.diagnostic(path, Kind::EnumerationFailed, error, true);
            return None;
        }
        if !self.verify(&child, path) {
            return None;
        }
        self.bindings.push(ObservedBinding {
            identity: child.identity,
            relative: path.to_owned(),
        });
        Some(child)
    }

    fn layout(&mut self, held: Arc<HeldDirectory>, relative: &Path, depth: usize) {
        let Some(entries) = self.entries(&held, relative) else {
            return;
        };
        // Weight/index/metadata evidence above depth three makes this branch a
        // legacy or ambiguous package. Never reinterpret its descendants as repos.
        if entries
            .iter()
            .any(|(name, _)| name.to_str().is_some_and(evidence::is_model_evidence))
        {
            self.diagnostic(
                relative,
                Kind::NonCanonicalLayout,
                "model evidence outside a canonical three-component model root; branch not scanned",
                true,
            );
            return;
        }
        for (name, metadata) in entries {
            let path = relative.join(&name);
            if metadata.is_symlink() {
                self.diagnostic(&path, Kind::SymlinkRefused, "symlink not traversed", true);
                continue;
            }
            if !metadata.is_dir() {
                if name.to_str().is_none() {
                    self.diagnostic(
                        &path,
                        Kind::NonCanonicalLayout,
                        "non-UTF-8 layout entry cannot be classified",
                        true,
                    );
                }
                continue;
            }
            let Some(text) = name.to_str() else {
                self.diagnostic(
                    &path,
                    Kind::NonCanonicalLayout,
                    "non-UTF-8 layout component",
                    true,
                );
                continue;
            };
            // Import staging has its own owner and is outside canonical models.
            if text.starts_with(TEMP_IMPORT_PREFIX) {
                continue;
            }
            let canonical = if depth == 2 {
                normalize_artifact_path_slug(text)
            } else {
                normalize_name(text)
            };
            if canonical != text {
                self.diagnostic(
                    &path,
                    Kind::NonCanonicalLayout,
                    "component does not match canonical storage naming",
                    true,
                );
                continue;
            }
            let Some(child) = self.child(&held, &name, &path) else {
                continue;
            };
            if depth == 2 {
                self.model(child, &path);
            } else {
                self.layout(child, &path, depth + 1);
            }
        }
        self.verify(&held, relative);
    }

    fn model(&mut self, held: Arc<HeldDirectory>, relative: &Path) {
        let mut model = ShardModelDiscovery {
            model_dir: self.root.canonical.join(relative),
            library_relative_root: relative.to_owned(),
            has_metadata: false,
            enumeration_complete: true,
            observed_files: Vec::new(),
            shard_sets: Vec::new(),
            indexes: Vec::new(),
        };
        let mut files = BTreeSet::new();
        self.model_files(
            held.clone(),
            relative,
            Path::new(""),
            0,
            &mut model,
            &mut files,
        );
        if !self.verify(&held, relative) {
            model.enumeration_complete = false;
            files.clear();
            model.indexes.clear();
        }
        model.observed_files = files.iter().cloned().collect();
        evidence::analyze(&mut model, &files, &mut self.report.diagnostics);
        self.report.model_roots.push(model);
    }

    fn model_files(
        &mut self,
        held: Arc<HeldDirectory>,
        root: &Path,
        directory: &Path,
        depth: usize,
        model: &mut ShardModelDiscovery,
        files: &mut BTreeSet<PathBuf>,
    ) {
        let relative = root.join(directory);
        if depth > MAX_MODEL_DEPTH {
            self.diagnostic(
                &relative,
                Kind::ResourceLimit,
                "model directory depth budget exhausted",
                true,
            );
            model.enumeration_complete = false;
            return;
        }
        let Some(entries) = self.entries(&held, &relative) else {
            model.enumeration_complete = false;
            return;
        };
        for (name, metadata) in entries {
            let file_relative = directory.join(&name);
            let path = root.join(&file_relative);
            if metadata.is_symlink() {
                self.diagnostic(&path, Kind::SymlinkRefused, "symlink not inspected", true);
                model.enumeration_complete = false;
            } else if metadata.is_dir() {
                match self.child(&held, &name, &path) {
                    Some(child) => {
                        self.model_files(child, root, &file_relative, depth + 1, model, files)
                    }
                    None => model.enumeration_complete = false,
                }
            } else if metadata.is_file() {
                let Some(text) = name.to_str() else {
                    self.diagnostic(
                        &path,
                        Kind::InvalidShardName,
                        "non-UTF-8 filename cannot be classified",
                        true,
                    );
                    model.enumeration_complete = false;
                    continue;
                };
                if directory.as_os_str().is_empty() && text == "metadata.json" {
                    model.has_metadata = true;
                } else if !directory.as_os_str().is_empty()
                    && (text == "metadata.json" || text == ".pumas_download")
                {
                    self.diagnostic(
                        &path,
                        Kind::NonCanonicalLayout,
                        "nested package boundary is ambiguous",
                        true,
                    );
                    model.enumeration_complete = false;
                }
                files.insert(file_relative.clone());
                if evidence::is_weight_index(text) {
                    if !self.verify(&held, &relative) {
                        model.enumeration_complete = false;
                        continue;
                    }
                    let index = evidence::read_index(&held.directory, &file_relative, &metadata);
                    match index {
                        Ok(index) => model.indexes.push(index),
                        Err(error) => {
                            self.diagnostic(&path, Kind::InvalidShardIndex, error, true);
                            model.enumeration_complete = false;
                            model.indexes.push(evidence::invalid_index(file_relative));
                        }
                    }
                }
            } else {
                self.diagnostic(
                    &path,
                    Kind::MetadataFailed,
                    "non-regular entry not inspected",
                    true,
                );
                model.enumeration_complete = false;
            }
        }
        if !self.verify(&held, &relative) {
            model.enumeration_complete = false;
            // Observations through a renamed/replaced child are not current evidence.
            files.retain(|file| !file.starts_with(directory));
            model
                .indexes
                .retain(|index| !index.relative_path.starts_with(directory));
        }
    }
}

fn empty_report(root: &Path) -> ShardRecoveryDiscovery {
    ShardRecoveryDiscovery {
        library_root: root.to_owned(),
        enumeration_complete: true,
        model_roots: Vec::new(),
        diagnostics: Vec::new(),
    }
}

pub(super) fn worker_failed(root: &Path, error: impl ToString) -> ShardRecoveryDiscovery {
    let mut report = empty_report(root);
    report.enumeration_complete = false;
    report.diagnostics.push(ShardRecoveryDiagnostic {
        path: root.to_owned(),
        kind: Kind::WorkerFailed,
        message: error.to_string(),
    });
    report
}

pub(super) fn discover(root: &Path) -> ShardRecoveryDiscovery {
    scan(root, |_, _| Ok(()))
}

fn scan(
    root: &Path,
    checkpoint: impl FnMut(&Path, Phase) -> io::Result<()>,
) -> ShardRecoveryDiscovery {
    let binding = match RootBinding::open(root) {
        Ok(binding) => binding,
        Err(error) => {
            let mut report = empty_report(root);
            report.enumeration_complete = false;
            report.diagnostics.push(ShardRecoveryDiagnostic {
                path: root.to_owned(),
                kind: Kind::BindingChanged,
                message: error.to_string(),
            });
            return report;
        }
    };
    scan_bound(binding, checkpoint)
}

fn scan_bound(
    binding: RootBinding,
    checkpoint: impl FnMut(&Path, Phase) -> io::Result<()>,
) -> ShardRecoveryDiscovery {
    let mut scanner = Scanner {
        report: empty_report(&binding.canonical),
        root: binding,
        remaining_entries: MAX_ENTRIES,
        bindings: Vec::new(),
        checkpoint,
    };
    if let Err(error) = scanner.checkpoint(Path::new(""), Phase::RootBound) {
        scanner.diagnostic(Path::new(""), Kind::EnumerationFailed, error, true);
    } else {
        scanner.layout(scanner.root.held.clone(), Path::new(""), 0);
    }
    if let Err(error) = scanner.checkpoint(Path::new(""), Phase::BeforeFinish) {
        scanner.diagnostic(Path::new(""), Kind::EnumerationFailed, error, true);
    }
    for observed in std::mem::take(&mut scanner.bindings) {
        let path = &observed.relative;
        if !scanner.verify_observed(&observed) {
            for model in &mut scanner.report.model_roots {
                if path.starts_with(&model.library_relative_root)
                    || model.library_relative_root.starts_with(path)
                {
                    model.enumeration_complete = false;
                    model.observed_files.clear();
                    model.shard_sets.clear();
                    model.indexes.clear();
                }
            }
        }
    }
    if !scanner.verify(&scanner.root.held.clone(), Path::new("")) {
        scanner.report.model_roots.clear();
    }
    scanner.report
}

pub(super) fn legacy_projection(report: ShardRecoveryDiscovery) -> Vec<IncompleteShardRecovery> {
    if !report.enumeration_complete {
        tracing::warn!("Legacy shard discovery is incomplete; its Vec cannot establish completeness or authorization");
    }
    for diagnostic in &report.diagnostics {
        tracing::warn!(path = %diagnostic.path.display(), kind = ?diagnostic.kind,
            "Legacy shard discovery: {}", diagnostic.message);
    }
    let diagnostics = &report.diagnostics;
    report
        .model_roots
        .into_iter()
        .filter_map(|model| {
            if model.has_metadata
                || diagnostics.iter().any(|diagnostic| {
                    diagnostic.path.starts_with(&model.library_relative_root)
                        && !matches!(
                            diagnostic.kind,
                            Kind::MissingShardIndex | Kind::MissingIndexedFile
                        )
                })
                || !model.enumeration_complete
                || model
                    .shard_sets
                    .iter()
                    .any(|set| set.status == ShardSetDiscoveryStatus::Ambiguous)
                || !model
                    .shard_sets
                    .iter()
                    .any(|set| set.status == ShardSetDiscoveryStatus::MissingOrdinals)
            {
                return None;
            }
            let parts: Vec<_> = model
                .library_relative_root
                .iter()
                .map(|part| part.to_str())
                .collect::<Option<_>>()?;
            let [model_type, family, name] = parts.as_slice() else {
                return None;
            };
            let official_name = name.replace('_', " ");
            Some(IncompleteShardRecovery {
                model_dir: model.model_dir,
                repo_id: format!("{family}/{official_name}"),
                family: (*family).to_owned(),
                official_name,
                model_type: Some((*model_type).to_owned()),
                existing_files: model
                    .observed_files
                    .into_iter()
                    .filter(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(evidence::is_weight)
                    })
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
            })
        })
        .collect()
}
