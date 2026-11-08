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
**Please do not report security vulnerabilities through public GitHub issues.**

1. Send an email to the security contact: `jero.capelle@gmail.com`.
2. Include:
   - A detailed description of the vulnerability.
   - Steps to reproduce or proof-of-concept (PoC).
   - Your assessment of impact and affected components.
3. You will receive an initial response within 48 hours.
4. We will coordinate a patch and responsible disclosure timeline with you before releasing a public security advisory.
