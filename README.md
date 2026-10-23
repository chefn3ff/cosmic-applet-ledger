<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="resources/ledger-icon/svg/ledger-lockup-mark-dark.svg">
  <img src="resources/ledger-icon/svg/ledger-lockup-mark-dark.svg" alt="Ledger" width="360">
</picture>

<br>
<br>

**A flat, minimalist COSMIC panel applet for tracking AI usage, rate limits, and tokens across coding agents and providers.**

[![CI](https://github.com/mrn3ff/cosmic-applet-ledger/actions/workflows/ci.yml/badge.svg)](https://github.com/mrn3ff/cosmic-applet-ledger/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/mrn3ff/cosmic-applet-ledger)](https://github.com/mrn3ff/cosmic-applet-ledger/releases/latest)
[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-blue)](LICENSE)

*Forked from [YapCap](https://github.com/TopiCsarno/yapcap) by Topi Csarno, redesigned with a minimalist aesthetic inspired by Omarchy.*

</div>

---

Ledger sits in the COSMIC panel and shows how much of your AI quota you've used — session and weekly limits, spend, and token usage — for every provider you connect. It talks to provider APIs directly with accounts you add in Ledger. No telemetry, no cloud sync, no third-party servers.

## Features

- **One layout for every provider.** Each provider's page shows a **LIMITS** card with its session and weekly windows and a reset countdown.
- **Token charts.** **TOKENS BY DAY** covers the last seven days (with today highlighted) and **TOKENS BY MODEL** ranks your most-used models. Available for Claude and OpenRouter.
- **Thirteen providers** — see [Providers](#providers).
- **Multiple accounts per provider**, with an in-app sign-in flow for each.
- **Automatic detection.** Providers found on your machine are enabled for you; the rest can be switched on in Settings.
- **Update notices.** Ledger checks GitHub for a new release on startup and shows it on the About page. It never downloads or installs anything itself.

## Providers

| Provider | Sign in with | Shows |
| --- | --- | --- |
| Antigravity | Google (browser) | Model-group quota (5-hour and weekly) |
| Claude | Claude (browser, paste the code back) | Session and weekly limits; tokens by day and model from Claude Code |
| Codex | OpenAI (browser), or import from OpenCode | 5-hour and weekly limits |
| Copilot | GitHub device login, or import from OpenCode | Chat/completions (Free) or premium requests (paid) |
| Cursor | Scans the Cursor IDE's local sign-in | Total and API usage |
| ElevenLabs | API key | Monthly credit usage and reset, credits by API key; optional credits ring on every other provider |
| Gemini | Google (browser) — enable manually | Pro, Flash and Lite quota |
| Grok | xAI (browser), or import from the Grok CLI | Weekly credit usage |
| Kimi | API key | Weekly and rate-limit windows |
| Minimax | API key | 5-hour and weekly windows |
| OpenCode Go | API key | 5-hour and weekly windows |
| OpenRouter | API key | Daily and weekly spend, credit balance, tokens by day and model |
| Z.AI Coding Plan | API key | 5-hour and weekly windows |

API-key forms for Kimi, Minimax, OpenCode Go and Z.AI can be prefilled from OpenCode's `~/.local/share/opencode/auth.json`. Ledger only reads that file when you add an account, and never writes to it.

**OpenRouter:** use a management key to see spend across all of your keys and today's token usage. A regular API key only reports its own spend, and token history stops at yesterday.

## Install

### From a release

Download a package from the [latest release](https://github.com/mrn3ff/cosmic-applet-ledger/releases/latest).

```bash
# Debian, Ubuntu, Pop!_OS
sudo apt install ./cosmic-applet-ledger_*_amd64.deb

# Fedora, openSUSE
sudo rpm -i ./cosmic-applet-ledger-*.x86_64.rpm
```

A `.tar.gz` with the prebuilt binary and resources is also attached to each release.

### From source

You need Rust 1.88 or newer, `just`, and the COSMIC build dependencies:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

sudo apt install -y build-essential pkg-config just libssl-dev \
  libxkbcommon-dev libwayland-dev libfontconfig1-dev libfreetype6-dev libexpat1-dev
```

Then build and install. `just install` installs under `~/.local` and adds Ledger to your panel:

```bash
git clone https://github.com/mrn3ff/cosmic-applet-ledger
cd cosmic-applet-ledger
just install
```

Remove it again with `just uninstall`.

## Getting started

1. If you installed a package, add Ledger to the panel: **COSMIC Settings → Desktop → Panel → Configure panel applets**, then add **Ledger**.
2. Click the panel icon to open Ledger. Providers detected on your machine already have a tab.

   <img src="resources/screenshots/screenshot-panel.png" alt="Ledger's icon in the COSMIC panel" width="560">

3. Open **Settings** (the gear icon). Under **PROVIDERS**, switch on any provider you want that wasn't detected.

   <img src="resources/screenshots/screenshot-settings.png" alt="Settings: providers, preferences and about" width="320">

4. Click a provider's name to open its account page and add an account.

   <img src="resources/screenshots/screenshot-accounts.png" alt="Claude account settings" width="320">

Once an account is connected, the provider's tab shows its limits and usage. Use the `…` button to switch providers and the refresh button to update right away.

<table>
  <tr>
    <th width="33%">Claude</th>
    <th width="33%">OpenRouter</th>
    <th width="33%">Codex</th>
  </tr>
  <tr>
    <td valign="top"><img src="resources/screenshots/screenshot-claude.png" alt="Claude: limits, tokens by day and tokens by model" width="100%"></td>
    <td valign="top"><img src="resources/screenshots/screenshot-openrouter.png" alt="OpenRouter: daily and weekly spend with token charts" width="100%"></td>
    <td valign="top"><img src="resources/screenshots/screenshot-codex.png" alt="Codex: session and weekly limits" width="100%"></td>
  </tr>
</table>

<sub>Screenshots use demo data from <code>just run-demo</code>.</sub>

### Settings

- **Panel Status Bars** — show usage bars in the panel, or just the Ledger icon.
- **Refresh Interval** — how often Ledger polls provider APIs: 1, 5 (default), 15 or 30 minutes.
- **Providers** — turn providers on or off and manage their accounts.

## Where your data lives

| Path | Contents |
| --- | --- |
| `~/.config/cosmic/io.github.mrn3ff.cosmic-applet-ledger/` | Settings, enabled providers and account list |
| `~/.local/state/cosmic-applet-ledger/<provider>-accounts/` | Ledger's copy of each account's credentials |
| `~/.local/state/cosmic-applet-ledger/logs/ledger.log.YYYY-MM-DD` | Daily logs |

Credentials are stored as files readable only by you (`0600`, in `0700` folders), not in a system keyring. Removing an account in Ledger deletes only Ledger's copy; your provider account and other tools' sign-ins are untouched.

Ledger also reads, but never changes:

- `~/.claude/projects/` — Claude Code session logs, for Claude's token charts
- `~/.grok/auth.json` — only when you choose to import from the Grok CLI
- `~/.local/share/opencode/auth.json` — only to prefill account forms
- The Cursor IDE's local sign-in, when you scan for Cursor accounts

Logs never contain credentials or tokens. If you find one that does, please [open an issue](https://github.com/mrn3ff/cosmic-applet-ledger/issues).

## Development

```bash
just run-demo         # standalone window with demo accounts for every provider
cargo test            # unit tests
```

CI runs `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` on every push; run them before opening a pull request. Pushing a `v*` tag builds the `.deb`, `.rpm` and tarball and publishes a GitHub release.

See [CHANGELOG.md](CHANGELOG.md) for release history.

## License

Licensed under the Mozilla Public License 2.0 — see [LICENSE](LICENSE).

*Original codebase and provider engine: YapCap © Topi Csarno.*
