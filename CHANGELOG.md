# Changelog

## v0.6.0 — 2026-06-12

christen-route: docs(route) add christen route section to README (AC8)

Route subcommand (SystemdSource + render_dropin + apply_route, ACs 1-6) was
already in-tree from v0.3.0. This commit adds the missing README documentation
(AC8): drop-in format, print-vs-apply boundary, systemd-analyze verify gate,
and christen cap prerequisite. All route tests green (AC1-AC6).

## v0.5.0 — 2026-06-12

docs(route): add christen route section to README (AC8)

Documents the drop-in format (clear-then-set ExecStart= pair), the print-vs-apply
boundary, the systemd-analyze verify gate, and the christen cap prerequisite.
Route implementation (SystemdSource, render_dropin, apply_route) + tests (AC1-6)
were already in-tree from v0.3.0; this commit completes the final documentation AC.

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
