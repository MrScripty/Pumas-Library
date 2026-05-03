# Model Ref Fixtures

## Purpose

Small JSON fixtures for `PumasModelRef` migration and graph-preservation
contracts. These fixtures prove unresolved legacy refs stay unresolved with
diagnostics instead of being silently replaced by a different model.

## Fixture Index

- `unresolved_legacy_path.json`: unresolved legacy path with an empty model id
  and a stable migration diagnostic.
