//! Bounded observations after a single refused open, never a recovery oracle.
use std::path::Path;

pub fn observation(root: &Path) -> String {
    #[cfg(target_os = "linux")]
    {
        use std::io::Read;
        use std::os::unix::fs::MetadataExt;
        let identities: Vec<_> = [root.to_path_buf(), root.join("shared-resources/models")]
            .into_iter()
            .filter_map(|path| {
                std::fs::metadata(&path)
                    .ok()
                    .map(|m| (path, m.dev(), m.ino()))
            })
            .collect();
        let mut descriptors = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
            for entry in entries.take(1024).flatten() {
                let path = entry.path();
                let Ok(metadata) = std::fs::metadata(&path) else {
                    continue;
                };
                let target = std::fs::read_link(&path).ok();
                let root_identity = identities
                    .iter()
                    .any(|(_, dev, ino)| metadata.dev() == *dev && metadata.ino() == *ino);
                if !root_identity && !target.as_ref().is_some_and(|p| p.starts_with(root)) {
                    continue;
                }
                let mut info = String::new();
                if let Ok(file) =
                    std::fs::File::open(Path::new("/proc/self/fdinfo").join(entry.file_name()))
                {
                    let _ = file.take(16 * 1024).read_to_string(&mut info);
                }
                let locks: Vec<_> = info
                    .lines()
                    .filter(|line| line.starts_with("lock:"))
                    .collect();
                descriptors.push(format!("fd={:?} target={target:?} dev={} ino={} root_identity={root_identity} locks={locks:?}", entry.file_name(), metadata.dev(), metadata.ino()));
                if descriptors.len() == 32 {
                    break;
                }
            }
        }
        format!("post-refusal pid={} identities={identities:?} descriptors={descriptors:?}; observation only, no holder/cessation verdict", std::process::id())
    }
    #[cfg(not(target_os = "linux"))]
    {
        format!("post-refusal root={root:?}; descriptor observation unavailable on this platform")
    }
}
