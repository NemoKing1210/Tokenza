# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Tray app for Windows 10/11 that shows usage limits and reset times for Claude Code, Codex and Cursor.
- Flyout with standard and compact views, tray icon as ring or bars.
- Threshold and "limit available again" notifications.
- Smart per-provider polling with backoff on rate limiting, or a fixed interval.
- Per-provider settings: display name, visible limit windows, ordering, tray and notification participation.
- Light, dark and system themes; Acrylic backdrop.
- Interface in English, Russian, Spanish, German, French, Portuguese (Brazil) and Simplified Chinese.
- `tokenza` command-line tool that prints provider limits as JSON.
- OpenCode (Go plan: 5 h, weekly and monthly meters) and Command Code (5 h, weekly and monthly limits) providers. OpenCode needs the `OPENCODE_GO_WORKSPACE_ID` and `OPENCODE_GO_AUTH_COOKIE` environment variables; see the README.
- Connect providers from **Settings → Providers**: enter the key, cookie or workspace ID of OpenCode and Command Code in the app, or add your own provider (HTTPS URL, authentication, JSON paths of the limit values). Entered credentials are stored in the OS credential store (Windows Credential Manager), not in the settings file.
- Open-source project files: license, contributing guide, code of conduct, security policy, CI.
