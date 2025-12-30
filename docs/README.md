# Documentation Index

This directory contains all project documentation for the ComfyUI Linux Launcher.

## Active Documentation

### For Users
- **[../README.md](../README.md)** - Main project README with installation and usage instructions

### For Developers
- **[../CONTRIBUTING.md](../CONTRIBUTING.md)** - **START HERE** - Development standards, code quality requirements, and contribution guidelines
- **[TESTING.md](TESTING.md)** - Comprehensive testing guide, fixtures, and best practices
- **[SECURITY.md](SECURITY.md)** - Security practices, vulnerability scanning, and reporting

### Legal & Compliance
- **[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)** - License information for dependencies
- **[sbom/](sbom/)** - Software Bill of Materials (SBOM) for dependency tracking

## Archived Documentation

The `archive/` subdirectory contains historical work-in-progress documents that are no longer actively maintained but preserved for reference:

- **[archive/PRODUCTION_READINESS_PLAN.md](archive/PRODUCTION_READINESS_PLAN.md)** - Original production readiness checklist (completed)
- **[archive/MYPY_PROGRESS.md](archive/MYPY_PROGRESS.md)** - Type checking remediation tracking (completed)
- **[archive/COMFYUI_VERSION_MANAGER_PLAN.md](archive/COMFYUI_VERSION_MANAGER_PLAN.md)** - Original version manager design spec
- **[archive/WEIGHTED_PROGRESS_IMPLEMENTATION.md](archive/WEIGHTED_PROGRESS_IMPLEMENTATION.md)** - Installation progress tracking design

These archived documents represent completed work and are kept for historical context.

---

## Documentation Standards

When adding new documentation:

1. **User-facing docs** → Root directory (README.md)
2. **Developer docs** → `docs/` directory
3. **Completed/obsolete docs** → `docs/archive/` directory
4. **Generated artifacts** → `docs/sbom/` or appropriate subdirectory

All documentation should be in Markdown format and follow the repository's formatting standards.
