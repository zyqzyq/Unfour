# Security Policy

Unfour handles sensitive material — SSH private keys, database
passwords, and API tokens — so we take security reports seriously.

## Supported Versions

Unfour `0.1.x` is the first public release line. Security fixes are applied to the latest `0.x`
release only. There is no long-term support branch yet.

| Version | Supported |
| ------- | --------- |
| latest `0.x` | ✅ |
| older | ❌ |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Instead, report privately through one of:

- GitHub's [private vulnerability reporting](https://github.com/zyqzyq/Unfour/security/advisories/new)
  (preferred), or
- email to the maintainer at **zyqreid@gmail.com** with the subject
  `[SECURITY] Unfour`.

Please include:

- A description of the issue and its impact.
- Steps to reproduce or a proof of concept.
- Affected version / commit, and your environment.

We will acknowledge your report within a reasonable time, keep you updated on
progress, and credit you in the release notes unless you prefer to remain
anonymous. Please give us a reasonable window to ship a fix before any public
disclosure.

## Security Design Notes

- Credentials are stored as references in the OS keychain, never as SQLite
  plaintext.
- Sensitive headers (`authorization`, `cookie`, `proxy-authorization`,
  `x-api-key`, `x-auth-token`) are redacted in logs, history, and activity
  details.
- Database SQL execution can run destructive statements; high-risk operations
  are gated behind explicit confirmation. Do not point pre-release or development builds at
  production systems.
