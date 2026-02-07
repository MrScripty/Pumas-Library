/// Custom uniffi-bindgen binary for generating language bindings.
///
/// This binary uses the UniFFI bindgen API to generate Python/Kotlin/Swift
/// bindings from the compiled pumas-uniffi cdylib.
///
/// Usage:
///   cargo run -p pumas-uniffi --bin pumas-uniffi-bindgen -- \
///     generate --library -l python -o bindings/python \
///     target/release/libpumas_uniffi.so
fn main() {
    uniffi::uniffi_bindgen_main();
}
