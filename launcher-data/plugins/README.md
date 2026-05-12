# Launcher Plugin Manifests

## Purpose
This directory contains machine-consumed plugin manifest JSON files used by launcher and application-management flows.

## Producer Contract
Each manifest must describe one plugin by stable identifier, display name, executable or service behavior, version policy, and any managed filesystem locations. Schema changes require a migration or compatibility note.

## Consumer Contract
Consumers must parse manifests as structured data and reject missing required fields instead of applying ad hoc defaults.

`installationType: "in-process"` means the runtime is supplied by Pumas itself
and must not be routed through binary, Python, Docker, or version-manager
installation flows. ONNX Runtime uses this shape because embedding sessions are
owned by the Rust process and exposed through backend runtime profiles plus the
Pumas `/v1` gateway.

## Validation Contract
Manifest validation should run in launcher or app-manager tests before manifests are packaged for release.

## Non-Goals
Runtime plugin cache data is out of scope. Reason: this directory should contain source-controlled manifests, not generated plugin state. Revisit trigger: add user-installed plugin support.
