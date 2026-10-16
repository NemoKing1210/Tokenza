# Security Policy

Tokenza reads credentials that other tools store on your machine, so we take security reports seriously.

## Supported versions

Only the latest release receives security fixes while the project is pre-1.0.

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Report privately through GitHub: open the **Security** tab of this repository and choose **Report a vulnerability** (GitHub private vulnerability reporting). Include:

- a description of the issue and its impact,
- steps to reproduce or a proof of concept,
- the affected version and Windows version.

You can expect an acknowledgement within a few days. We will keep you informed while we investigate, work on a fix, and credit you in the release notes if you wish.

## Scope

In scope, for example:

- Anything that causes tokens or credential files to be written to disk, logged, displayed, or sent anywhere other than the provider's own endpoint.
- Anything that modifies or corrupts the credential files or databases of the tracked tools.
- Content-Security-Policy bypasses or injection in the app's web view.
- Unsafe handling of data returned by provider endpoints.

Out of scope:

- Vulnerabilities in the tracked tools themselves (Claude Code, Codex, Cursor) or their services. Please report those to their vendors.
- Issues that require an attacker to already have full access to your user account, since the credential files Tokenza reads are readable by that account anyway.
- Changes to provider endpoints that make Tokenza stop working (open a regular issue instead).
