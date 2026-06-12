# Changelog

## v0.4.1 — 2026-06-12

feat(ledger): add README ledger section (AC8)

Documents the LedgerEntry schema, open-only-as-SIGKILL-signal semantics,
agentns-doctor receipt relationship, session id attribution to agorabus/
memlog/provfs, and full CLI usage examples. Ledger module + tests (AC1-6)
were already in-tree; this commit completes the missing documentation AC.

## v0.2.0 — 2026-06-09

feat(cap): add `christen cap` subcommand — detect cap_sys_admin+ep file capability on launcher binaries, print exact `sudo setcap` command (never auto-executes), verify via sbx/agent_session read. AC1-5 tests green; AC6 deferred (wintermute kernel required).

## [Unreleased]

### Added
- Initial scaffold: `LaunchSite`, `SiteKind`, `WrapState`, `RouteAction`, `RoutePlan` types
- `LaunchSiteSource` trait and `FakeSource` in-memory implementation
- Pure `plan()` function: `&[RawSite]` + `KernelInfo` + `wrapper_installed` → `RoutePlan`
- `ChristenConfig::load()` with TOML config support and documented defaults
- `intent_for()` derivation table with `intent_overrides` config support
- `christen plan` subcommand: table and `--format json` output
- Exit non-zero when ≥1 site is `Unwrapped` on a `-wintermute` kernel with wrapper installed
- Full acceptance test suite (AC1–AC8)
- Property-based invariant tests via `proptest`
