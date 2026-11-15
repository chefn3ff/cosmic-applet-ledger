# Changelog

All notable changes to Ledger are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-11-14

### Changed
- Provider logos are black and white by default, following the theme like
  the rest of the applet, in the panel and the popup alike.

### Added
- **Color Provider Icons** preference (first under OPTIONAL) brings back each
  provider's brand colors.
- `LEDGER_SHOT_ONLY` re-shoots selected README screenshots.

### Fixed
- A refreshed token whose save fails (disk full, permissions) is kept in
  memory and saved on the next read, so a rotated refresh token is never lost.
- Sign-in callbacks: one byte-safe decoder replaces four copies that could
  panic on unusual input.
- Codex, Gemini, Antigravity and Grok sign-in listeners now time out a silent
  connection after 10 seconds, give up after 5 minutes, read requests split
  across packets, and skip a stray request or a wrong `state` instead of
  ending the sign-in.

## [0.4.1] - 2026-11-14

### Changed
- App ID is now `io.github.chefn3ff.cosmic-applet-ledger`, matching the
  GitHub account. On first start Ledger copies your settings from the old
  `io.github.mrn3ff.cosmic-applet-ledger` folder (the old folder is kept).
  `just install` removes the old panel entry and files. **If you installed a
  .deb or .rpm, add Ledger to the panel again after upgrading.**
- Repository, About and update-check links point at `chefn3ff` directly.
- README rewritten: what Ledger adds to YapCap, a gallery of every provider
  page, a credit rings section, and a layout that reads well on a phone. All
  screenshots retaken from demo data.
- Demo mode no longer checks GitHub for updates, and its Fish Audio figures
  are made up.

### Added
- `just screenshots` regenerates every README screenshot from demo data.
- Debug builds: `LEDGER_DEBUG_POPUP_WINDOW` also accepts `switch` and
  `accounts`.

## [0.4.0] - 2026-11-14

### Added
- Fish Audio provider (API key). Shows your **API credit**, the pay-as-you-go
  balance that audio API calls spend, as dollars left of what you have topped
  up (or of your balance, when free credit puts it above your top-ups). It
  uses Fish Audio's own mark. It does not read Plus-plan or subscription credits. Fish Audio account
  in demo mode (`LEDGER_DEMO=1`).
- **Fish Credits Ring** preference, like the ElevenLabs one. With both rings
  on, the header shows one ring; click it to switch to the other service. The
  choice is remembered.
- Debug builds: `LEDGER_DEBUG_POPUP_WINDOW=1` (or `settings`) shows the popup
  in a normal window, for screenshots without a panel.

### Changed
- The Switch Provider button is now two stacked opposing arrows.
- PREFERENCES lists Refresh Interval first, then Panel Status Bars, then an
  OPTIONAL group with the credit rings.
- Every changed setting now syncs across Ledger processes by its name, so
  OpenRouter changes no longer stay in the process that made them.

### Fixed
- An ElevenLabs key without the User: Read permission now says so, instead of
  asking you to sign in again. Pasting a key ID (not starting with `sk_`) is
  refused when you save it.
- OpenRouter rate limits honour Retry-After, and its API key header is marked
  sensitive.
- Credential files are written atomically (temp file, fsync, rename), so a
  crash right after a token refresh can no longer lose the new token.
- Refresh-owner tests no longer flake when the suite runs in parallel.
- Credit rings draw the selected account, not the first account with cached
  data.

## [0.3.0] - 2026-10-23

### Added
- ElevenLabs provider (API key). Shows monthly credit usage, the plan tier
  and when credits reset.
- ElevenLabs **CREDITS BY KEY** card: credits per API key for the current
  billing period, each bar scaled to the plan limit. Needs workspace metrics
  access; the page still loads without it.
- **ElevenLabs Credits Ring** preference: when ElevenLabs is enabled, shows a
  credits gauge in every other provider's header, with credits and reset
  time on hover.
- ElevenLabs account in demo mode (`LEDGER_DEMO=1`).

## [0.2.2] - 2026-10-18

### Security
- Claude sign-in now uses a separate random OAuth `state` value. It was
  reusing the PKCE code verifier, which exposed that secret in the
  authorization URL and browser history.
- Updated rustls to 0.23.46, h2 to 0.4.20 and openssl to 0.10.81 to pick
  up fixes for published security advisories.

## [0.2.1] - 2026-09-27

### Fixed
- The Debian package is now named `cosmic-applet-ledger`, matching the
  RPM and tarball. It was published as `ledger`, which clashes with the
  unrelated `ledger` accounting package in Debian and Ubuntu. Installing
  0.2.1 replaces an earlier `ledger` build cleanly.

## [0.2.0] - 2026-09-27

### Added
- OpenRouter provider (API key). Shows today's and this week's spend,
  remaining credit balance, tokens by day (including today) and tokens by
  model. Management keys report account-wide spend across all keys.
- TOKENS BY DAY (last seven days) and TOKENS BY MODEL sections in the
  provider detail view. Populated for Claude and OpenRouter.
- Claude token stats read from Claude Code's local session transcripts
  (`~/.claude/projects`, or `$CLAUDE_CONFIG_DIR`).
- "Icon only" panel icon style.
- Ledger icon set: app, symbolic, lockup and web icons.
- OpenRouter account in demo mode (`LEDGER_DEMO=1`).

### Changed
- Redesigned the popup, settings, preferences and account screens with a
  flat, minimalist look.
- Providers are listed alphabetically in the applet and demo mode, and in
  the code.
- Token chart bars scale against fixed token tiers instead of the busiest
  day, so bars are comparable between refreshes.

### Fixed
- Providers that report a single limit window (e.g. Grok) no longer show
  it as both Session and Weekly.
- TOKENS BY DAY is hidden for providers that report no token data instead
  of showing seven empty bars.
- The detail view explains when a provider is detected but has no
  account, or needs a login, instead of showing nothing.
- The test suite compiles and passes again: fixtures gained the token
  usage fields, storage-path tests use the rebranded
  `cosmic-applet-ledger` directory, and UI tests match the redesign.

## [0.1.0] - 2026-09-26

### Added
- Forked from [YapCap](https://github.com/TopiCsarno/yapcap) `main` after
  v0.6.0, including upstream work not yet released there: the Grok and
  Z.AI Coding Plan providers and a Polish translation.
- Rebranded applet as Ledger (`cosmic-applet-ledger`).
- Cleaned metadata, D-Bus IDs, and app registration for COSMIC.

---

### Historical YapCap Changes (Pre-fork)

See original repository: https://github.com/TopiCsarno/yapcap
