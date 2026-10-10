//! Source-pinned selected-byte checks, including the one UV-relocated CPython
//! source file. Relocation validation never changes or omits executable bytes.
use super::{RetainedRuntimeReadSource, RuntimeReadRole, AUDIO_RUNTIME_CANDIDATE_RECIPE};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io;

fn refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "selected bytes differ from fixed audio recipe",
    )
}

#[cfg(target_os = "linux")]
fn relocation(source: &RetainedRuntimeReadSource, pin: &Value) -> io::Result<(u64, String)> {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    let member = pin["member"].as_str().ok_or_else(refused)?;
    let distribution = pin["distribution"].as_str().ok_or_else(refused)?;
    let root = source.clone_root(RuntimeReadRole::Interpreter)?;
    let held = std::fs::read_link(format!("/proc/self/fd/{}", root.as_raw_fd()))?;
    let prefix = held.join(distribution);
    let prefix = prefix.to_str().ok_or_else(refused)?;
    let quoted = serde_json::to_string(prefix).map_err(io::Error::other)?;
    let escaped = &quoted[1..quoted.len() - 1];
    let mut bytes = Vec::new();
    source
        .clone_member(RuntimeReadRole::Interpreter, member)?
        .take(131073)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 131072 {
        return Err(refused());
    }
    let data = std::str::from_utf8(&bytes).map_err(|_| refused())?;
    validate_relocation_text(data, escaped, pin)
}

#[cfg(any(target_os = "linux", test))]
fn validate_relocation_text(data: &str, escaped: &str, pin: &Value) -> io::Result<(u64, String)> {
    // A filesystem name is not automatically safe Python literal text. This
    // bounded recipe supports only ASCII path characters that cannot terminate
    // either quote style or introduce escapes/control characters.
    if !escaped.starts_with('/')
        || !escaped
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
    {
        return Err(refused());
    }
    let expected = pin["occurrences"].as_u64().ok_or_else(refused)?;
    if escaped.is_empty() || data.matches(escaped).count() as u64 != expected {
        return Err(refused());
    }
    // The exact normalized hash fixes every byte surrounding all 27 prefix
    // occurrences. Their positions are therefore fixed inside quoted strings;
    // no arbitrary code or caller-provided substitution template is accepted.
    let normalized = data.replace(escaped, "@PUMAS_MANAGED_CPYTHON_PREFIX@");
    let size = normalized.len() as u64;
    let sha = format!("{:x}", Sha256::digest(normalized.as_bytes()));
    if Some(size) != pin["size"].as_u64() || Some(sha.as_str()) != pin["sha256"].as_str() {
        return Err(refused());
    }
    Ok((size, sha))
}

#[cfg(not(target_os = "linux"))]
fn relocation(_: &RetainedRuntimeReadSource, _: &Value) -> io::Result<(u64, String)> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "managed audio recipe requires Linux",
    ))
}

pub(super) fn validate(
    source: &RetainedRuntimeReadSource,
    role: RuntimeReadRole,
) -> io::Result<()> {
    let pin: Value =
        serde_json::from_str(AUDIO_RUNTIME_CANDIDATE_RECIPE).map_err(io::Error::other)?;
    let key = match role {
        RuntimeReadRole::Interpreter => "interpreter_selection",
        RuntimeReadRole::Dependencies => "dependency_selection",
        RuntimeReadRole::NativeLibraries => "native_selection",
        RuntimeReadRole::Sidecar => return Err(refused()),
    };
    let mut members = source
        .manifest()
        .filter(|(selected, _)| *selected == role)
        .map(|(_, member)| {
            (
                member.path().to_owned(),
                member.size(),
                member.sha256().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    if Some(members.len() as u64) != pin[key]["members"].as_u64() {
        return Err(refused());
    }
    if role == RuntimeReadRole::Interpreter {
        let selected = pin["interpreter_relocation"]["member"]
            .as_str()
            .ok_or_else(refused)?;
        let member = members
            .iter_mut()
            .find(|member| member.0 == selected)
            .ok_or_else(refused)?;
        let (size, sha) = relocation(source, &pin["interpreter_relocation"])?;
        member.1 = size;
        member.2 = sha;
    }
    members.sort();
    let mut hash = Sha256::new();
    hash.update(b"pumas-audio-runtime-recipe-v1\0");
    for (name, size, sha) in members {
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update(size.to_be_bytes());
        hash.update(sha.as_bytes());
    }
    if Some(format!("{:x}", hash.finalize()).as_str()) != pin[key]["sha256"].as_str() {
        return Err(refused());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relocation_comparison_refuses_changed_code_count_and_forged_prefix() {
        let normalized = "build_time_vars = {\"prefix\": \"@PUMAS_MANAGED_CPYTHON_PREFIX@\"}\n";
        let pin = serde_json::json!({"occurrences":1, "size": normalized.len(),
            "sha256": format!("{:x}", Sha256::digest(normalized.as_bytes()))});
        for prefix in [
            "/root/with'quote",
            "/root/with\"quote",
            "/root/with\\escape",
            "/root/with\ncontrol",
            "/root/'+str(__import__('os').getpid())+'",
        ] {
            let data = normalized.replace("@PUMAS_MANAGED_CPYTHON_PREFIX@", prefix);
            assert!(validate_relocation_text(&data, prefix, &pin).is_err());
        }
        for prefix in ["/first/root", "/second/root"] {
            let data = normalized.replace("@PUMAS_MANAGED_CPYTHON_PREFIX@", prefix);
            assert!(validate_relocation_text(&data, prefix, &pin).is_ok());
            assert!(validate_relocation_text(
                &data.replace("build_time_vars", "execute_payload"),
                prefix,
                &pin
            )
            .is_err());
            assert!(validate_relocation_text(&(data.clone() + prefix), prefix, &pin).is_err());
            assert!(
                validate_relocation_text(&data.replace(prefix, "/forged"), prefix, &pin).is_err()
            );
            assert!(validate_relocation_text(&data, "/caller/forged", &pin).is_err());
        }
    }
}
