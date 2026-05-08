# Bindings

## Purpose
This directory contains generated or host-language binding artifacts and smoke harnesses for non-Rust consumers.

## Ownership
Rust binding generation is owned by `rust/crates/pumas-uniffi` and `rust/crates/pumas-rustler`. Host-language packaging and smoke checks live under language-specific subdirectories here.

## Producer Contract
Generated artifacts must identify the native library version, generation tool, target platform, and compatibility tier before release.

## Consumer Contract
Host-language consumers should use language-specific packages or smoke harnesses rather than reaching into Rust workspace internals.

## Non-Goals
Core domain behavior is out of scope. Reason: bindings are projection surfaces over `pumas-library`. Revisit trigger: add a host-language implementation that owns durable state.

User-directed model serving is currently a desktop/RPC implementation surface.
Serving APIs should not be added to generated host-language bindings until a
supported binding consumer exists and native plus host-language verification can
land in the same slice.
