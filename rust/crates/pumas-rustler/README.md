# pumas-rustler

## Purpose
`pumas_rustler` exposes selected Pumas Library capabilities as Rustler NIFs for BEAM hosts.

## Ownership
This crate owns BEAM-safe wrapper functions, NIF registration, and conversion between Rust errors and host-visible responses. Core behavior remains in `pumas-library`.

## Producer Contract
NIF exports must avoid long blocking work on scheduler threads unless explicitly moved to a safe execution model. Host-facing payloads must be validated before reaching core services.

## Consumer Contract
Elixir and Erlang consumers should treat this cdylib as a compatibility boundary and should not rely on internal Rust module layout.

## Testing Contract
Default workspace tests exclude this crate because linking requires an Erlang runtime. Smoke verification belongs in a host-aware job or documented manual release step.

## Non-Goals
None. Reason: this crate is a host-language boundary. Revisit trigger: add pure Rust wrapper modules that can be tested without BEAM.
