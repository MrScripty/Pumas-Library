# pumas-uniffi

## Purpose
`pumas-uniffi` exposes selected Pumas Library operations through UniFFI-generated language bindings.

## Ownership
This crate owns generated binding configuration, FFI-safe DTOs, async runtime bridging, and compatibility expectations for UniFFI consumers. Core domain logic remains in `pumas-library`.

## Producer Contract
Every exported API must have an explicit support tier before broadening the binding surface. Host-facing strings, paths, and JSON payloads must be validated before reaching core services.

## Consumer Contract
Binding consumers should use generated packages and avoid depending on Rust-internal DTO layout. Compatibility is governed by the artifact contract and the support tier assigned to each API.

## Testing Contract
Binding smoke tests should validate artifact generation and a minimal host call path. Domain correctness tests belong in `pumas-library`.

## Non-Goals
None. Reason: this crate is a host-language boundary. Revisit trigger: introduce a separate bindings-core crate.
