//! Production RPC subprocess + owned HTTPS objects; no provider account/inference.
#![cfg(all(
    feature = "s3",
    not(feature = "inference-plugins"),
    target_os = "linux"
))]
#[test]
fn conditional_single_object_uses_existing_rpc_owners() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../scripts/tests/qualify-s3-conditional-rpc.py");
    let output = std::process::Command::new("python3")
        .arg(script)
        .arg("--rpc")
        .arg(env!("CARGO_BIN_EXE_pumas-rpc"))
        .output()
        .expect("owned process qualification must launch");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}
