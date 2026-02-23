# Network

## Purpose

HTTP networking layer providing resilient request handling with retry logic, circuit breaking,
rate-limit awareness, and centralized connectivity management. Includes a GitHub API client
for release fetching and a download manager with progress tracking.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `client.rs` | `HttpClient` - HTTP client wrapper with rate-limit state tracking |
| `retry.rs` | `RetryConfig` / `retry_async` - Exponential backoff with jitter for transient failures |
| `circuit_breaker.rs` | `CircuitBreaker` - Closed/Open/HalfOpen state machine to prevent cascading failures |
| `manager.rs` | `NetworkManager` - Centralized connectivity checking, per-domain circuit breakers, web source registry |
| `download.rs` | `DownloadManager` - File downloads with progress callbacks and resume support |
| `github.rs` | `GitHubClient` - GitHub Releases API with local caching |
| `web_source.rs` | `WebSource` / `WebSourceId` / `CacheStrategy` traits for extensible source registration |

## Design Decisions

- **Per-domain circuit breakers**: Each external domain (github.com, huggingface.co) has its own
  circuit breaker so a failure on one service does not block requests to another.
- **Offline-first fallback**: When connectivity is lost, `NetworkManager` serves stale cached data
  rather than failing, controlled by each source's `CacheStrategy`.
- **Trait-based web sources**: The `WebSource` trait allows new external services to be registered
  with the `NetworkManager` without modifying core networking code.

## Dependencies

### Internal
- `crate::error` - `PumasError` / `Result`
- `crate::cache` - SQLite cache backend (used by `GitHubClient`)

### External
- `reqwest` - HTTP client
- `tokio` - Async runtime
- `async-trait` - Async trait definitions
