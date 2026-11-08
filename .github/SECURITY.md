# Security Policy

## 🔒 Supported Versions

We provide security updates and patches for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

---

## 🚨 Reporting a Vulnerability

Because `agy-suite` enforces Bubblewrap container boundaries, path isolation, and agent capability perimeters, we treat security vulnerabilities with top priority.

### In-Scope Vulnerabilities
- Bubblewrap sandbox escapes or filesystem unshare circumventions.
- Path traversal vulnerabilities allowing unauthorized writes outside the workspace root.
- Unsanitized command injections or argument splitting bugs in `parameterized_command`.
- Accidental leakage of host environment credentials (`.env`, SSH keys) through tool returns.

### Reporting Procedure
Please report security vulnerabilities by opening an issue on GitHub:

1. [Open a new issue](https://github.com/JeronimoCapelle/agy-suite/issues/new) in the repository.
2. Title the issue with `[Security]` prefix and include:
   - A detailed description of the vulnerability.
   - Steps to reproduce or proof-of-concept (PoC).
   - Your assessment of impact, affected components, and configuration (e.g. sandbox settings).
3. We will review, triage, and address the report promptly.
