//! Native operations on held filesystem capabilities.
#![deny(unsafe_code)]

use cap_std::fs::Dir;
use std::fs::File;
use std::io;

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
#[allow(unsafe_code)]
pub(crate) fn sync_directory_file(file: &File) -> io::Result<()> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        ReOpenFile, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE,
    };
    // SAFETY: the borrowed handle remains live throughout ReOpenFile. A successful
    // return is a new owned handle, transferred exactly once into File.
    let handle = unsafe {
        ReOpenFile(
            file.as_raw_handle(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_FLAG_BACKUP_SEMANTICS,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ReOpenFile returned a valid, newly owned handle above.
    unsafe { File::from_raw_handle(handle) }.sync_all()
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
    use windows_sys::Win32::Storage::FileSystem::{
        FileRenameInfo, SetFileInformationByHandle, DELETE, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_RENAME_INFO,
    };
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
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let bytes = offset
        .checked_add(name.len() * 2)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "rename name too long"))?;
    // Word storage gives FILE_RENAME_INFO its required pointer alignment and
    // enough initialized space for the variable-length UTF-16 tail.
    let mut storage = vec![0usize; (bytes as usize).div_ceil(std::mem::size_of::<usize>())];
    let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    // SAFETY: storage is aligned, initialized, and sized for the header and name.
    // Both handles and all pointers remain live for this synchronous call.
    let result = unsafe {
        (*info).Anonymous.ReplaceIfExists = u8::from(replace);
        (*info).RootDirectory = target_parent.as_raw_handle();
        (*info).FileNameLength = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(name.as_ptr(), (*info).FileName.as_mut_ptr(), name.len());
        SetFileInformationByHandle(source.as_raw_handle(), FileRenameInfo, info.cast(), bytes)
    };
    if result == 0 {
        Err(io::Error::last_os_error())
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
    }
}
