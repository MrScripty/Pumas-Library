# Plugins

## Purpose

JSON-based plugin configuration system that allows defining new applications without code
changes. Each plugin specifies app metadata (name, icon, GitHub repo), capabilities
(version management, shortcuts), connection settings, API endpoints, and UI panel layout.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `loader.rs` | `PluginLoader` - Discovers and loads plugin JSON files from the plugins directory |
| `schema.rs` | `PluginConfig`, `AppCapabilities`, `ConnectionConfig`, `ApiEndpoint`, and related types |

## Design Decisions

- **JSON over Rust code for app definitions**: New applications (beyond built-in ComfyUI/Ollama)
  can be added by dropping a JSON file into the plugins directory, with no recompilation needed.
- **Capability flags**: `AppCapabilities` uses boolean flags (e.g., `has_version_management`,
  `has_shortcuts`) so the frontend can conditionally render UI sections based on what each
  app supports.

## Dependencies

### Internal
- `crate::error` - `PumasError` / `Result`

### External
- `serde` / `serde_json` - Plugin JSON deserialization
