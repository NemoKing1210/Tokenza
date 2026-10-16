# Tokenza

[![CI](https://github.com/OWNER/tokenza/actions/workflows/ci.yml/badge.svg)](https://github.com/OWNER/tokenza/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A lightweight Windows tray app that shows how much of your AI coding agents' usage limits you have left, and when they reset.

Tokenza watches **Claude Code**, **Codex**, **Cursor**, **OpenCode** (Go plan), **Command Code**, **Devin** and **Antigravity**. Click the tray icon to see every limit window at a glance, and get a notification before you run out.

> **Status:** early development (v0.1). Windows 10/11 only. Expect rough edges.

## Features

- **Tray icon** showing your worst limit as a ring, two bars (5 h and weekly), a pie or a number. A settings page lets you choose whose limits it follows (all providers, a few, or one) and which periods count. Any provider can also get an icon of its own, with its logo.
- **Flyout** with one card per provider: usage, time until reset, last update. Standard and compact views.
- **Global hotkey** (`Ctrl`+`Alt`+`T` by default) opens the flyout in the middle of the screen the mouse is on, without touching the tray; press it again to close. Can be turned off or changed in Settings.
- **Notifications** when usage crosses a warning or critical threshold (once per reset cycle) and when a limit becomes available again.
- **Smart polling** that tunes the refresh rate per provider: faster near a limit, slower when idle, and backs off when a provider rate-limits you. A fixed interval is also available.
- **Per-provider settings**: display name, which limit windows to show, tray and notification participation, ordering.
- **Light / dark / system theme** and Windows Acrylic backdrop.
- **Localized** into English, Russian, Spanish, German, French, Portuguese (Brazil) and Simplified Chinese.
- **Start with Windows** (optional).

## Privacy and network access

Tokenza is built to be easy to audit:

- It reads the login that each agent already stores on your machine (for example `~/.claude/.credentials.json`), **into memory, only for the duration of a request**. Tokens are never written to disk, logged or shown.
- It never refreshes or modifies your tokens. If a token has expired, Tokenza asks you to sign in to the agent again.
- There is **no telemetry** and no Tokenza server.
- The only network requests go to the provider you are tracking:

| Provider    | Domain             | Credentials read from                                  |
|-------------|--------------------|--------------------------------------------------------|
| Claude Code | `api.anthropic.com`| `~/.claude/.credentials.json`                          |
| Codex       | `chatgpt.com`      | `~/.codex/auth.json`                                   |
| Cursor      | `cursor.com`       | Cursor's local `state.vscdb` (opened read-only)        |
| OpenCode Go | `opencode.ai`      | `OPENCODE_GO_WORKSPACE_ID` and `OPENCODE_GO_AUTH_COOKIE` environment variables (see below) |
| Command Code| `api.commandcode.ai`| `~/.commandcode/auth.json` (or `COMMAND_CODE_API_KEY`) |
| Devin       | `server.codeium.com`| `%APPDATA%\Devin\credentials.toml` (the CLI's API key) |
| Antigravity | `cloudcode-pa.googleapis.com`| `~/.gemini/antigravity-cli/antigravity-oauth-token` (read-only; unverified, see below) |

**Devin and Antigravity.** Devin's daily and weekly quota is read with the API key the Devin CLI keeps in `credentials.toml`; the key is only ever sent to `server.codeium.com`, whatever `api_server_url` says in that file. Antigravity's limits come from Google's private `retrieveUserQuota` method with the access token of the Antigravity CLI login. That method is undocumented, and this provider was written from community notes and has not been run against a signed-in account yet: if its card says the response changed, or shows nothing, please open an issue.

**OpenCode Go setup.** The OpenCode console has no API-key access to usage, and the `opencode` CLI does not store a console login, so Tokenza never touches your browser's cookies. (It only checks that `~/.local/share/opencode/auth.json` has an `opencode-go` entry, to tell "installed but not connected" from "not installed"; the key is not used.) Instead, open the OpenCode card in **Settings → Providers** and enter the workspace ID (the `wrk_…` part of `opencode.ai/workspace/wrk_…/go`) and the value of the `__Host-console_session` cookie, copied from your browser's developer tools. The `OPENCODE_GO_WORKSPACE_ID` and `OPENCODE_GO_AUTH_COOKIE` environment variables work too. The cookie expires eventually; replace it when the card asks you to sign in again.

### Keys you enter yourself

Some providers need a key that no agent stores locally, and you can also add services Tokenza does not know (**Settings → Providers → Add provider**: a name, an HTTPS URL, how to authenticate, and the JSON paths of the numbers to show).

- What you type is kept in the **Windows Credential Manager** under the name "Tokenza", never in `settings.json`, and is never shown again (you can replace or remove it).
- Tokenza reads it into memory only while building a request, like the logins it reads from the agents.
- A custom provider only ever talks to the single HTTPS URL you entered, sends the token only there, and does not follow redirects. Custom providers add no domain to the list above; the one you configure is the only one contacted. The form's quick-fill buttons (OpenRouter, Synthetic) only pre-fill that URL and the JSON paths; nothing is contacted until you save the provider or press **Test**, which sends the one request to the URL in the form and stores nothing.

Settings (thresholds, order, language, …) are stored in your user config directory as `settings.json` and contain no credentials.

The last fetched limits (percentages, reset times, account label) are cached next to it as `snapshots.json`, so they stay visible when a check fails; this file contains no credentials either.

### Unofficial endpoints

These providers do not publish a documented API for subscription usage. Tokenza uses the same endpoints that the agents' own usage views use. They can change or disappear without notice, and providers could restrict third-party use of them. If a response format changes, Tokenza shows an "Endpoint changed" card instead of wrong numbers, and the provider module needs an update. Use Tokenza at your own discretion.

## Install

Pre-built installers will be published on the [Releases](../../releases) page once available. Until then, build from source (see below).

## Build from source

Requirements:

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20 or newer
- Windows 10/11 with the [WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on Windows 11)
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

```bash
git clone https://github.com/OWNER/tokenza.git
cd tokenza/apps/tokenza
npm install
npx tauri dev                          # run with hot reload (Vite on port 1421)
npx tauri build                        # release build with an NSIS installer
npx tauri build --debug --no-bundle    # quick debug build: target/debug/tokenza-app.exe
```

Launching the executable a second time opens the flyout of the already running instance.

### Command-line prototype

The core library also ships a small CLI that prints the limits of every detected agent as JSON:

```bash
cargo run -p tokenza-cli              # all providers
cargo run -p tokenza-cli -- claude    # one provider: claude | codex | cursor | opencode | commandcode
```

## Project layout

```
crates/tokenza-core   Provider implementations and the shared data model (Rust)
crates/tokenza-cli    Command-line front end for the core library
apps/tokenza          Desktop app
  src-tauri           Tauri 2 backend: tray, scheduler, notifications, settings (Rust)
  src                 React + TypeScript UI (Fluent UI, Zustand)
docs/                 Architecture notes
```

See [docs/architecture.md](docs/architecture.md) for how the pieces fit together.

## Development

```bash
# Rust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Frontend (from apps/tokenza)
npm run build      # type-check + production build
npm test
```

Running `npm run dev` in `apps/tokenza` serves the UI in a normal browser with demo data, which is handy for UI work without the Tauri shell.

## Contributing

Contributions are welcome: bug reports, new providers, translations and fixes. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first and follow the [Code of Conduct](CODE_OF_CONDUCT.md). To report a security issue, see [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE)

## Trademarks and acknowledgements

Tokenza is an independent project and is **not affiliated with, endorsed by, or sponsored by** Anthropic, OpenAI, Anysphere (Cursor), Anomaly (OpenCode), Command Code, Cognition (Devin, Windsurf), Google (Antigravity) or Microsoft. Claude, Claude Code, Codex, ChatGPT, OpenAI, Cursor, OpenCode, Command Code, Devin, Antigravity and Windows are trademarks of their respective owners; they are used here only to identify the services Tokenza reads data from.

Provider logos come from [Simple Icons](https://simpleicons.org/) (CC0 icon data), except the Command Code, Devin and Antigravity marks, which are the icons published on their websites; the marks themselves remain the property of their owners.

Built with [Tauri](https://tauri.app/), [React](https://react.dev/), [Fluent UI](https://react.fluentui.dev/) and [tiny-skia](https://github.com/RazrFalcon/tiny-skia).
