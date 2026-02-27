# pumas-app-manager ollama_client

## Purpose
Contains focused helpers for Ollama client behavior that are separated from the main HTTP client implementation to keep file size and responsibilities manageable.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `naming.rs` | Ollama model-name derivation logic and related tests. |

## Design Decisions
- Keep the core HTTP client flow in `ollama_client.rs` and isolate reusable pure helpers here.
- Unit tests for helper behavior live next to the helper implementation.

## Dependencies
**Internal:** used by `ollama_client.rs` via module import.
**External:** none.

## Usage Examples
```rust
let name = derive_ollama_name("Llama 2 7B");
assert_eq!(name, "llama-2-7b");
```
