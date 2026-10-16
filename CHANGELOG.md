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
- Global hotkey (`Ctrl`+`Alt`+`T` by default) that opens the flyout in the middle of the active screen and closes it on the second press. Turn it off or pick another combination in **Settings → General**.
- OpenCode (Go plan: 5 h, weekly and monthly meters) and Command Code (5 h, weekly and monthly limits) providers. OpenCode needs the `OPENCODE_GO_WORKSPACE_ID` and `OPENCODE_GO_AUTH_COOKIE` environment variables; see the README.
- Connect providers from **Settings → Providers**: enter the key, cookie or workspace ID of OpenCode and Command Code in the app, or add your own provider (HTTPS URL, authentication, JSON paths of the limit values). Entered credentials are stored in the OS credential store (Windows Credential Manager), not in the settings file.
- Add provider form: quick-fill presets for OpenRouter and Synthetic, a Show/Hide button for the token, and a **Test** button that tries the URL, token and paths against the live response before anything is saved.
- OpenCode is recognised from the `opencode` CLI's Go login. Until the console cookie and workspace ID are entered, its card says "Not connected" and points to **Settings → Providers** instead of "Agent not found". An account without a Go plan gets "No active plan" rather than an endpoint error.
- Claude Code plan (Pro, Max) is shown even when the credentials file has no `subscriptionType`: Tokenza falls back to the plan cached in `~/.claude.json` (read-only).
- Cards show when the subscription ends or renews ("subscription until 1 Nov 2026") for Codex (from the login) and Command Code (billing period end). Claude Code, Cursor and OpenCode do not expose it locally, so they show no such line.
- Devin provider: daily and weekly quota, plan name and pay-as-you-go balance, read with the API key of the Devin CLI (`%APPDATA%\Devin\credentials.toml`). New "Day" window kind.
- Antigravity provider (experimental): model quota from the Antigravity CLI login, shown as the most-used model per window. Written from community documentation of a private endpoint and not yet verified against a signed-in account.
- Open-source project files: license, contributing guide, code of conduct, security policy, CI.
