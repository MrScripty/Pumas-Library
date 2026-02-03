fn main() {
    // UniFFI build script - generates bindings scaffolding
    uniffi::generate_scaffolding("src/pumas.udl").unwrap();
}
