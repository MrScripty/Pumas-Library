//! Desktop launch contract: an OS-assigned port reported in one structured
//! readiness line, a launch nonce echoed by `health_check`, and a normal drain
//! when the parent closes stdin (only with `--exit-on-stdin-eof`).
#![cfg(target_os = "linux")]
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct Server {
    child: Child,
    ready: Value,
    _temp: tempfile::TempDir,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start an owned server on port 0 and wait for its `PUMAS_RPC_READY=` line.
fn start(extra: &[&str], nonce: Option<&str>) -> Server {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_pumas-rpc"));
    command
        .args(["--port", "0", "--launcher-root"])
        .arg(&root)
        .args(extra)
        .env("PUMAS_REGISTRY_DB_PATH", temp.path().join("registry.db"))
        .env("ORT_SKIP_DOWNLOAD", "1")
        .env("TMPDIR", temp.path())
        .env_remove("PUMAS_RPC_LAUNCH_NONCE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(nonce) = nonce {
        command.env("PUMAS_RPC_LAUNCH_NONCE", nonce);
    }
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (lines, received) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let ready = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match received.recv_timeout(remaining) {
            Ok(line) => {
                if let Some(json) = line.strip_prefix("PUMAS_RPC_READY=") {
                    break serde_json::from_str(json).unwrap();
                }
            }
            Err(_) => panic!("server did not report readiness"),
        }
    };
    Server {
        child,
        ready,
        _temp: temp,
    }
}

fn health_check(port: u16) -> Value {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"health_check","params":{}}"#;
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(
        socket,
        "POST /rpc HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut reply = String::new();
    socket.read_to_string(&mut reply).unwrap();
    let (head, body) = reply.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    serde_json::from_str(body).unwrap()
}

fn wait(server: &mut Server, within: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(status) = server.child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn ready_line_reports_the_bound_port_and_health_echoes_the_launch_nonce() {
    let server = start(&[], Some("desktop-launch-7f3a"));
    assert_eq!(server.ready["version"], 1);
    assert_eq!(server.ready["nonce"], "desktop-launch-7f3a");
    let port = u16::try_from(server.ready["port"].as_u64().unwrap()).unwrap();
    assert_ne!(port, 0);
    let health = health_check(port);
    assert_eq!(
        health["result"],
        serde_json::json!({"status": "ok", "launch_nonce": "desktop-launch-7f3a"})
    );

    let unnamed = start(&[], None);
    assert!(unnamed.ready["nonce"].is_null());
    let port = u16::try_from(unnamed.ready["port"].as_u64().unwrap()).unwrap();
    assert_eq!(
        health_check(port)["result"],
        serde_json::json!({"status": "ok"})
    );
}

#[test]
fn stdin_eof_drains_and_exits_only_with_the_desktop_flag() {
    let mut flagged = start(&["--exit-on-stdin-eof"], None);
    drop(flagged.child.stdin.take());
    let status = wait(&mut flagged, Duration::from_secs(30))
        .expect("stdin EOF must stop a server started with --exit-on-stdin-eof");
    assert!(status.success(), "normal drain must succeed: {status:?}");

    let mut unflagged = start(&[], None);
    drop(unflagged.child.stdin.take());
    assert!(
        wait(&mut unflagged, Duration::from_secs(2)).is_none(),
        "without the flag, closing stdin must not stop the server"
    );
    let terminated = Command::new("kill")
        .args(["-TERM", &unflagged.child.id().to_string()])
        .status()
        .unwrap();
    assert!(terminated.success());
    let status = wait(&mut unflagged, Duration::from_secs(30)).expect("SIGTERM must still stop it");
    assert!(status.success(), "{status:?}");
}
