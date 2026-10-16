# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository. See [README.md](README.md) for the product overview and [CONTRIBUTING.md](CONTRIBUTING.md) for the contribution process.

## What this project is

Tokenza is a Windows 10/11 tray app (Tauri 2 + React/TypeScript) that shows the usage limits of AI coding agents (Claude Code, Codex, Cursor) and when they reset. It reads each agent's locally stored login, calls that provider's usage endpoint, and displays the result.

## Repository layout

```
crates/tokenza-core   Provider trait, data model, one module per provider (Rust, no UI knowledge)
crates/tokenza-cli    `tokenza [claude|codex|cursor]` prints limits as JSON
apps/tokenza          Desktop app
  src-tauri           Tauri backend: tray, scheduler, adaptive polling, notifications, settings
  src                 React + TypeScript UI (Fluent UI, Zustand); locales in src/locales
docs/architecture.md  How the pieces fit together
```

## Commands

Run from the repository root unless noted.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cd apps/tokenza
npm ci                 # or npm install
npm run build          # tsc --noEmit && vite build  (also produces dist/)
npm test               # vitest
npx tauri dev          # full app with hot reload (Vite on port 1421)
npm run dev            # UI only, in a browser, with demo data
```

All checks above must pass before a change is considered done; CI runs the same ones.

## Gotchas

- **Build the frontend before compiling the Tauri crate.** `tokenza-app` embeds `apps/tokenza/dist` at compile time, so `cargo check/clippy/test --workspace` fails on a fresh checkout until `npm run build` has run in `apps/tokenza`.
- **The app is Windows-first.** Tray, Acrylic backdrop, autostart and the Cursor credential path (`%APPDATA%`) are Windows-oriented. Don't introduce code that breaks `cargo check` on other platforms without `#[cfg]` guards where it is easy to avoid.
- **Translations have two homes.** UI strings: `apps/tokenza/src/locales/*.ts` (typed against `en.ts`). Tray menu, toasts and window titles: `apps/tokenza/src-tauri/src/i18n.rs`. A string used in both needs updating in both. `src/i18n.test.ts` enforces key and placeholder parity across languages.
- **Adding a provider touches several files** (core module, `all_providers()`, `KNOWN_PROVIDERS` and name match in `settings.rs`, `PROVIDER_NAMES` in `format.ts`, `DEFAULT_ORDER` in `ProvidersPane.tsx`, `logoPaths.ts`). The full checklist is in CONTRIBUTING.md.
- `src/demo.ts` supplies sample data when the UI runs outside Tauri; update it if you change the state DTO in `types.ts` / `state.rs`.
- Generated or local-only paths are git-ignored: `target/`, `node_modules/`, `apps/tokenza/dist/`, `apps/tokenza/src-tauri/gen/`, `.claude/`. Never commit them.

## Hard rules (privacy and security)

Tokenza handles other tools' credentials. These are non-negotiable:

1. Tokens are read into memory per request and dropped afterwards. Never persist, log, print, or send them anywhere except the provider's own endpoint. The one exception is a key the user types into Tokenza themselves: it may be stored, but only in the OS credential store (`secrets.rs`), never in `settings.json`, logs or the UI state.
2. Never modify or refresh the user's credential files; open third-party databases read-only.
3. No telemetry and no new network destinations. A new domain needs discussion and an update to the README privacy table.
4. Never commit real tokens, even expired ones. Use obviously fake values in tests.
5. Keep the strict Content-Security-Policy in `tauri.conf.json`; all provider traffic stays in Rust, not in the web view.

## Conventions

- **English only** for code, comments, identifiers, commit messages and docs. User-facing text belongs in the locale files; other languages there are translations, not a license to write Russian (or any other language) elsewhere.
- Rust: `rustfmt` default settings, no clippy warnings. Return typed errors (`ProviderError`) instead of panicking; a changed provider response should become `SchemaChanged`, never wrong numbers.
- TypeScript: strict mode; shared formatting helpers live in `format.ts`.
- Comment the *why*, not the *what*. Match the style of surrounding code.
- Add or update tests with behavior changes (provider parsers are tested with fake JSON bodies; see `providers/claude.rs`).
- User-visible changes get an entry under "Unreleased" in [CHANGELOG.md](CHANGELOG.md).
- Keep changes focused; don't reformat or refactor unrelated code in the same change.
