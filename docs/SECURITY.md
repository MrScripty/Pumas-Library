# Security Policy

## Vulnerability Scanning

This project uses automated security scanning to identify and address vulnerabilities in dependencies.

### Tools

- **pip-audit**: Scans Python dependencies for known CVEs
- **npm audit**: Scans Node.js dependencies for known vulnerabilities

### Running Security Scans

#### Python Dependencies

```bash
# Activate virtual environment
source venv/bin/activate

# Scan project dependencies
pip-audit -r requirements.txt

# Scan locked dependencies
pip-audit -r requirements-lock.txt

# Get detailed output with descriptions
pip-audit --desc

# Output to JSON for analysis
pip-audit --format json > security-audit-python.json
```

#### Node.js Dependencies

```bash
# From project root
cd frontend

# Scan all dependencies
npm audit

# Scan production dependencies only
npm audit --omit=dev

# Get JSON output
npm audit --json > security-audit-frontend.json
```

### Security Scan Results

**Last Scan Date:** 2025-12-29

**Python Dependencies:**
- Project requirements.txt: ✅ 0 vulnerabilities
- requirements-lock.txt: ✅ 0 vulnerabilities
- Status: CLEAN

**Node.js Dependencies:**
- Production dependencies: ✅ 0 vulnerabilities
- Status: CLEAN

### Fixing Vulnerabilities

#### Python

```bash
# Automatic fix (updates to latest safe versions)
pip-audit --fix

# Manual fix
# 1. Review the vulnerability report
# 2. Update the affected package in requirements.txt
# 3. Regenerate lock file: pip-compile requirements.txt -o requirements-lock.txt
# 4. Re-run tests to ensure compatibility
```

#### Node.js

```bash
# Automatic fix for non-breaking changes
npm audit fix

# Fix including breaking changes (review carefully)
npm audit fix --force

# Manual fix
# 1. Review npm audit output
# 2. Update package.json with safe versions
# 3. Run npm install
# 4. Re-run tests to ensure compatibility
```

### Maintenance Schedule

- **Weekly**: Automated scans run as part of development workflow
- **Before Release**: Full security audit required
- **After Dependency Updates**: Always run scans after updating dependencies
- **Security Advisories**: Monitor GitHub Security Advisories for real-time alerts

### Reporting Security Issues

If you discover a security vulnerability in this project, please report it by:

1. **DO NOT** open a public issue
2. Email the maintainer directly or use GitHub's private vulnerability reporting
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if available)

### Security Best Practices

This project follows these security practices:

1. **Dependency Management**
   - All dependencies pinned in lock files (requirements-lock.txt, package-lock.json)
   - Regular updates with security scanning
   - Minimal dependency footprint

2. **Input Validation**
   - Version tags validated with regex patterns
   - File paths sanitized against traversal attacks
   - URLs validated for safe schemes only

3. **File Operations**
   - Atomic writes prevent corruption
   - File locking prevents race conditions
   - Temporary files cleaned up properly

4. **External Resources**
   - GitHub API: read-only access, no credentials required
   - PyPI: verified package sources
   - No external CDN dependencies

5. **Process Security**
   - Sandboxed subprocess execution
   - Process title masking for privacy
   - Proper cleanup of child processes

### Excluded from Scanning

The following packages are system-level dependencies and not part of the project's dependency tree. They are excluded from security scans:

- System Python packages (python-apt, ufw, etc.)
- Ubuntu-specific packages (ubuntu-drivers-common, etc.)
- Desktop environment packages (catfish, menulibre, etc.)

These are managed by the system package manager and receive security updates through OS updates.

### Continuous Monitoring

Security scanning is integrated into the development workflow:

- Pre-commit hooks enforce code quality
- Dependencies tracked in version control
- Regular security audits documented
- Vulnerability reports archived for reference

## Supported Versions

Currently, only the latest version of the project receives security updates.

| Version | Supported          |
| ------- | ------------------ |
| Latest  | :white_check_mark: |
| Older   | :x:                |

## Security Contact

For security concerns, please contact the project maintainers through GitHub.
