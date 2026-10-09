//! Offline byte assembly strategy for the existing Torch publication owner.
use super::*;
use crate::version_manager::torch_component::{
    safe_member, TorchComponentFile, TorchComponentPlan,
};
use pumas_library::acquisition::{AcquisitionHost, HttpAttemptHost};
use pumas_library::runtime_read_source::{RetainedRuntimeReadSource, RuntimeReadRole};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};

struct ComponentHost {
    cancel: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl HttpAttemptHost for ComponentHost {
    async fn pause_requested(&self) {
        std::future::pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        false
    }
    fn cancel_requested(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) || self.shutdown.load(Ordering::SeqCst)
    }
    async fn record_progress(&mut self, _: u64) -> Result<()> {
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for ComponentHost {
    async fn retry(&mut self, _: u32, delay: Option<Duration>, _: Option<&str>) -> Result<()> {
        if self.cancel_requested() {
            return Err(failed("Component acquisition cancelled"));
        }
        if let Some(delay) = delay {
            tokio::select! {
                _ = tokio::time::sleep(delay) => {},
                _ = super::super::super::wait_for_install_cancel(self.cancel.clone()) => return Err(failed("Component acquisition cancelled")),
                _ = super::super::super::wait_for_install_cancel(self.shutdown.clone()) => return Err(failed("Component owner shutting down")),
            }
        }
        Ok(())
    }
}

fn copy_exact(reader: &mut impl Read, output: &Path, expected: &TorchComponentFile) -> Result<()> {
    let parent = output
        .parent()
        .ok_or_else(|| failed("Component output parent absent"))?;
    std::fs::create_dir_all(parent).map_err(PumasError::from)?;
    let mut destination = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .map_err(PumasError::from)?;
    let mut sha = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let count = reader.read(&mut buffer).map_err(PumasError::from)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| failed("Component copy size overflow"))?;
        if size > expected.size {
            return Err(failed("Component copied bytes exceed manifest"));
        }
        destination
            .write_all(&buffer[..count])
            .map_err(PumasError::from)?;
        sha.update(&buffer[..count]);
    }
    if size != expected.size || format!("{:x}", sha.finalize()) != expected.sha256 {
        return Err(failed("Component copied size or digest changed"));
    }
    Ok(())
}

fn digest_url(hash: &str) -> Result<String> {
    // RECORD's canonical SHA256 is URL-safe unpadded base64, never arbitrary text.
    let bytes: Vec<u8> = hash
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(
                std::str::from_utf8(pair).map_err(|_| failed("Invalid RECORD digest"))?,
                16,
            )
            .map_err(|_| failed("Invalid RECORD digest"))
        })
        .collect::<Result<_>>()?;
    if bytes.len() != 32 {
        return Err(failed("Invalid RECORD digest length"));
    }
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut encoded = String::new();
    let mut bits = 0_u32;
    let mut remaining = 0;
    for byte in bytes {
        bits = (bits << 8) | u32::from(byte);
        remaining += 8;
        while remaining >= 6 {
            remaining -= 6;
            encoded.push(ALPHABET[((bits >> remaining) & 63) as usize] as char);
        }
    }
    if remaining != 0 {
        encoded.push(ALPHABET[((bits << (6 - remaining)) & 63) as usize] as char);
    }
    Ok(encoded)
}

fn bounded_text(path: &Path, bound: u64) -> Result<String> {
    let file = File::open(path).map_err(PumasError::from)?;
    let mut bytes = Vec::new();
    file.take(bound + 1)
        .read_to_end(&mut bytes)
        .map_err(PumasError::from)?;
    if bytes.len() as u64 > bound {
        return Err(failed("Component metadata exceeds bound"));
    }
    String::from_utf8(bytes).map_err(|_| failed("Component metadata is not UTF-8"))
}

fn header_values<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    text.lines()
        .take_while(|line| !line.is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name == key).then_some(value.trim())
        })
        .collect()
}

fn validate_record(
    packages: &Path,
    component: &crate::version_manager::TorchComponentManifest,
) -> Result<()> {
    let prefix = format!(
        "{}-{}.dist-info/",
        component.distribution.replace('-', "_"),
        component.version
    );
    let record = format!("{prefix}RECORD");
    let metadata = bounded_text(&packages.join(format!("{prefix}METADATA")), 1024 * 1024)?;
    let wheel = bounded_text(&packages.join(format!("{prefix}WHEEL")), 1024 * 1024)?;
    let normalize = |s: &str| s.to_ascii_lowercase().replace('_', "-");
    let names = header_values(&metadata, "Name");
    if names.len() != 1
        || normalize(names[0]) != normalize(&component.distribution)
        || header_values(&metadata, "Version") != vec![component.version.as_str()]
        || header_values(&wheel, "Wheel-Version") != vec!["1.0"]
        || header_values(&wheel, "Tag") != vec![component.wheel_tag.as_str()]
    {
        return Err(failed("Wheel distribution/version/tag metadata mismatch"));
    }
    let text = bounded_text(&packages.join(&record), 16 * 1024 * 1024)?;
    let expected: BTreeMap<_, _> = component
        .archive_files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let mut seen = BTreeSet::new();
    for row in text.lines() {
        // Only canonical unquoted CSV is supported: reviewed portable paths
        // cannot contain comma, quote, CR or LF. Reject unsupported CSV rather
        // than misparse or silently ignore it.
        let cells: Vec<_> = row.split(',').collect();
        if cells.len() != 3
            || !safe_member(cells[0])
            || !seen.insert(cells[0])
            || seen.len() > 200_000
        {
            return Err(failed("Invalid, duplicate or unsupported wheel RECORD row"));
        }
        let file = expected
            .get(cells[0])
            .ok_or_else(|| failed("RECORD names an unselected member"))?;
        if cells[0] == record {
            if !cells[1].is_empty() || !cells[2].is_empty() {
                return Err(failed("RECORD self row must have blank hash and size"));
            }
        } else if cells[1] != format!("sha256={}", digest_url(&file.sha256)?)
            || cells[2] != file.size.to_string()
        {
            return Err(failed("Wheel RECORD hash or size mismatch"));
        }
    }
    if seen.len() != expected.len() || !seen.contains(record.as_str()) {
        return Err(failed(
            "Wheel RECORD does not describe its exact closed namespace",
        ));
    }
    Ok(())
}

fn preflight_zip(file: &mut File) -> Result<()> {
    use std::io::{Seek, SeekFrom};
    let size = file.metadata().map_err(PumasError::from)?.len();
    if !(22..=2 * 1024 * 1024 * 1024).contains(&size) {
        return Err(failed("Wheel compressed size exceeds standard ZIP bounds"));
    }
    let tail_size = size.min(65557);
    file.seek(SeekFrom::Start(size - tail_size))
        .map_err(PumasError::from)?;
    let mut tail = vec![0_u8; tail_size as usize];
    file.read_exact(&mut tail).map_err(PumasError::from)?;
    let footer = tail
        .windows(4)
        .rposition(|bytes| bytes == b"PK\x05\x06")
        .ok_or_else(|| failed("Standard ZIP footer missing"))?;
    if footer + 22 > tail.len() {
        return Err(failed("Truncated ZIP footer"));
    }
    let u16_at =
        |offset: usize| u16::from_le_bytes([tail[footer + offset], tail[footer + offset + 1]]);
    let u32_at = |offset: usize| {
        u32::from_le_bytes(
            tail[footer + offset..footer + offset + 4]
                .try_into()
                .expect("fixed footer field"),
        )
    };
    let count = u16_at(10);
    let central_size = u32_at(12);
    let central_start = u32_at(16);
    let footer_position = size - tail_size + footer as u64;
    if u16_at(4) != 0
        || u16_at(6) != 0
        || u16_at(8) != count
        || count == 0
        || count == u16::MAX
        || central_size == u32::MAX
        || central_start == u32::MAX
        || central_size > 64 * 1024 * 1024
        || footer + 22 + u16_at(20) as usize != tail.len()
        || u64::from(central_start) + u64::from(central_size) != footer_position
    {
        return Err(failed(
            "Unsupported ZIP64, split or oversized central directory",
        ));
    }
    // zip 2.x may retry earlier footer candidates after a central-header error.
    // This deliberately narrow subset rejects every alternate footer signature,
    // including embedded data, before the library can choose unbounded metadata.
    file.seek(SeekFrom::Start(0)).map_err(PumasError::from)?;
    let mut scanned = 0_u64;
    let mut window = Vec::with_capacity(65539);
    let mut chunk = [0_u8; 65536];
    loop {
        let count = file.read(&mut chunk).map_err(PumasError::from)?;
        if count == 0 {
            break;
        }
        let start = scanned - window.len() as u64;
        window.extend_from_slice(&chunk[..count]);
        for (offset, signature) in window.windows(4).enumerate() {
            if signature == b"PK\x06\x06"
                || signature == b"PK\x06\x07"
                || (signature == b"PK\x05\x06" && start + offset as u64 != footer_position)
            {
                return Err(failed("Ambiguous ZIP footer or embedded ZIP64 signature"));
            }
        }
        scanned += count as u64;
        let keep = window.len().saturating_sub(3);
        window.drain(..keep);
    }
    file.seek(SeekFrom::Start(u64::from(central_start)))
        .map_err(PumasError::from)?;
    for _ in 0..count {
        let mut header = [0_u8; 46];
        file.read_exact(&mut header).map_err(PumasError::from)?;
        if &header[..4] != b"PK\x01\x02" {
            return Err(failed("Invalid bounded central directory"));
        }
        let word = |offset: usize| u16::from_le_bytes([header[offset], header[offset + 1]]);
        let name_size = word(28) as usize;
        let extra_size = word(30) as usize;
        let comment_size = word(32) as usize;
        if name_size == 0
            || name_size > 1024
            || extra_size > 4096
            || comment_size > 4096
            || word(34) != 0
            || [20, 24, 42]
                .iter()
                .any(|offset| header[*offset..*offset + 4] == [255; 4])
        {
            return Err(failed("Unsupported bounded ZIP member metadata"));
        }
        let mut name = vec![0_u8; name_size];
        file.read_exact(&mut name).map_err(PumasError::from)?;
        let name = std::str::from_utf8(&name).map_err(|_| failed("Invalid ZIP member encoding"))?;
        if !safe_member(name.strip_suffix('/').unwrap_or(name)) {
            return Err(failed("Invalid bounded ZIP member path"));
        }
        let mut extra = vec![0_u8; extra_size];
        file.read_exact(&mut extra).map_err(PumasError::from)?;
        let mut offset = 0;
        while offset < extra.len() {
            if offset + 4 > extra.len() {
                return Err(failed("Invalid ZIP extra metadata"));
            }
            let kind = u16::from_le_bytes([extra[offset], extra[offset + 1]]);
            let length = u16::from_le_bytes([extra[offset + 2], extra[offset + 3]]) as usize;
            offset += 4;
            if kind == 1 || offset + length > extra.len() {
                return Err(failed("ZIP64 or truncated extra metadata is unsupported"));
            }
            offset += length;
        }
        file.seek(SeekFrom::Current(comment_size as i64))
            .map_err(PumasError::from)?;
        if file.stream_position().map_err(PumasError::from)? > footer_position {
            return Err(failed("ZIP central directory escaped its bounds"));
        }
    }
    if file.stream_position().map_err(PumasError::from)? != footer_position {
        return Err(failed("ZIP entry count disagrees with central directory"));
    }
    file.seek(SeekFrom::Start(0)).map_err(PumasError::from)?;
    Ok(())
}

fn extract_component(
    mut wheel: File,
    packages: &Path,
    component: &crate::version_manager::TorchComponentManifest,
) -> Result<()> {
    preflight_zip(&mut wheel)?;
    let mut archive =
        zip::ZipArchive::new(wheel).map_err(|_| failed("Invalid component wheel archive"))?;
    if archive.is_empty() || archive.len() > 200_000 {
        return Err(failed("Component archive entry count exceeds bound"));
    }
    let expected: BTreeMap<_, _> = component
        .archive_files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let mut names = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|_| failed("Cannot inspect wheel entry"))?;
        let name = member.name().to_owned();
        let directory = member.is_dir();
        let path = if directory {
            name.strip_suffix('/').unwrap_or(&name)
        } else {
            &name
        };
        let mode = member.unix_mode().unwrap_or(0) & 0o170000;
        if !safe_member(path)
            || !names.insert(path.to_ascii_lowercase())
            || member.encrypted()
            || !matches!(
                member.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
            || (directory && mode != 0 && mode != 0o040000)
            || (!directory && mode != 0 && mode != 0o100000)
        {
            return Err(failed(
                "Wheel contains aliased, encrypted, linked or unsupported entries",
            ));
        }
        if directory {
            if member.size() != 0
                || !expected
                    .keys()
                    .any(|name| name.starts_with(&format!("{path}/")))
            {
                return Err(failed("Wheel contains an unselected directory"));
            }
            continue;
        }
        let expected = expected
            .get(name.as_str())
            .ok_or_else(|| failed("Wheel contains an unselected member"))?;
        total = total
            .checked_add(member.size())
            .ok_or_else(|| failed("Wheel expanded size overflow"))?;
        if member.size() != expected.size || total > 4 * 1024 * 1024 * 1024 {
            return Err(failed(
                "Wheel expanded bytes disagree with bounded manifest",
            ));
        }
        copy_exact(&mut member, &packages.join(&name), expected)?;
        seen.insert(name);
    }
    if seen.len() != expected.len() {
        return Err(failed("Wheel omitted a selected member"));
    }
    validate_record(packages, component)
}

fn stage_bundle(
    base: Arc<RetainedRuntimeReadSource>,
    component: crate::version_manager::TorchComponentManifest,
    identity: serde_json::Value,
    association: (String, String),
    mut wheel: File,
    expected_wheel: TorchComponentFile,
    staging: Arc<TorchPendingStage>,
) -> Result<PathBuf> {
    let (revision, digest) = association;
    base.validate().map_err(PumasError::from)?;
    let private_wheel = staging.path().join("selected-component.whl");
    copy_exact(&mut wheel, &private_wheel, &expected_wheel)?;
    let wheel = File::open(private_wheel).map_err(PumasError::from)?;
    let runtime = staging.path().join("runtime");
    std::fs::create_dir(&runtime).map_err(PumasError::from)?;
    let packages = runtime.join("venv/lib/python3.12/site-packages");
    std::fs::create_dir_all(&packages).map_err(PumasError::from)?;
    let replacements: BTreeSet<_> = component
        .replace_base_members
        .iter()
        .map(String::as_str)
        .collect();
    for (role, file) in base.manifest() {
        let output = match role {
            RuntimeReadRole::Interpreter => continue,
            RuntimeReadRole::Dependencies if replacements.contains(file.path()) => continue,
            RuntimeReadRole::Dependencies => packages.join(file.path()),
            RuntimeReadRole::Sidecar
                if matches!(
                    file.path(),
                    "probe-results.json" | "component-manifest.json"
                ) =>
            {
                continue
            }
            RuntimeReadRole::Sidecar => runtime.join(file.path()),
        };
        if !safe_member(file.path()) {
            return Err(failed("Base has an unsupported selected path"));
        }
        let mut input = base
            .clone_member(role, file.path())
            .map_err(PumasError::from)?;
        copy_exact(
            &mut input,
            &output,
            &TorchComponentFile {
                path: file.path().into(),
                size: file.size(),
                sha256: file.sha256().into(),
            },
        )?;
    }
    let mut recipe: serde_json::Value =
        serde_json::from_str(&bounded_text(&runtime.join("runtime.json"), 1024 * 1024)?)
            .map_err(|_| failed("Invalid selected base recipe"))?;
    if recipe["python"] != component.python
        || recipe["managed_python"]["executable"]["path"]
            .as_str()
            .is_none()
    {
        return Err(failed(
            "Selected base interpreter does not match component cohort",
        ));
    }
    extract_component(wheel, &packages, &component)?;
    let installed = StagedFilesManifest {
        files: component
            .final_dependency_files
            .iter()
            .map(|file| StagedFile {
                path: file.path.clone(),
                size: file.size,
                sha256: file.sha256.clone(),
            })
            .collect(),
    };
    validate_staged_files(&packages, &installed)?;
    std::fs::write(
        runtime.join("installed-files.json"),
        serde_json::to_vec(&serde_json::json!({"files":component.final_dependency_files}))
            .map_err(|e| failed(e.to_string()))?,
    )
    .map_err(PumasError::from)?;
    recipe["recipe_id"] = serde_json::json!("pumas-component-assembly-v1");
    recipe["assembly_only"] = serde_json::json!(true);
    recipe["qualification"] = serde_json::json!("assembled_unqualified");
    recipe["component_revision"] = serde_json::json!(revision);
    recipe["component_manifest_sha256"] = serde_json::json!(digest);
    let recipe_bytes = serde_json::to_vec(&recipe).map_err(|e| failed(e.to_string()))?;
    if recipe_bytes.len() > 1024 * 1024 {
        return Err(failed("Component recipe exceeds retained reader bound"));
    }
    std::fs::write(runtime.join("runtime.json"), recipe_bytes).map_err(PumasError::from)?;
    std::fs::write(
        runtime.join("component-manifest.json"),
        serde_json::to_vec(&identity).map_err(|e| failed(e.to_string()))?,
    )
    .map_err(PumasError::from)?;
    crate::version_manager::torch_read_source::validate_component_stage_namespace(&runtime)?;
    // Deliberately no venv/bin/python, pyvenv.cfg, bootstrap or executable alias.
    // A future registered consumer chooses approved startup configuration.
    base.validate().map_err(PumasError::from)?;
    Ok(runtime)
}

impl VersionInstaller {
    pub(super) async fn stage_component_runtime(
        &self,
        plan: &TorchComponentPlan,
        staging: &Arc<TorchPendingStage>,
    ) -> Result<PathBuf> {
        let consumer = self
            .acquisition_consumer
            .as_ref()
            .ok_or_else(|| failed("Component acquisition owner missing"))?;
        let mut request = plan
            .input
            .lock()
            .map_err(|_| failed("Component input poisoned"))?
            .take()
            .ok_or_else(|| failed("Component input was already consumed"))?;
        request.demand.operation = format!(
            "torch-component-prepared:{}:{}",
            plan.manifest_sha256,
            staging
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| failed("Component stage identity absent"))?
        );
        let file = &request.manifest.files()[0];
        let expected_wheel = TorchComponentFile {
            path: file.logical_path().into(),
            size: file
                .expected_size()
                .ok_or_else(|| failed("Wheel size evidence lost"))?,
            sha256: file
                .expected_sha256()
                .ok_or_else(|| failed("Wheel digest evidence lost"))?
                .value()
                .into(),
        };
        *staging
            ._component_base
            .lock()
            .map_err(|_| failed("Component stage custody poisoned"))? = Some(plan.base.clone());
        let base = plan.base.clone();
        let component = plan.component.clone();
        let identity = plan.identity.clone();
        let revision = plan.revision_tag.clone();
        let digest = plan.manifest_sha256.clone();
        let stage = staging.clone();
        let cleanup_stage = stage.clone();
        let expected_digest = digest.clone();
        consumer.acquire_local(request,Box::new(ComponentHost{cancel:self.cancel_flag.clone(),shutdown:self.shutdown_flag.clone()}),
            move |use_handle| async move {
                let wheel = use_handle.open_file(0).await?;
                let payload = serde_json::json!({"phase":"component_bytes_prepared","manifest_sha256":digest});
                let outcome = use_handle.run_blocking("assemble verified component bytes",move || Ok(stage_bundle(base,component,identity,(revision,digest),wheel,expected_wheel,stage))).await?;
                match outcome {
                    Ok(runtime) => Ok((runtime,payload)),
                    Err(error) => {
                        let cleanup = use_handle.withdraw_after_cleanup(move || {
                            let _stage = cleanup_stage;
                            match std::fs::remove_dir_all(_stage.path().join("runtime")) {
                                Ok(()) => Ok(()),
                                Err(error) if error.kind()==std::io::ErrorKind::NotFound => Ok(()),
                                Err(error) => Err(PumasError::from(error)),
                            }
                        }).await;
                        match cleanup {Ok(())=>Err(error),Err(cleanup)=>Err(failed(format!("{error}; component input withdrawal unsettled: {cleanup}")))}
                    }
                }
            },
            move |runtime,receipt| async move {
                if receipt.payload["phase"]!="component_bytes_prepared" || receipt.payload["manifest_sha256"]!=expected_digest {
                    return Err(failed("Component prepared receipt identity mismatch"));
                }
                Ok(runtime)
            }).await
    }
}
