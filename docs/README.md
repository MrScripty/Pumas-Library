# Documentation Index

This directory contains all project documentation for the ComfyUI Linux Launcher.

## Getting Started

### For Users
- **[../README.md](../README.md)** - Main project README with installation and usage instructions

### For Developers
- **[../CONTRIBUTING.md](../CONTRIBUTING.md)** - **START HERE** - Development standards, code quality requirements, and contribution guidelines
- **[../backend/README.md](../backend/README.md)** - Backend architecture and design decisions
- **[../frontend/README.md](../frontend/README.md)** - Frontend architecture and design decisions

## Code Standards

These documents define how code should be written and organized:

- **[CODING_STANDARDS.md](CODING_STANDARDS.md)** - General code style standards (React Aria usage)
- **[../frontend/CONTRIBUTING.md](../frontend/CONTRIBUTING.md)** - Frontend-specific coding standards
- **[REACT_ARIA_ENFORCEMENT.md](REACT_ARIA_ENFORCEMENT.md)** - React Aria usage enforcement and rationale

## Testing

- **[TESTING.md](TESTING.md)** - How testing works, how to run tests, and what tests exist

## Security

- **[SECURITY.md](SECURITY.md)** - Security practices, vulnerability scanning, and reporting procedures

## Architecture & Design

These documents explain how different parts of the system are designed and why:

### Core Architecture
- **[architecture/FRONTEND_ARCHITECTURE.md](architecture/FRONTEND_ARCHITECTURE.md)** - Frontend refactoring approach, component organization, and architectural decisions
- **[architecture/MULTI_APP_SYSTEM.md](architecture/MULTI_APP_SYSTEM.md)** - Multi-app launcher architecture and extensibility design
- **[architecture/MODEL_LIBRARY.md](architecture/MODEL_LIBRARY.md)** - Model management system design and mapping pipeline

### Theme System
- **[../frontend/THEME_SYSTEM.md](../frontend/THEME_SYSTEM.md)** - Dark theme implementation, color tokens, and usage patterns

## API Migration

These migration docs are kept for API consumers moving from release `0.1.0` to `0.2.0`:

- **[METADATA_V2_API_MIGRATION_0_1_0_TO_0_2_0.md](METADATA_V2_API_MIGRATION_0_1_0_TO_0_2_0.md)** - `0.1.0` to `0.2.0` endpoint mapping, migration patterns, and migration runner APIs
- **[plans/PUMAS_LIBRARY_METADATA_V2_CONSUMER_MIGRATION.md](plans/PUMAS_LIBRARY_METADATA_V2_CONSUMER_MIGRATION.md)** - Full consumer migration/cutover plan

## Legal & Compliance

- **[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)** - License information for dependencies

---

## Documentation Organization

```
docs/
├── README.md                          # This file - documentation index
├── TESTING.md                         # Testing guide
├── SECURITY.md                        # Security practices
├── CODING_STANDARDS.md                # Code style standards
├── REACT_ARIA_ENFORCEMENT.md          # React Aria enforcement
├── METADATA_V2_API_MIGRATION_0_1_0_TO_0_2_0.md # 0.1.0 -> 0.2.0 API consumer migration guide
├── THIRD-PARTY-NOTICES.md             # Legal notices
├── plans/                             # Detailed implementation and migration plans
└── architecture/                      # Architecture & design docs
    ├── FRONTEND_ARCHITECTURE.md       # Frontend design decisions
    ├── MULTI_APP_SYSTEM.md            # Multi-app architecture
    └── MODEL_LIBRARY.md               # Model management design
```

## Contributing Documentation

When adding new documentation:

1. **User-facing docs** → Root directory (README.md)
2. **Developer onboarding** → Root directory (CONTRIBUTING.md)
3. **Code standards** → `docs/` directory
4. **Architecture & design** → `docs/architecture/` directory
5. **Component-specific** → Component directory (e.g., `backend/README.md`, `frontend/README.md`)

All documentation should be in Markdown format and follow these guidelines:
- Use clear, descriptive headings
- Include code examples where appropriate
- Link to related documentation
- Keep documentation up-to-date with code changes
- Explain *why* decisions were made, not just *what* the code does

---

## Quick Links by Role

### I want to contribute code
1. Read [CONTRIBUTING.md](../CONTRIBUTING.md)
2. Review code standards: [CODING_STANDARDS.md](CODING_STANDARDS.md) and [frontend/CONTRIBUTING.md](../frontend/CONTRIBUTING.md)
3. Understand testing: [TESTING.md](TESTING.md)
4. Review architecture: [backend/README.md](../backend/README.md) and [frontend/README.md](../frontend/README.md)

### I want to understand the architecture
1. Backend: [backend/README.md](../backend/README.md)
2. Frontend: [frontend/README.md](../frontend/README.md)
3. Specific features: [architecture/](architecture/) directory

### I want to report a security issue
- See [SECURITY.md](SECURITY.md)

### I want to understand licensing
- See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)
- Main license: [../LICENSE](../LICENSE)

### I need API migration instructions
1. Read [METADATA_V2_API_MIGRATION_0_1_0_TO_0_2_0.md](METADATA_V2_API_MIGRATION_0_1_0_TO_0_2_0.md)
2. Use [plans/PUMAS_LIBRARY_METADATA_V2_CONSUMER_MIGRATION.md](plans/PUMAS_LIBRARY_METADATA_V2_CONSUMER_MIGRATION.md) for rollout details
