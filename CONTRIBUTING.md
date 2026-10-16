# Contributing to Tokenza

Thanks for your interest in Tokenza! Bug reports, fixes, new providers and translations are all welcome. This document explains how to get set up and what we expect from a contribution.

By participating you agree to follow our [Code of Conduct](CODE_OF_CONDUCT.md). For security problems, do **not** open a public issue; see [SECURITY.md](SECURITY.md).

## Ways to contribute

- **Report a bug** or **request a feature** using the [issue templates](.github/ISSUE_TEMPLATE). Search existing issues first.
- **Add or fix a translation** (see [Translations](#translations)).
- **Add a provider** for another AI agent (see [Adding a provider](#adding-a-provider)).
- **Improve the docs.**

For anything larger than a small fix, please open an issue first so we can agree on the approach before you spend time on it.

## Development setup

Requirements: Rust (stable), Node.js 20.19 or newer, and on Windows the MSVC C++ build tools plus the WebView2 runtime. See the [README](README.md#build-from-source) for details.

```bash
git clone https://github.com/OWNER/tokenza.git
cd tokenza/apps/tokenza
npm install
npx tauri dev
```

To work on the UI only, run `npm run dev` in `apps/tokenza` and open http://localhost:1421 in a browser; it shows demo data when it is not running inside Tauri.

## Before you open a pull request

Run the same checks as CI:

```bash
# from the repository root
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# from apps/tokenza
npm run build     # tsc --noEmit && vite build
npm test
```

Then:

1. Create a topic branch from `main` (`fix/tray-flicker`, `feat/provider-foo`).
2. Keep the pull request focused: one logical change, with tests where it makes sense.
3. Update the docs and [CHANGELOG.md](CHANGELOG.md) (under "Unreleased") for user-visible changes.
4. Fill in the pull request template and link the issue it closes.

### Commit messages

Use short, imperative subject lines, for example `Fix tray ring colour at 100%`. [Conventional Commits](https://www.conventionalcommits.org/) prefixes (`fix:`, `feat:`, `docs:`) are welcome but not required.

### Code style

- **Language:** all code, comments, identifiers, commit messages and documentation are written in English. User-facing strings live in the locale files.
- **Rust:** formatted with `rustfmt`, lint-clean under `clippy` with warnings denied. Prefer returning typed errors (`ProviderError`) over panicking.
- **TypeScript/React:** strict mode is on. Keep components small; shared formatting helpers go in `format.ts`.
- Comment the *why*, not the *what*.

## Privacy rules for code changes

Tokenza handles other tools' credentials, so these rules are strict:

- Tokens are read into memory per request and dropped afterwards. **Never persist, log, print or transmit them anywhere except to the provider's own endpoint.** The only exception is a key the user types into Tokenza: it lives in the OS credential store, never in `settings.json`.
- Never modify or refresh a user's credential files. Open third-party databases read-only.
- No telemetry, analytics or additional network destinations. Adding a domain requires an issue discussion and an update to the table in the README.
- Don't commit real tokens, even expired ones, in tests or fixtures. Use obviously fake values.

## Adding a provider

Providers live in [`crates/tokenza-core/src/providers`](crates/tokenza-core/src/providers). Each one implements the `Provider` trait from [`provider.rs`](crates/tokenza-core/src/provider.rs):

- `id()`: a stable lowercase identifier.
- `min_interval()`: the shortest polling interval the service tolerates.
- `detect()`: find local credentials and return an `Account`, without any network access.
- `fetch()`: call the usage endpoint and map the response to a `UsageSnapshot` of `UsageWindow`s. Return `ProviderError::SchemaChanged` when the response has no recognisable data, so the UI can say so instead of showing wrong numbers.

Steps:

1. Add `providers/<name>.rs`, declare it in `providers/mod.rs` and add it to `all_providers()`.
2. Write unit tests for the response parser using fake JSON bodies (see `claude.rs`).
3. Register the id in the app: `KNOWN_PROVIDERS` and the display-name match in `src-tauri/src/settings.rs`, `PROVIDER_NAMES` in `src/format.ts`, `DEFAULT_ORDER` and the data-source hint in `src/ProvidersPane.tsx`, and a logo in `src/logoPaths.ts` (providers without a logo get a neutral initial).
4. Add localized window labels if the provider introduces a new kind of limit window.
5. Document the new domain and credential location in the README privacy table.

## Translations

UI strings live in [`apps/tokenza/src/locales`](apps/tokenza/src/locales) (one file per language, typed against `en.ts`) and native strings for the tray and notifications in [`apps/tokenza/src-tauri/src/i18n.rs`](apps/tokenza/src-tauri/src/i18n.rs).

- To fix a string, edit it in both places if it appears in both.
- To add a language, copy `en.ts`, translate every key, register the language in `src/i18n.ts` (`LANGS` and the dictionary map) and add a matching table in `i18n.rs`. The existing tests in `i18n.test.ts` check that every language defines every key and placeholder.

## Reporting bugs

A good report includes the Tokenza version, Windows version, which provider is affected, what you expected and what happened, and any text shown on the provider card. **Never paste tokens, credential files or full API responses that may contain personal data.**

## License

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
