use super::*;
use std::io::Write;

fn ar_member(name: &str, data: &[u8]) -> Vec<u8> {
    let mut result = format!(
        "{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
        0,
        0,
        0,
        "100644",
        data.len()
    )
    .into_bytes();
    assert_eq!(result.len(), 60);
    result.extend_from_slice(data);
    if !data.len().is_multiple_of(2) {
        result.push(b'\n');
    }
    result
}
fn deb(data: &[u8]) -> Vec<u8> {
    let mut result = b"!<arch>\n".to_vec();
    result.extend(ar_member("debian-binary", b"2.0\n"));
    result.extend(ar_member("control.tar.xz", b"never executed"));
    result.extend(ar_member("data.tar.xz", data));
    result
}
fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    for (name, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, name, *bytes).unwrap();
    }
    let bytes = tar.into_inner().unwrap();
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
    encoder.write_all(&bytes).unwrap();
    deb(&encoder.finish().unwrap())
}
#[test]
fn source_recipe_is_closed_and_hashes_are_not_receipt_authority() {
    let selected = recipe().unwrap();
    assert_eq!(selected.packages.len(), 14);
    for package in selected.packages {
        assert!(verify(&package, b"substitute").is_err());
    }
}
#[test]
fn ar_requires_exact_format_and_never_uses_control_scripts() {
    let bytes = deb(b"selected payload");
    assert_eq!(data_archive(&bytes).unwrap(), b"selected payload");
    for end in [0, 7, 15, 60, bytes.len() - 1] {
        assert!(data_archive(&bytes[..end]).is_err());
    }
    let mut duplicate = bytes;
    duplicate.extend(ar_member("data.tar.xz", b"substitute"));
    assert!(data_archive(&duplicate).is_err());
}
#[test]
fn extraction_selects_only_fixed_library_and_copyright_namespaces() {
    let bytes = archive(&[
        (
            "usr/lib/x86_64-linux-gnu/libfixture.so",
            b"inert native fixture",
        ),
        ("usr/share/doc/fixture/copyright", b"fixture notice"),
        ("etc/ld.so.conf", b"must never configure host"),
        ("usr/bin/script", b"must never execute"),
    ]);
    let mut files = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    unpack(&bytes, &mut files, &mut aliases).unwrap();
    assert_eq!(files.len(), 2);
    assert!(aliases.is_empty());
    assert!(!files.contains_key("etc/ld.so.conf"));
    assert!(unpack(&bytes, &mut files, &mut aliases).is_err());
}
#[test]
fn names_refuse_absolute_and_parent_traversal() {
    for name in [
        "/usr/lib/evil",
        "usr/../evil",
        "../evil",
        "usr\\evil",
        "usr/evil\0",
    ] {
        assert!(closed_path(Path::new(name)).is_err(), "accepted {name:?}");
    }
    assert_eq!(
        closed_path(Path::new("./usr/lib/test.so")).unwrap(),
        "usr/lib/test.so"
    );
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
#[test]
#[ignore = "requires the exact source-pinned Debian archives; no model or kernel qualification"]
fn official_cohort_retains_pinned_bytes_not_ambient_libraries() {
    let directory =
        std::env::var_os("PUMAS_AUDIO_NATIVE_ARCHIVES").expect("fixed archive cache required");
    let recipe = recipe().unwrap();
    let archives = recipe
        .packages
        .iter()
        .map(|package| {
            let filename = reqwest::Url::parse(&package.url)
                .unwrap()
                .path_segments()
                .unwrap()
                .next_back()
                .unwrap()
                .to_owned();
            std::fs::read(Path::new(&directory).join(filename)).unwrap()
        })
        .collect();
    let parent = tempfile::tempdir().unwrap();
    let selection = stage(parent.path(), &recipe, archives).unwrap();
    let retained =
        pumas_library::runtime_read_source::RetainedRuntimeReadSource::capture(vec![selection])
            .unwrap();
    retained.validate().unwrap();
    let pin: serde_json::Value =
        serde_json::from_str(pumas_library::runtime_read_source::AUDIO_RUNTIME_CANDIDATE_RECIPE)
            .unwrap();
    let mut members = retained
        .manifest()
        .map(|(_, member)| (member.path(), member.size(), member.sha256()))
        .collect::<Vec<_>>();
    members.sort();
    assert_eq!(
        members.len() as u64,
        pin["native_selection"]["members"].as_u64().unwrap()
    );
    let mut hash = Sha256::new();
    hash.update(b"pumas-audio-runtime-recipe-v1\0");
    for (name, size, sha) in members {
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update(size.to_be_bytes());
        hash.update(sha.as_bytes());
    }
    assert_eq!(
        format!("{:x}", hash.finalize()),
        pin["native_selection"]["sha256"].as_str().unwrap()
    );
    let loader = retained
        .clone_member(RuntimeReadRole::NativeLibraries, LOADER)
        .unwrap();
    assert!(loader.metadata().unwrap().len() > 0);
    assert!(retained
        .clone_member(RuntimeReadRole::NativeLibraries, "etc/ld.so.conf")
        .is_err());
    assert!(retained
        .manifest()
        .any(|(_, member)| member.path() == "usr/share/common-licenses/LGPL-2.1"));
    let root = retained
        .clone_root(RuntimeReadRole::NativeLibraries)
        .unwrap();
    use std::os::fd::AsRawFd;
    let owned_path = std::fs::read_link(format!("/proc/self/fd/{}", root.as_raw_fd())).unwrap();
    let another = retained.clone();
    drop(retained);
    assert!(owned_path.exists());
    drop(another);
    assert!(!owned_path.exists());
}
