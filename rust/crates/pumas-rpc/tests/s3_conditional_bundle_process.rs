//! Actual production process with authored HTTPS packages; no provider/inference.
#![cfg(all(
    feature = "s3",
    not(feature = "inference-plugins"),
    target_os = "linux"
))]
#[test]
fn conditional_authored_bundles_use_existing_rpc_owners() {
    let output = std::process::Command::new("python3")
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../scripts/tests/qualify-s3-authored-conditional-rpc.py"),
        )
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
