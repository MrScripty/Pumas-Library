//! UniFFI bindings for pumas-core.
//!
//! This crate is an experimental UniFFI adapter over the pumas-core library.
//!
//! # Support status
//!
//! No host-language binding is supported. `bindings/support-matrix.json`
//! accepts no host tuple and lists every generator below as an unsupported
//! surface pending removal; see `docs/native-bindings.md`. The generators that
//! exist for local experiments are:
//!
//! - **Python, Kotlin, Swift, Ruby** - the in-tree `pumas-uniffi-bindgen`,
//!   built from the workspace's locked `uniffi` 0.28 version
//! - **C#** - `uniffi-bindgen-cs` v0.9.0+v0.28.3
//!
//! # Usage
//!
//! Generate bindings using `--library` mode:
//!
//! ```bash
//! # Build the cdylib
//! cargo build --locked -p pumas-uniffi --release
//!
//! # Generate Python bindings
//! cargo run --locked -p pumas-uniffi --features cli --bin pumas-uniffi-bindgen -- \
//!     generate --library --language python \
//!     --out-dir ./bindings/python target/release/libpumas_uniffi.so
//!
//! # Generate C# bindings
//! uniffi-bindgen-cs --library --config crates/pumas-uniffi/uniffi.toml \
//!     --out-dir ./bindings/csharp target/release/libpumas_uniffi.so
//! ```

// The entire bindings implementation is gated behind the "bindings" feature
// so that the CLI binary (`pumas-uniffi-bindgen`) can be built without
// compiling pumas-library and all its transitive dependencies.
#[cfg(feature = "bindings")]
mod bindings;

#[cfg(feature = "bindings")]
pub use bindings::*;
