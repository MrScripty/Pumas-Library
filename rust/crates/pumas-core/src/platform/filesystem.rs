//! Non-replacing moves for application-owned directory entries.
//!
//! The caller owns path selection and move lifecycle. This boundary prevents
//! replacement of an occupied destination; it does not grant containment,
//! pin parent directories, or make metadata/index updates crash-atomic.
//! Windows owns one synchronous FFI call: local, immutable, NUL-terminated
//! buffers remain alive for the entire call; no pointer or resource escapes.

#![deny(unsafe_code)]

use std::io;
use std::path::Path;

/// Move an entry without replacing any existing destination, including an
/// empty directory or dangling symlink. Unsupported filesystem operations
/// return their error; never fall back to replacing rename or copy/delete.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn rename_directory_noreplace(source: &Path, target: &Path) -> io::Result<()> {
    use rustix::fs::{renameat_with, RenameFlags, CWD};

    renameat_with(CWD, source, CWD, target, RenameFlags::NOREPLACE).map_err(Into::into)
}

#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) fn rename_directory_noreplace(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    fn terminated_path(path: &Path) -> io::Result<Vec<u16>> {
        let mut units: Vec<_> = path.as_os_str().encode_wide().collect();
        if units.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Path contains NUL",
            ));
        }
        units.push(0);
        Ok(units)
    }

    let source = terminated_path(source)?;
    let target = terminated_path(target)?;
    // SAFETY: both pointers address initialized, immutable UTF-16 buffers with
    // exactly one terminal NUL and no embedded NUL. The owned buffers outlive
    // the synchronous call, which retains neither pointer. Flags are zero:
    // replacement, deferred moves, and cross-volume copy/delete are disabled.
    let moved = unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), 0) };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) fn rename_directory_noreplace(_source: &Path, _target: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Non-replacing classification moves are unsupported on this target",
    ))
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos", windows)))]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn moves_to_absent_target_without_changing_payload() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source model");
        let target = root.path().join("target model");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("weights.bin"), b"selected artifact").unwrap();

        rename_directory_noreplace(&source, &target).unwrap();

        assert!(!source.exists());
        assert_eq!(
            fs::read(target.join("weights.bin")).unwrap(),
            b"selected artifact"
        );
    }

    #[test]
    fn refuses_empty_target_created_after_preflight() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("weights.bin"), b"source artifact").unwrap();
        assert!(!target.exists());
        // Another owner can create this after the domain's collision check.
        fs::create_dir(&target).unwrap();

        assert!(rename_directory_noreplace(&source, &target).is_err());

        assert_eq!(
            fs::read(source.join("weights.bin")).unwrap(),
            b"source artifact"
        );
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_dangling_target_symlink_without_following_or_replacing_it() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        let absent = root.path().join("absent");
        fs::create_dir(&source).unwrap();
        symlink(&absent, &target).unwrap();
        assert!(!target.exists());

        assert!(rename_directory_noreplace(&source, &target).is_err());

        assert!(source.is_dir());
        assert_eq!(fs::read_link(&target).unwrap(), absent);
        assert!(!absent.exists());
    }
}
