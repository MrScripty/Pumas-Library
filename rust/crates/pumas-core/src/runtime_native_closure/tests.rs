use super::*;
use crate::platform::capability_fs::open_pinned_directory;
use crate::runtime_read_source::{RuntimeReadFile, RuntimeReadRoot};
use sha2::{Digest, Sha256};
use std::io::{Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

fn write16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn write32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn write64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn elf(needed: &[&str], soname: Option<&str>, extra: &[(u64, u64)]) -> Vec<u8> {
    let mut bytes = vec![0; 4096];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    write16(&mut bytes, 16, 3);
    write16(&mut bytes, 18, 62);
    write32(&mut bytes, 20, 1);
    write64(&mut bytes, 32, 64);
    write16(&mut bytes, 52, 64);
    write16(&mut bytes, 54, 56);
    write16(&mut bytes, 56, 2);
    write32(&mut bytes, 64, 1);
    write64(&mut bytes, 64 + 32, 4096);
    write64(&mut bytes, 64 + 40, 4096);
    let mut strings = vec![0];
    let mut entries = Vec::new();
    for name in needed {
        entries.push((1, strings.len() as u64));
        strings.extend_from_slice(name.as_bytes());
        strings.push(0);
    }
    if let Some(name) = soname {
        entries.push((14, strings.len() as u64));
        strings.extend_from_slice(name.as_bytes());
        strings.push(0);
    }
    entries.extend_from_slice(extra);
    entries.extend([(5, 2048), (10, strings.len() as u64), (0, 0)]);
    write32(&mut bytes, 120, 2);
    write64(&mut bytes, 128, 512);
    write64(&mut bytes, 136, 512);
    write64(&mut bytes, 152, (entries.len() * 16) as u64);
    write64(&mut bytes, 160, (entries.len() * 16) as u64);
    for (index, (tag, value)) in entries.iter().enumerate() {
        write64(&mut bytes, 512 + index * 16, *tag);
        write64(&mut bytes, 520 + index * 16, *value);
    }
    bytes[2048..2048 + strings.len()].copy_from_slice(&strings);
    bytes
}
fn member(path: &str) -> NativeMember {
    NativeMember {
        role: RuntimeReadRole::Dependencies,
        path: path.into(),
    }
}
fn source(
    root: &Path,
    files: &[(&str, Vec<u8>)],
    lease: Arc<dyn Send + Sync>,
) -> Arc<RetainedRuntimeReadSource> {
    let mut manifest = Vec::new();
    for (path, bytes) in files {
        std::fs::write(root.join(path), bytes).unwrap();
        manifest.push(
            RuntimeReadFile::new(
                (*path).into(),
                bytes.len() as u64,
                hex::encode(Sha256::digest(bytes)),
            )
            .unwrap(),
        );
    }
    RetainedRuntimeReadSource::capture(vec![RuntimeReadRoot::new(
        RuntimeReadRole::Dependencies,
        open_pinned_directory(root).unwrap().into_std_file(),
        manifest,
        vec![],
        lease,
    )
    .unwrap()])
    .unwrap()
}
fn refuses(bytes: Vec<u8>) {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path(), &[("entry.so", bytes)], Arc::new(()));
    assert!(RetainedNativeDependencyGraph::inspect(source, &[member("entry.so")]).is_err());
}

#[test]
fn exact_retained_transitive_and_cyclic_edges_are_resolved_without_loader_execution() {
    let root = tempfile::tempdir().unwrap();
    let source = source(
        root.path(),
        &[
            ("entry.so", elf(&["liba.so"], None, &[])),
            ("actual-a.so", elf(&["libb.so"], Some("liba.so"), &[])),
            ("actual-b.so", elf(&["liba.so"], Some("libb.so"), &[])),
        ],
        Arc::new(()),
    );
    let graph = RetainedNativeDependencyGraph::inspect(
        source,
        &[
            member("entry.so"),
            member("actual-a.so"),
            member("actual-b.so"),
        ],
    )
    .unwrap();
    assert_eq!(
        graph.nodes()[0].dependencies[0].selected,
        member("actual-a.so")
    );
    assert_eq!(
        graph.nodes()[1].dependencies[0].selected,
        member("actual-b.so")
    );
    assert_eq!(
        graph.nodes()[2].dependencies[0].selected,
        member("actual-a.so")
    );
}

#[test]
fn unresolved_external_dependency_and_path_names_refuse() {
    for name in [
        "libexternal.so",
        "/usr/lib/libexternal.so",
        "../external.so",
        "$ORIGIN/external.so",
    ] {
        refuses(elf(&[name], None, &[]));
    }
}

#[test]
fn interpreter_is_refused_even_if_a_same_named_retained_library_exists() {
    let mut entry = elf(&[], None, &[]);
    write16(&mut entry, 56, 3);
    write32(&mut entry, 176, 3);
    write64(&mut entry, 184, 3000);
    write64(&mut entry, 208, 32);
    write64(&mut entry, 216, 32);
    entry[3000..3028].copy_from_slice(b"/lib64/ld-linux-x86-64.so.2\0");
    let root = tempfile::tempdir().unwrap();
    let source = source(
        root.path(),
        &[
            ("entry.so", entry),
            ("ld-linux-x86-64.so.2", elf(&[], None, &[])),
        ],
        Arc::new(()),
    );
    assert!(RetainedNativeDependencyGraph::inspect(
        source,
        &[member("entry.so"), member("ld-linux-x86-64.so.2")]
    )
    .is_err());
}

#[test]
fn rpath_runpath_audit_config_and_filter_loaders_refuse() {
    for tag in [
        15,
        29,
        0x6fff_fdf6,
        0x6fff_fdf7,
        0x6fff_fef8,
        0x6fff_fef9,
        0x6fff_fefa,
        0x6fff_fefb,
        0x6fff_fefc,
        0x7fff_fffd,
        0x7fff_ffff,
    ] {
        refuses(elf(&[], None, &[(tag, 0)]));
    }
    for flags in [0x10, 0x2000] {
        refuses(elf(&[], None, &[(0x6fff_fffb, flags)]));
    }
}

#[test]
fn malformed_truncated_overflowed_and_incoherent_tables_refuse() {
    let mut malformed = Vec::new();
    malformed.push(vec![0; 63]);
    let mut bytes = elf(&[], None, &[]);
    bytes[4] = 1;
    malformed.push(bytes);
    let mut bytes = elf(&[], None, &[]);
    write16(&mut bytes, 56, u16::MAX);
    malformed.push(bytes);
    let mut bytes = elf(&[], None, &[]);
    write64(&mut bytes, 32, u64::MAX - 20);
    malformed.push(bytes);
    let mut bytes = elf(&[], None, &[]);
    write64(&mut bytes, 128, u64::MAX);
    malformed.push(bytes);
    let mut bytes = elf(&[], None, &[]);
    write64(&mut bytes, 136, 1024);
    malformed.push(bytes);
    let mut bytes = elf(&[], None, &[]);
    write16(&mut bytes, 56, 3);
    write32(&mut bytes, 176, 1);
    write64(&mut bytes, 176 + 32, 4096);
    write64(&mut bytes, 176 + 40, 4096);
    malformed.push(bytes);
    let mut bytes = elf(&[], None, &[]);
    write64(&mut bytes, 528, 10);
    write64(&mut bytes, 536, (MAX_STRING_TABLE + 1) as u64);
    malformed.push(bytes);
    let mut bytes = elf(&[], None, &[]);
    write64(&mut bytes, 544, 9);
    malformed.push(bytes);
    let mut bytes = elf(&["good.so"], None, &[]);
    write64(&mut bytes, 520, u64::MAX);
    malformed.push(bytes);
    let mut bytes = elf(&["good.so"], None, &[]);
    bytes[2056] = b'x';
    malformed.push(bytes);
    malformed.push(elf(&["same.so", "same.so"], None, &[]));
    malformed.push(elf(&[], Some("liba.so"), &[(14, 1)]));
    for bytes in malformed {
        refuses(bytes);
    }
}

#[test]
fn duplicate_basename_soname_or_member_is_never_guessed() {
    let root = tempfile::tempdir().unwrap();
    let source = source(
        root.path(),
        &[
            ("one.so", elf(&[], Some("same.so"), &[])),
            ("two.so", elf(&[], Some("same.so"), &[])),
        ],
        Arc::new(()),
    );
    assert!(RetainedNativeDependencyGraph::inspect(
        source.clone(),
        &[member("one.so"), member("two.so")]
    )
    .is_err());
    assert!(
        RetainedNativeDependencyGraph::inspect(source, &[member("one.so"), member("one.so")])
            .is_err()
    );
}

#[test]
fn descriptor_reads_preserve_offsets_and_changed_source_cannot_gain_a_graph() {
    let root = tempfile::tempdir().unwrap();
    let bytes = elf(&[], None, &[]);
    std::fs::write(root.path().join("entry.so"), &bytes).unwrap();
    let mut file = File::open(root.path().join("entry.so")).unwrap();
    file.seek(SeekFrom::Start(19)).unwrap();
    inspect_elf(&file).unwrap();
    assert_eq!(file.stream_position().unwrap(), 19);
    let source = source(root.path(), &[("entry.so", bytes.clone())], Arc::new(()));
    let successor = root.path().join("successor");
    std::fs::write(&successor, &bytes).unwrap();
    std::fs::rename(successor, root.path().join("entry.so")).unwrap();
    assert!(RetainedNativeDependencyGraph::inspect(source, &[member("entry.so")]).is_err());
}

#[test]
fn graph_keeps_exact_byte_lease_until_final_disposal() {
    struct Lease(Arc<AtomicUsize>);
    impl Drop for Lease {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let root = tempfile::tempdir().unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let source = source(
        root.path(),
        &[("entry.so", elf(&[], None, &[]))],
        Arc::new(Lease(drops.clone())),
    );
    let graph =
        RetainedNativeDependencyGraph::inspect(source.clone(), &[member("entry.so")]).unwrap();
    drop(source);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(graph);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn locally_compiled_actual_shared_objects_form_a_retained_graph() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("leaf.c"),
        "int controlled_leaf(void) { return 73; }\n",
    )
    .unwrap();
    std::fs::write(root.path().join("entry.c"),"extern int controlled_leaf(void); int controlled_entry(void) { return controlled_leaf(); }\n").unwrap();
    for args in [
        vec![
            "-shared",
            "-fPIC",
            "-nostdlib",
            "leaf.c",
            "-Wl,-soname,libcontrolled_leaf.so",
            "-o",
            "libcontrolled_leaf.so",
        ],
        vec![
            "-shared",
            "-fPIC",
            "-nostdlib",
            "entry.c",
            "-L.",
            "-lcontrolled_leaf",
            "-o",
            "entry.so",
        ],
    ] {
        let output = std::process::Command::new("cc")
            .args(args)
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let files: Vec<_> = ["leaf.c", "entry.c", "libcontrolled_leaf.so", "entry.so"]
        .into_iter()
        .map(|name| (name, std::fs::read(root.path().join(name)).unwrap()))
        .collect();
    let source = source(root.path(), &files, Arc::new(()));
    let graph = RetainedNativeDependencyGraph::inspect(
        source.clone(),
        &[member("entry.so"), member("libcontrolled_leaf.so")],
    )
    .unwrap();
    assert_eq!(
        graph.nodes()[0].dependencies,
        vec![NativeDependency {
            requested: "libcontrolled_leaf.so".into(),
            selected: member("libcontrolled_leaf.so")
        }]
    );
    assert!(graph.nodes()[1].dependencies.is_empty());
    assert!(RetainedNativeDependencyGraph::inspect(source, &[member("entry.so")]).is_err());
}
