# Runtime Profile Support Modules

## Purpose

This directory contains focused support modules for the provider-neutral runtime
profile service in `runtime_profiles.rs`.

`launch_strategy.rs` owns the typed managed launch strategy contract used by
runtime profile launch-spec derivation and lifecycle code. `launch_specs.rs`
owns managed launch-spec derivation, implicit port allocation, process
environment, and existing provider launch arguments. Launch-spec derivation
consumes the typed launch strategy for process environment and argument
selection. The contract separates binary-process, Python-sidecar, and
external-only profile intent so new providers can be added without encoding
another provider match in launch callers.

## API Consumers

- `runtime_profiles.rs` projects provider behavior launch targets into
  `RuntimeProfileLaunchStrategy` values for managed launch specs.
- `api/state_runtime_profiles.rs` consumes the typed strategy when converting a
  launch spec into a concrete process launch config.

## Structured Producer Contract

Launch strategy values are Rust DTOs with `serde` support and snake_case wire
names. Tests cover the current Ollama and llama.cpp mappings plus the
external-only strategy.

## Lifecycle

Existing managed Ollama and llama.cpp profiles map to binary process
strategies. The ONNX Runtime provider will use the Python sidecar strategy in a
later slice.

## Errors

Provider/mode combinations that cannot produce a launch strategy return
structured `InvalidParams` errors before process launch.

## Compatibility

The strategy layer preserves existing Ollama and llama.cpp launch behavior. It
does not change persisted runtime profile schema.

## Revisit Trigger

Revisit this README when ONNX Runtime sidecar launch wiring, shutdown ownership,
or additional launch kinds are added.
