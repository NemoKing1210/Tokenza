# Tokenza

[![CI](https://github.com/OWNER/tokenza/actions/workflows/ci.yml/badge.svg)](https://github.com/OWNER/tokenza/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A lightweight Windows tray app that shows how much of your AI coding agents' usage limits you have left, and when they reset.

Tokenza watches **Claude Code**, **Codex** and **Cursor**. Click the tray icon to see every limit window at a glance, and get a notification before you run out.

> **Status:** early development (v0.1). Windows 10/11 only. Expect rough edges.

## Features

- **Tray icon** showing your worst limit as a ring, or two bars (5 h and weekly).
- **Flyout** with one card per provider: usage, time until reset, last update. Standard and compact views.
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

Settings (thresholds, order, language, …) are stored in your user config directory as `settings.json` and contain no credentials.

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
cargo run -p tokenza-cli -- claude    # one provider: claude | codex | cursor
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

Tokenza is an independent project and is **not affiliated with, endorsed by, or sponsored by** Anthropic, OpenAI, Anysphere (Cursor) or Microsoft. Claude, Claude Code, Codex, ChatGPT, OpenAI, Cursor and Windows are trademarks of their respective owners; they are used here only to identify the services Tokenza reads data from.

Provider logos come from [Simple Icons](https://simpleicons.org/) (CC0 icon data); the marks themselves remain the property of their owners.

Built with [Tauri](https://tauri.app/), [React](https://react.dev/), [Fluent UI](https://react.fluentui.dev/) and [tiny-skia](https://github.com/RazrFalcon/tiny-skia).
