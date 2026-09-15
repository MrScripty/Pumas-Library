//! Native operations on held filesystem capabilities.
#![deny(unsafe_code)]

use cap_std::fs::Dir;
use std::fs::File;
use std::io;

/// Disambiguate Windows' missing-path error from a file used as a parent.
/// Call only after an operation reports a missing path.
pub(crate) fn check_missing_path(path: &std::path::Path) -> io::Result<()> {
    #[cfg(windows)]
    for parent in path
        .ancestors()
        .skip(1)
        .filter(|p| !p.as_os_str().is_empty())
    {
        match std::fs::metadata(parent) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "path parent is not a directory",
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        }
    }
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

/// Open a directory while allowing its name to change; callers validate its
/// physical identity before publishing through the held capability.
pub(crate) fn open_directory(path: &std::path::Path) -> io::Result<Dir> {
    #[cfg(not(windows))]
    {
        Dir::open_ambient_dir(path, cap_std::ambient_authority())
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)?;
        if !file.metadata()?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "expected a directory",
            ));
        }
        Ok(Dir::from_std_file(file))
    }
}

#[cfg(windows)]
pub(crate) fn open_directory_nofollow(parent: &Dir, name: &std::ffi::OsStr) -> io::Result<Dir> {
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    };
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    let file = parent.open_with(name, &options)?.into_std();
    let metadata = file.metadata()?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "expected a directory without a reparse point",
        ));
    }
    Ok(Dir::from_std_file(file))
}

/// Flush directory metadata through a handle with the rights the OS requires.
#[cfg(unix)]
pub(crate) fn sync_directory(directory: &Dir) -> io::Result<()> {
    directory.open(".")?.sync_all()
}

#[cfg(windows)]
pub(crate) fn sync_directory(directory: &Dir) -> io::Result<()> {
    sync_directory_file(&directory.try_clone()?.into_std_file())
}

#[cfg(unix)]
pub(crate) fn sync_directory_file(file: &File) -> io::Result<()> {
    file.sync_all()
}

#[cfg(windows)]
pub(crate) fn sync_directory_file(file: &File) -> io::Result<()> {
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
    let directory = Dir::from_std_file(file.try_clone()?);
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    directory.open_with(".", &options)?.sync_all()
}

/// Rename one basename relative to held parents, without resolving ambient paths.
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) fn rename_at(
    source_parent: &Dir,
    source_name: &std::ffi::OsStr,
    target_parent: &Dir,
    target_name: &std::ffi::OsStr,
    replace: bool,
) -> io::Result<()> {
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
    use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
    use windows_sys::Wdk::Storage::FileSystem::{
        FileRenameInformation, NtSetInformationFile, FILE_RENAME_INFORMATION,
    };
    use windows_sys::Win32::Foundation::RtlNtStatusToDosError;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
    };
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
    for name in [source_name, target_name] {
        let mut components = std::path::Path::new(name).components();
        if !matches!(components.next(), Some(std::path::Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "rename requires a basename",
            ));
        }
    }
    let mut options = OpenOptions::new();
    options
        .access_mode(DELETE | FILE_GENERIC_READ)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    let source = source_parent.open_with(source_name, &options)?.into_std();
    if source.metadata()?.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "rename source is a reparse point",
        ));
    }
    let target_parent = target_parent.try_clone()?.into_std_file();
    let name: Vec<u16> = target_name.encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "rename name contains NUL",
        ));
    }
    let offset = std::mem::offset_of!(FILE_RENAME_INFORMATION, FileName);
    let bytes = offset
        .checked_add(name.len() * 2)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "rename name too long"))?;
    // Word storage gives FILE_RENAME_INFORMATION its required pointer alignment and
    // enough initialized space for the variable-length UTF-16 tail.
    let mut storage = vec![0usize; (bytes as usize).div_ceil(std::mem::size_of::<usize>())];
    let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
    // SAFETY: storage is aligned, initialized, and sized for the header and name.
    // Both handles and all pointers remain live for this synchronous call.
    let result = unsafe {
        let mut status: IO_STATUS_BLOCK = std::mem::zeroed();
        (*info).Anonymous.ReplaceIfExists = u8::from(replace);
        (*info).RootDirectory = target_parent.as_raw_handle();
        (*info).FileNameLength = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(name.as_ptr(), (*info).FileName.as_mut_ptr(), name.len());
        NtSetInformationFile(
            source.as_raw_handle(),
            &mut status,
            info.cast(),
            bytes,
            FileRenameInformation,
        )
    };
    if result < 0 {
        // SAFETY: this pure conversion accepts any NTSTATUS value.
        Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(result) } as i32,
        ))
    } else {
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_flushes_held_directory() {
        let temp = tempfile::tempdir().unwrap();
        let directory = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority()).unwrap();
        sync_directory(&directory).unwrap();
    }

    #[test]
    fn windows_held_rename_replaces_only_when_authorized() {
        let temp = tempfile::tempdir().unwrap();
        let directory = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority()).unwrap();
        directory.write("source", b"new").unwrap();
        directory.write("target", b"old").unwrap();
        assert!(rename_at(
            &directory,
            "source".as_ref(),
            &directory,
            "target".as_ref(),
            false
        )
        .is_err());
        assert_eq!(directory.read("target").unwrap(), b"old");
        rename_at(
            &directory,
            "source".as_ref(),
            &directory,
            "target".as_ref(),
            true,
        )
        .unwrap();
        assert_eq!(directory.read("target").unwrap(), b"new");
        assert!(!directory.exists("source"));
    }

    #[test]
    fn windows_held_rename_moves_directories_without_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let directory = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority()).unwrap();
        directory.create_dir("source").unwrap();
        directory.create_dir("target").unwrap();
        directory.write("source/payload", b"data").unwrap();
        let held_source = open_directory_nofollow(&directory, "source".as_ref()).unwrap();
        assert!(rename_at(
            &directory,
            "source".as_ref(),
            &directory,
            "target".as_ref(),
            false
        )
        .is_err());
        rename_at(
            &directory,
            "source".as_ref(),
            &directory,
            "moved".as_ref(),
            false,
        )
        .unwrap();
        assert_eq!(directory.read("moved/payload").unwrap(), b"data");
        assert_eq!(held_source.read("payload").unwrap(), b"data");
    }

    #[test]
    fn windows_directory_authority_rejects_junctions() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("sentinel"), b"preserve").unwrap();
        let output = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/j"])
            .arg(temp.path().join("junction"))
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let directory = open_directory(temp.path()).unwrap();
        assert!(open_directory_nofollow(&directory, "junction".as_ref()).is_err());
        assert_eq!(
            std::fs::read(outside.path().join("sentinel")).unwrap(),
            b"preserve"
        );
        std::fs::remove_dir(temp.path().join("junction")).unwrap();
    }
}
