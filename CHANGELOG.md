# Changelog

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
