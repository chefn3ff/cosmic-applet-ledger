# Changelog

All notable changes to Ledger are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
