fn main() {
    // No-op: all types use proc-macro annotations (#[derive(uniffi::Record)], etc.)
    // so UDL-based scaffolding is not needed. The uniffi::setup_scaffolding!() macro
    // in lib.rs handles the FFI glue code generation.
}
