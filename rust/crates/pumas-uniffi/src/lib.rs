//! UniFFI bindings for pumas-core.
//!
//! This crate provides cross-language bindings for the pumas-core library,
//! enabling native access from Python, C#, Swift, Kotlin, Go, and Ruby.
//!
//! # Supported Languages
//!
//! - **Python** - Official UniFFI support
//! - **C#** - Via uniffi-bindgen-cs
//! - **Kotlin** - Official UniFFI support
//! - **Swift** - Official UniFFI support
//! - **Ruby** - Official UniFFI support
//! - **Go** - Via uniffi-bindgen-go
//!
//! # Usage
//!
//! Generate bindings using `--library` mode:
//!
//! ```bash
//! # Build the cdylib
//! cargo build -p pumas-uniffi --release
//!
//! # Generate Python bindings
//! pumas-uniffi-bindgen generate --library --language python \
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
