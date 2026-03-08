# Documentation Index

This directory contains project documentation for the current Rust/Electron implementation.

## Core Docs

- [../README.md](../README.md) - Project overview, install/use flows, and release validation commands
- [../CONTRIBUTING.md](../CONTRIBUTING.md) - Contribution workflow and coding expectations
- [../frontend/README.md](../frontend/README.md) - Frontend-focused implementation notes

## Standards and Practices

- [CODING_STANDARDS.md](CODING_STANDARDS.md) - General coding conventions
- [REACT_ARIA_ENFORCEMENT.md](REACT_ARIA_ENFORCEMENT.md) - Frontend interaction/accessibility guardrails
- [TESTING.md](TESTING.md) - Test/build validation workflows
- [SECURITY.md](SECURITY.md) - Security process and scanning guidance
- [MODEL_RUNTIME_RESEARCH_AGENT.md](MODEL_RUNTIME_RESEARCH_AGENT.md) - Agent workflow for model runtime dependency and inference-settings research/persistence

## Architecture

- [architecture/README.md](architecture/README.md) - Architecture docs index
- [architecture/SYSTEM_ARCHITECTURE.md](architecture/SYSTEM_ARCHITECTURE.md) - Runtime/process architecture
- [architecture/MODEL_LIBRARY_ARCHITECTURE.md](architecture/MODEL_LIBRARY_ARCHITECTURE.md) - Model library and dependency contract architecture

## Plans

- [plans/README.md](plans/README.md) - Implementation plans for cross-module changes
- [plans/external-reference-diffusers-implementation-plan.md](plans/external-reference-diffusers-implementation-plan.md) - Plan for external-reference diffusers bundle support integrated with the current model-library systems
- [plans/directory-import-disambiguation-implementation-plan.md](plans/directory-import-disambiguation-implementation-plan.md) - Plan for distinguishing bundle-root directories from multi-model containers during GUI import

## Legal and Compliance

- [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) - Third-party license notices
- [../LICENSE](../LICENSE) - Project license

## Directory Layout

```text
docs/
├── README.md
├── CODING_STANDARDS.md
├── REACT_ARIA_ENFORCEMENT.md
├── TESTING.md
├── SECURITY.md
├── THIRD-PARTY-NOTICES.md
├── plans/
│   ├── README.md
│   ├── directory-import-disambiguation-implementation-plan.md
│   └── external-reference-diffusers-implementation-plan.md
├── sbom/
└── architecture/
    ├── README.md
    ├── SYSTEM_ARCHITECTURE.md
    └── MODEL_LIBRARY_ARCHITECTURE.md
```
