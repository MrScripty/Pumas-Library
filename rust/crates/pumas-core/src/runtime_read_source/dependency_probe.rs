//! Explicit non-model qualification driver. No shipping policy is modified.
//! Only the fixed source-pinned interpreter/dependency/native byte cohort can
//! reach the constant probe program. No caller program/model/path is accepted.
use super::{RetainedRuntimeReadSource, RuntimeReadRole};
use crate::platform::audio_read_boundary::{AudioReadBoundary, AudioReadGrant};
use crate::platform::managed_child::{ManagedChild, ManagedChildCustodySlot};
use serde_json::Value;
#[cfg(test)]
use sha2::{Digest, Sha256};
use std::io;
use std::os::fd::AsRawFd;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;

const INTERPRETER: &str = "cpython-3.12.14-linux-x86_64-gnu/bin/python3.12";
const LOADER: &str = "usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2";
const SCRIPT: &str = r#"
import sys, json, os, socket, threading
sys.path.insert(0, '/proc/self/fd/' + sys.argv[1])
canary = sys.argv[2]
def denied_read():
    try:
        with open(canary, 'rb') as source: source.read(1)
    except PermissionError: return
    raise RuntimeError('unselected read was not denied')
denied_read()
try:
    with open(canary + '.write', 'wb') as target: target.write(b'x')
except PermissionError: pass
else: raise RuntimeError('unselected write was not denied')
try: socket.socket(socket.AF_INET, socket.SOCK_STREAM)
except PermissionError: pass
else: raise RuntimeError('ambient socket was not denied')
thread_errors = []
def in_thread():
    try: denied_read()
    except BaseException as error: thread_errors.append(type(error).__name__)
thread = threading.Thread(target=in_thread)
thread.start(); thread.join()
assert not thread_errors, thread_errors
cache_root = '/proc/self/fd/' + sys.argv[1]
assert os.environ['TORCHINDUCTOR_CACHE_DIR'] == cache_root
try:
    with open(cache_root + '/pumas-forbidden-cache-write', 'xb') as target: target.write(b'x')
except PermissionError: pass
else: raise RuntimeError('compiler cache write was not denied')
import torch
# Import first so a failed initial Dynamo import is not masked by a later
# duplicate cache-artifact registration during Transformers lazy imports.
import torch._dynamo
import transformers, numpy, scipy, librosa, soundfile, soxr
from transformers import CohereAsrConfig, CohereAsrProcessor, CohereAsrForConditionalGeneration
assert torch.__version__ == '2.10.0+cpu'
assert transformers.__version__ == '5.4.0'
assert torch.ones(4, device='cpu').sum().item() == 4.0
print(json.dumps({'status':'passed','torch':torch.__version__, 'transformers':transformers.__version__, 'prefix':sys.prefix, 'scope':'confined dependency imports and CPU operation only; no model or ASR'}), flush=True)
"#;
fn error(message: &str) -> io::Error {
    io::Error::other(message)
}
fn validate(source: &RetainedRuntimeReadSource) -> io::Result<()> {
    for role in [
        RuntimeReadRole::Interpreter,
        RuntimeReadRole::Dependencies,
        RuntimeReadRole::NativeLibraries,
    ] {
        super::validate_audio_candidate_read_role(source, role)?;
    }
    source.validate()
}
fn path(file: &std::fs::File) -> String {
    format!("/proc/self/fd/{}", file.as_raw_fd())
}
struct Lease {
    _source: Arc<RetainedRuntimeReadSource>,
    _canary: Arc<tempfile::TempDir>,
}
struct ProbeGuard {
    child: Option<ManagedChild>,
    custody: Arc<ManagedChildCustodySlot>,
}
impl Drop for ProbeGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            drop(child); // Parks all original byte owners before cleanup scheduling.
            let custody = self.custody.clone();
            // If thread admission fails, the original slot retains uncertainty.
            let _ = std::thread::Builder::new()
                .name("audio-probe-drain".into())
                .spawn(move || {
                    let _ = custody.drain(Duration::from_secs(15));
                });
        }
    }
}
fn spawn(source: Arc<RetainedRuntimeReadSource>) -> io::Result<ProbeGuard> {
    AudioReadBoundary::supported_abi()?;
    validate(&source)?;
    let canary = Arc::new(
        tempfile::Builder::new()
            .prefix("pumas-audio-read-canary-")
            .tempdir()?,
    );
    let canary_path = canary.path().join("unselected");
    std::fs::write(&canary_path, b"unselected controlled test bytes")?;
    let interpreter = source.clone_member(RuntimeReadRole::Interpreter, INTERPRETER)?;
    let loader = source.clone_member(RuntimeReadRole::NativeLibraries, LOADER)?;
    let native = source.clone_root(RuntimeReadRole::NativeLibraries)?;
    let packages = source.clone_root(RuntimeReadRole::Dependencies)?;
    let cache_root = path(&packages);
    let mut command = Command::new(path(&loader));
    command
        .args(["--inhibit-cache", "--library-path"])
        .arg(format!("{}/usr/lib/x86_64-linux-gnu", path(&native)))
        .arg("--argv0")
        .arg(path(&interpreter))
        .arg(path(&interpreter))
        .args(["-I", "-S", "-B", "-X", "utf8", "-c", SCRIPT])
        .arg(packages.as_raw_fd().to_string())
        .arg(&canary_path);
    let directories = source
        .directory_manifest()
        .filter(|(role, _)| *role != RuntimeReadRole::Sidecar)
        .map(|(role, name)| {
            source
                .clone_directory(role, name)
                .and_then(AudioReadGrant::directory)
        });
    let files = source
        .manifest()
        .filter(|(role, _)| *role != RuntimeReadRole::Sidecar)
        .map(|(role, member)| {
            source.clone_member(role, member.path()).and_then(|file| {
                AudioReadGrant::file(
                    file,
                    role == RuntimeReadRole::NativeLibraries && member.path() == LOADER,
                )
            })
        });
    let boundary = AudioReadBoundary::prepare(
        directories
            .chain(files)
            .chain(std::iter::once(AudioReadGrant::kernel_entropy())),
        vec![interpreter, loader, native, packages],
    )?;
    validate(&source)?;
    boundary.confine_command(&mut command);
    // Import-time Dynamo cache discovery requires an existing directory. Bind
    // its documented setting to a held read-only capability; all cache writes
    // remain denied, and no ambient temporary-directory discovery is needed.
    command.env("TORCHINDUCTOR_CACHE_DIR", cache_root);
    let custody = ManagedChildCustodySlot::new();
    let lease = Arc::new(Lease {
        _source: source,
        _canary: canary,
    });
    let mut child = ManagedChild::spawn(&mut command, custody.clone())?;
    child.attach_cleanup_lease(lease);
    Ok(ProbeGuard {
        child: Some(child),
        custody,
    })
}

pub(super) async fn run(source: Arc<RetainedRuntimeReadSource>) -> io::Result<Value> {
    // The blocking task retains its guard even if the awaiting caller disappears.
    let mut guard = tokio::task::spawn_blocking(move || spawn(source))
        .await
        .map_err(io::Error::other)??;
    let child = guard
        .child
        .as_mut()
        .ok_or_else(|| error("probe child unavailable"))?;
    let (input, output) = child.take_private_stdio()?;
    drop(input);
    let diagnostics = child.take_private_stderr()?;
    let mut output = tokio::process::ChildStdout::from_std(output)?.take(32769);
    let mut diagnostics = tokio::process::ChildStderr::from_std(diagnostics)?.take(32769);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let observed = tokio::time::timeout(Duration::from_secs(120), async {
        tokio::try_join!(
            output.read_to_end(&mut stdout),
            diagnostics.read_to_end(&mut stderr)
        )
    })
    .await;
    let custody = guard.custody.clone();
    let status = tokio::task::spawn_blocking(move || {
        // Keep the whole guard here: any observation/drain error must still
        // park the child and schedule the bounded custody retry.
        let child = guard
            .child
            .as_mut()
            .ok_or_else(|| error("probe child unavailable"))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let voluntary = loop {
            if let Some(status) = child.observe_exit()? {
                break Some(status);
            }
            if std::time::Instant::now() >= deadline {
                break None;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        child.terminate_and_drain(Duration::from_secs(15))?;
        drop(guard.child.take()); // Confirmed drain, so no retry remains necessary.
        voluntary.ok_or_else(|| error("probe did not exit voluntarily"))
    })
    .await
    .map_err(io::Error::other)??;
    if custody.is_active() || custody.has_parked_child() || custody.is_cleanup_pending() {
        return Err(error("probe child cleanup is unconfirmed"));
    }
    observed.map_err(|_| error("probe read deadline exceeded"))??;
    if stdout.len() > 32768 || stderr.len() > 32768 {
        return Err(error("probe output exceeded bound"));
    }
    if !status.success() {
        return Err(io::Error::other(format!(
            "confined dependency probe failed: {}",
            String::from_utf8_lossy(&stderr)
        )));
    }
    let mut report: Value = serde_json::from_slice(&stdout).map_err(io::Error::other)?;
    if report["status"] != "passed" {
        return Err(error("probe result was not passed"));
    }
    report["child_tree_drained"] = Value::Bool(true);
    report["production_available"] = Value::Bool(false);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::capability_fs::open_pinned_directory;
    use crate::runtime_read_source::{RuntimeReadFile, RuntimeReadRoot};

    #[test]
    fn arbitrary_retained_files_cannot_reach_the_fixed_probe() {
        let root = Arc::new(tempfile::tempdir().unwrap());
        let mut selections = Vec::new();
        for (name, role) in [
            ("interpreter", RuntimeReadRole::Interpreter),
            ("dependencies", RuntimeReadRole::Dependencies),
            ("native", RuntimeReadRole::NativeLibraries),
        ] {
            let directory = root.path().join(name);
            std::fs::create_dir(&directory).unwrap();
            std::fs::write(directory.join("candidate"), b"untrusted fixture").unwrap();
            selections.push(
                RuntimeReadRoot::new(
                    role,
                    open_pinned_directory(&directory).unwrap().into_std_file(),
                    vec![RuntimeReadFile::new(
                        "candidate".into(),
                        17,
                        format!("{:x}", Sha256::digest(b"untrusted fixture")),
                    )
                    .unwrap()],
                    vec![],
                    root.clone(),
                )
                .unwrap(),
            );
        }
        let retained = RetainedRuntimeReadSource::capture(selections).unwrap();
        retained.validate().unwrap();
        assert!(validate(&retained).is_err());
    }
}
