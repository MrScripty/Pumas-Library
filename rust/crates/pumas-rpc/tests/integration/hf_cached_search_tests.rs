//! Existing launched RPC contract, owned cache records and rejecting loopback upstream.
#[test]
fn launched_rpc_preserves_cached_provenance_and_live_acquisition_boundary() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../scripts/tests/qualify-hf-cached-search-ui.py");
    let output = std::process::Command::new("python3")
        .arg(script)
        .arg("--rpc")
        .arg(env!("CARGO_BIN_EXE_pumas-rpc"))
        .arg("--only-rpc")
        .output()
        .expect("owned qualification helper must launch");
    assert!(
        output.status.success(),
        "RPC qualification failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
