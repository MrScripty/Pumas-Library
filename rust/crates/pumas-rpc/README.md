# pumas-rpc

## Purpose
`pumas-rpc` builds the JSON-RPC server binary used by the Electron shell and local automation to call Rust-backed Pumas Library operations.

## Ownership
This crate owns HTTP transport setup, JSON-RPC request ingress, request-to-handler dispatch, and process lifecycle for the backend server. Domain behavior remains in `pumas-library` or `pumas-app-manager`.

## Producer Contract
Handlers must parse renderer or network supplied payloads at the RPC boundary before forwarding typed commands to domain services. Method names must stay aligned with `docs/contracts/desktop-rpc-methods.md`.

The first typed-command pass covers model import/download and process open handlers. New handlers should prefer `handlers::parse_params` plus serde aliases for camelCase compatibility instead of ad hoc `serde_json::Value` extraction.

The HTTP server accepts CORS requests only from loopback browser origins and only for `GET`/`POST` with `Content-Type`. External LAN or internet browser origins are not part of the supported trust boundary.

## Consumer Contract
Electron should treat this crate as the only Rust process RPC endpoint. Tests may launch the binary or call server helpers, but should avoid reaching into domain modules through this crate.

## Testing Contract
Integration tests under `tests/` cover transport behavior and representative request/response contracts. Unit tests for durable model-library state belong in `pumas-library`.

## Non-Goals
None. Reason: transport, dispatch, and lifecycle are all active responsibilities for this crate. Revisit trigger: split a reusable server library out of the binary crate.
