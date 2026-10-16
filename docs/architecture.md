# Architecture

Tokenza is a Cargo workspace plus a small web UI.

```
┌──────────────────────────────── apps/tokenza ────────────────────────────────┐
│  src/ (React + TypeScript)        ◄── IPC / events ──►   src-tauri/ (Rust)   │
│  flyout + settings windows                               tray, scheduler,    │
│                                                          notifications,      │
│                                                          settings, state     │
└────────────────────────────────────────────┬─────────────────────────────────┘
                                             │ uses
                                  ┌──────────▼──────────┐
                                  │  crates/tokenza-core │  providers + data model
                                  └──────────▲──────────┘
                                             │ uses
                                  ┌──────────┴──────────┐
                                  │  crates/tokenza-cli  │  JSON printer
                                  └─────────────────────┘
```

## `tokenza-core`

Everything that knows about a specific AI agent lives here, and nothing here knows about the UI.

- `Provider` trait (`provider.rs`): `detect()` finds a local login without touching the network; `fetch()` calls the provider's usage endpoint and returns a `UsageSnapshot`.
- Data model (`model.rs`): a snapshot is a list of `UsageWindow`s (5-hour session, weekly, monthly, credits), each with an optional used percentage and reset time. Snapshots never contain tokens.
- `ProviderError` (`error.rs`): typed failures (`NotLoggedIn`, `TokenExpired`, `RateLimited`, `SchemaChanged`, `Offline`). The UI chooses the card text and the suggested action from the kind.
- `providers/`: one module per agent (`claude`, `codex`, `cursor`). Each reads its credentials per request, sends one HTTPS request to a single fixed domain, and parses the response into windows.

## `tokenza-app` (`apps/tokenza/src-tauri`)

| Module        | Responsibility                                                                    |
|---------------|-----------------------------------------------------------------------------------|
| `main.rs`     | Tauri setup, IPC commands, single-instance handling, autostart, window events    |
| `scheduler.rs`| Background loop; polls each provider on its own schedule and publishes results   |
| `adaptive.rs` | Smart polling: backs off after rate-limit answers, speeds up near a limit, slows when idle |
| `state.rs`    | Shared application state and the DTO sent to the UI                               |
| `settings.rs` | User settings, persisted as `settings.json` in the app config directory           |
| `notify.rs`   | Threshold and "limit available again" toasts, deduplicated per reset cycle        |
| `tray.rs`     | Tray icon, menu and the flyout window                                             |
| `tray_icon.rs`| Renders the ring/bars tray indicator with `tiny-skia`                             |
| `i18n.rs`     | Strings for the tray menu, toasts and window titles; system-language detection    |

Data flow: the scheduler asks a provider for a snapshot, stores it in `AppState`, evaluates notifications, redraws the tray icon and emits an event. The UI fetches the full state on startup (`get_state`) and then reacts to events; user changes go back through `set_settings`.

## UI (`apps/tokenza/src`)

- `main.tsx` mounts either the flyout or the settings view depending on the `?view=` query parameter of the Tauri window URL.
- `store.ts` (Zustand) mirrors the backend state. Outside Tauri (plain `npm run dev` in a browser) it falls back to the sample data in `demo.ts`.
- `Flyout.tsx`, `ProvidersPane.tsx`, `UpdatePane.tsx`, `Settings.tsx` are the main screens; `format.ts` holds the shared formatting helpers.
- `i18n.ts` and `locales/*.ts` hold the UI dictionaries; `locales/en.ts` defines the key set that all other languages must match.

## Security model

- The web view runs with a strict Content-Security-Policy (see `tauri.conf.json`) and has no direct network access beyond IPC; all provider traffic happens in Rust.
- Credentials exist only as local variables inside `fetch()` and are dropped when the request completes.
- Third-party credential stores are opened read-only and never modified.
