# Runtime Profile Support Modules

## Purpose

This directory contains focused support modules for the provider-neutral runtime
profile service in `runtime_profiles.rs`.

`launch_strategy.rs` owns the typed managed launch strategy contract used by
runtime profile launch-spec derivation and lifecycle code. `launch_specs.rs`
owns managed launch-spec derivation, implicit port allocation, process
environment, and existing provider launch arguments. Launch-spec derivation
consumes the typed launch strategy for process environment and argument
selection. The contract separates binary-process, in-process runtime, and
external-only profile intent so new providers can be added without encoding
another provider match in launch callers.

`route_config.rs` owns runtime-profile config initialization, one-way legacy
route migration, and model-route validation. The service consumes this module
as a persistence boundary so provider-scoped route cleanup does not live inside
runtime-profile orchestration.

## API Consumers

- `runtime_profiles.rs` projects provider behavior launch targets into
  `RuntimeProfileLaunchStrategy` values for managed launch specs.
- `api/state_runtime_profiles.rs` consumes the typed strategy when converting a
  launch spec into a concrete process launch config.
- `runtime_profiles.rs` delegates persisted config loading and route validation
  to `route_config.rs`.

## Structured Producer Contract

Launch strategy values are Rust DTOs with `serde` support and snake_case wire
names. Tests cover the current Ollama and llama.cpp mappings plus the
ONNX in-process runtime and external-only strategies.

Runtime profile config files are persisted as JSON through the metadata atomic
read/write helpers. Legacy schema-1 model-only routes are rewritten once into
provider-scoped routes only when the referenced profile identifies the provider;
ambiguous legacy routes are dropped.

## Lifecycle

Existing managed Ollama and llama.cpp profiles map to binary process
strategies. The ONNX Runtime provider maps to an in-process runtime strategy;
session-manager construction and lifecycle ownership are wired in later ONNX
slices.

## Errors

Provider/mode combinations that cannot produce a launch strategy return
structured `InvalidParams` errors before process launch.

## Compatibility

The strategy layer preserves existing Ollama and llama.cpp launch behavior. It
does not change persisted runtime profile schema.

Route config migration preserves schema-2 provider-scoped route shape and does
not reintroduce a dual old/new route reader.

## Revisit Trigger

Revisit this README when ONNX Runtime session-manager lifecycle, shutdown
ownership, or additional launch kinds are added.
