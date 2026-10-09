//! Source-pinned Linux native runtime bytes in a fresh owned directory.
//!
//! This is installer provenance and byte custody, not execution qualification.
//! No dpkg scripts run, no host library is copied, and no global loader or OS
//! setting changes. The confined child owner must retain this root through exit.

use pumas_library::runtime_read_source::{RuntimeReadFile, RuntimeReadRole, RuntimeReadRoot};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::{Component, Path};
use std::sync::Arc;

const RECIPE: &str = include_str!("audio_native_cohort/recipe.json");
const MAX_ARCHIVE: usize = 8 * 1024 * 1024;
const MAX_EXPANDED: u64 = 32 * 1024 * 1024;
const MAX_MEMBER: u64 = 8 * 1024 * 1024;
const MAX_FILES: usize = 4096;
pub(super) const LOADER: &str = "usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2";
pub(super) const LIBRARIES: &str = "usr/lib/x86_64-linux-gnu";

#[derive(Deserialize)]
struct Recipe {
    recipe: String,
    packages: Vec<Package>,
}
#[derive(Deserialize)]
struct Package {
    name: String,
    url: String,
    size: usize,
    sha256: String,
}
fn refuse(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn recipe() -> io::Result<Recipe> {
    let recipe: Recipe = serde_json::from_str(RECIPE).map_err(io::Error::other)?;
    if recipe.recipe != "debian-trixie-amd64-20261009-v1" || recipe.packages.len() != 14 {
        return Err(refuse("native recipe identity or package count differs"));
    }
    let mut names = std::collections::BTreeSet::new();
    for package in &recipe.packages {
        let url = reqwest::Url::parse(&package.url).map_err(io::Error::other)?;
        if url.scheme() != "https"
            || url.host_str() != Some("deb.debian.org")
            || !url.path().starts_with("/debian/pool/main/")
            || !url.path().ends_with("_amd64.deb")
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || package.size == 0
            || package.size > MAX_ARCHIVE
            || package.sha256.len() != 64
            || !package.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !names.insert(&package.name)
        {
            return Err(refuse("native recipe source pin is invalid"));
        }
    }
    Ok(recipe)
}
fn verify(package: &Package, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() != package.size || format!("{:x}", Sha256::digest(bytes)) != package.sha256 {
        return Err(refuse(
            "native package size or SHA-256 differs from source pin",
        ));
    }
    Ok(())
}

/// Download only fixed public artifacts; caller/request metadata cannot select
/// a URL or digest. Redirects and responses above the exact pin are refused.
pub(super) async fn prepare(parent: &Path) -> io::Result<RuntimeReadRoot> {
    if !cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native cohort supports Linux x86_64 only",
        ));
    }
    let recipe = recipe()?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(io::Error::other)?;
    let mut archives = Vec::new();
    for package in &recipe.packages {
        let mut response = client
            .get(&package.url)
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?;
        if response
            .content_length()
            .is_some_and(|size| size != package.size as u64)
        {
            return Err(refuse("native package response length differs"));
        }
        let mut bytes = Vec::with_capacity(package.size);
        while let Some(chunk) = response.chunk().await.map_err(io::Error::other)? {
            if chunk.len() > package.size.saturating_sub(bytes.len()) {
                return Err(refuse("native package response exceeds source pin"));
            }
            bytes.extend_from_slice(&chunk);
        }
        verify(package, &bytes)?;
        archives.push(bytes);
    }
    // This parent is supplied by VersionManager, never by the inference caller.
    let parent = parent.to_owned();
    tokio::task::spawn_blocking(move || stage(&parent, &recipe, archives))
        .await
        .map_err(io::Error::other)?
}

/// Parse only the bounded three-member Debian ar container. Its control archive
/// is never unpacked/executed. No archive-selected path is used for filesystem IO.
fn data_archive(bytes: &[u8]) -> io::Result<&[u8]> {
    if !bytes.starts_with(b"!<arch>\n") {
        return Err(refuse("native package is not ar"));
    }
    let mut offset = 8usize;
    let mut result = None;
    let mut names = std::collections::BTreeSet::new();
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset + 60)
            .ok_or_else(|| refuse("truncated ar header"))?;
        if &header[58..] != b"`\n" {
            return Err(refuse("invalid ar header"));
        }
        let name = std::str::from_utf8(&header[..16])
            .map_err(io::Error::other)?
            .trim();
        if !matches!(name, "debian-binary" | "control.tar.xz" | "data.tar.xz")
            || !names.insert(name)
        {
            return Err(refuse("unexpected native package member"));
        }
        let size: usize = std::str::from_utf8(&header[48..58])
            .map_err(io::Error::other)?
            .trim()
            .parse()
            .map_err(io::Error::other)?;
        let start = offset
            .checked_add(60)
            .ok_or_else(|| refuse("ar offset overflow"))?;
        let end = start
            .checked_add(size)
            .ok_or_else(|| refuse("ar size overflow"))?;
        let member = bytes
            .get(start..end)
            .ok_or_else(|| refuse("truncated ar member"))?;
        if name == "debian-binary" && member != b"2.0\n" {
            return Err(refuse("unknown Debian package format"));
        }
        if name == "data.tar.xz" {
            result = Some(member);
        }
        offset = end
            .checked_add(size % 2)
            .ok_or_else(|| refuse("ar alignment overflow"))?;
    }
    if offset != bytes.len() || names.len() != 3 {
        return Err(refuse("incomplete native package"));
    }
    result.ok_or_else(|| refuse("native package has no data"))
}

fn closed_path(path: &Path) -> io::Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir if parts.is_empty() => {}
            Component::Normal(value) => parts.push(
                value
                    .to_str()
                    .ok_or_else(|| refuse("non-UTF8 native member"))?,
            ),
            _ => return Err(refuse("native member escapes archive namespace")),
        }
    }
    let value = parts.join("/");
    if value.len() > 1024 || value.contains('\\') || value.contains('\0') {
        return Err(refuse("native member name exceeds bound"));
    }
    Ok(value)
}
fn native_file(name: &str) -> bool {
    name.starts_with("usr/lib/x86_64-linux-gnu/")
}
fn notice(name: &str) -> bool {
    (name.starts_with("usr/share/doc/") && name.ends_with("/copyright"))
        || name.starts_with("usr/share/common-licenses/")
}

fn unpack(
    bytes: &[u8],
    files: &mut BTreeMap<String, Vec<u8>>,
    aliases: &mut BTreeMap<String, String>,
) -> io::Result<()> {
    let decoder = xz2::read::XzDecoder::new(Cursor::new(data_archive(bytes)?));
    let mut expanded = Vec::new();
    decoder.take(MAX_EXPANDED + 1).read_to_end(&mut expanded)?;
    if expanded.len() as u64 > MAX_EXPANDED {
        return Err(refuse("native archive expands beyond bound"));
    }
    let mut archive = tar::Archive::new(Cursor::new(expanded));
    let mut count = 0;
    for entry in archive.entries()? {
        let mut entry = entry?;
        count += 1;
        if count > MAX_FILES {
            return Err(refuse("native archive member count exceeded"));
        }
        let name = closed_path(&entry.path()?)?;
        if !native_file(&name) && !notice(&name) {
            continue;
        }
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        if kind.is_file() {
            if entry.size() > MAX_MEMBER {
                return Err(refuse("native member exceeds size bound"));
            }
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            if aliases.contains_key(&name) || files.insert(name, data).is_some() {
                return Err(refuse("duplicate native member"));
            }
        } else if kind.is_symlink() && (native_file(&name) || notice(&name)) {
            let target = entry
                .link_name()?
                .ok_or_else(|| refuse("native alias has no target"))?;
            // Runtime aliases in this fixed cohort are siblings. Do not accept
            // absolute, parent traversal, chained directory or cross-root links.
            let target = closed_path(&target)?;
            if target.is_empty() || target.contains('/') {
                return Err(refuse("native alias is not a sibling"));
            }
            let resolved = format!(
                "{}/{target}",
                name.rsplit_once('/')
                    .ok_or_else(|| refuse("invalid native alias"))?
                    .0
            );
            if files.contains_key(&name) || aliases.insert(name, resolved).is_some() {
                return Err(refuse("duplicate native alias"));
            }
        } else {
            return Err(refuse("unsupported selected native member type"));
        }
    }
    Ok(())
}

fn stage(parent: &Path, recipe: &Recipe, archives: Vec<Vec<u8>>) -> io::Result<RuntimeReadRoot> {
    if archives.len() != recipe.packages.len() {
        return Err(refuse("native archive cohort incomplete"));
    }
    let mut files = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    for (package, archive) in recipe.packages.iter().zip(&archives) {
        verify(package, archive)?;
        unpack(archive, &mut files, &mut aliases)?;
    }
    for (name, target) in aliases {
        let bytes = files
            .get(&target)
            .ok_or_else(|| refuse("native alias target missing or chained"))?
            .clone();
        if files.insert(name, bytes).is_some() {
            return Err(refuse("native alias overlaps a member"));
        }
    }
    for package in &recipe.packages {
        let copyright_package = match package.name.as_str() {
            "libgcc-s1" | "libgomp1" | "libstdc++6" => "gcc-14-base",
            other => other,
        };
        if !files.contains_key(&format!("usr/share/doc/{copyright_package}/copyright")) {
            return Err(refuse("native package copyright evidence missing"));
        }
    }
    if !files.contains_key(LOADER) || !files.contains_key(&format!("{LIBRARIES}/libc.so.6")) {
        return Err(refuse("native loader/libc pair missing"));
    }
    files.insert("provenance.json".into(), RECIPE.as_bytes().to_vec());
    if files.len() > MAX_FILES {
        return Err(refuse("native cohort file bound exceeded"));
    }
    let parent_metadata = std::fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || std::fs::canonicalize(parent)? != parent
    {
        return Err(refuse("native staging parent is not canonical directory"));
    }
    let owner = Arc::new(
        tempfile::Builder::new()
            .prefix("audio-native-")
            .tempdir_in(parent)?,
    );
    let mut expected = Vec::new();
    for (name, bytes) in files {
        let path = owner.path().join(&name);
        std::fs::create_dir_all(
            path.parent()
                .ok_or_else(|| refuse("native destination parent missing"))?,
        )?;
        std::fs::write(&path, &bytes)?;
        if name == LOADER {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        expected.push(RuntimeReadFile::new(
            name,
            bytes.len() as u64,
            format!("{:x}", Sha256::digest(&bytes)),
        )?);
    }
    let directory = File::open(owner.path())?;
    RuntimeReadRoot::new(
        RuntimeReadRole::NativeLibraries,
        directory,
        expected,
        vec![],
        owner,
    )
}

#[cfg(test)]
#[path = "audio_native_cohort/tests.rs"]
mod tests;
