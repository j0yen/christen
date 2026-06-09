# christen

The wintermute agent-namespace substrate is built and booted but inert:

## Overview

The wintermute agent-namespace substrate is built and booted but inert:
every session is born in the *initial* namespace with session id `0…0`
because no launch path routes through `agentns-claude`. Before anything can
fix that, there must be a typed, testable model of **where sessions are
born** (`LaunchSite`), **whether each is wrapped** (`WrapState`), and **what
edit would route it through the wrapper** (`RouteAction`). **christen-plan**
creates the `christen` workspace and that foundation: the shared types, a
`LaunchSiteSource` trait that abstracts discovery, a `FakeSource` for tests,
and a **pure** `plan(sites, kernel, wrapper_installed)` that emits a
print-only `RoutePlan`. `christen plan` shows it at a glance. The rest of
the vision extends this crate.


## Acceptance


1. `cargo build` and `cargo test` succeed offline; a test asserts `plan`
   makes **zero** source calls (it operates only on the passed `&[RawSite]`
   + injected `KernelInfo` + `wrapper_installed`).
2. `LaunchSite`, `SiteKind`, `WrapState`, `RouteAction`, `RoutePlan` are
   public and `serde`-(de)serializable; a round-trip test covers each.
3. `ChristenConfig::load` parses `config/christen.example.toml`; a fixture
   yields the expected `default_budget` + `systemd_dir` + one
   `intent_overrides` entry, and an absent file yields documented defaults.
4. `plan` classifies correctly against `FakeSource` fixtures: a systemd site
   whose exec line lacks `agentns-claude` on a `-wintermute` kernel with the
   wrapper installed → `Unwrapped` + `Wire` (the `to` line contains
   `agentns-claude --intent <derived> --budget <default> --`); a site whose
   exec line already contains `agentns-claude` → `Wrapped` + `AlreadyWrapped`;
   a shell-rc site → `Advise`; any site on a kernel with `agent_ns:false` or
   `wrapper_installed:false` → `Skip` with the documented reason.
5. `intent_for` derives `/build`/`/dream`/`/self-review`/`interactive` for
   the four canonical site ids, and `intent_overrides` from config wins over
   the derivation; both covered by tests.
6. `christen plan --format json` emits one entry per site plus the
   `RoutePlan` tallies (`to_wire`/`advised`/`already`/`skipped`); schema
   matches the documented `RoutePlan`.
7. `christen plan` exits non-zero when ≥1 site is `Unwrapped` (wrapper
   installed, `-wintermute` kernel), zero otherwise; two integration cases
   driving `FakeSource`. `christen plan | head -1` does not panic (SIGPIPE
   reset verified by a test that closes the read end early).
8. README documents the config format, the type surface, the `intent_for`
   derivation table, and the `LaunchSiteSource` trait so christen-detect /
   christen-route / christen-cap / christen-ledger have a contract to extend.

## Install

```sh
cargo install --path .
```

## License

MIT © Joe Yen
