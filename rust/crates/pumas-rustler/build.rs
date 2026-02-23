fn main() {
    // NIF symbols (_enif_*) are resolved at runtime when the BEAM VM loads the
    // shared library.  macOS's linker errors on unresolved symbols by default,
    // so we tell it to allow them.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rustc-link-arg=-undefined");
        println!("cargo:rustc-link-arg=dynamic_lookup");
    }
}
