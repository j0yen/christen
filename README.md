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

## christen probe — namespace state classifier

`christen probe` reads the `/proc` agent-namespace surface for the current
process (or a target PID) and classifies it into one of four states with
correct, actionable prose.

### The four states

| State | Meaning | `ok` | Docket op |
|-------|---------|------|-----------|
| `absent` | `/proc/$pid/ns/agent` symlink missing | `true` on stock kernel, `false` on `-wintermute` | `Report` (wintermute only) |
| `init` | Session id is all-zeros or ns inode is the init inode | `true` (not a fault) | `Report` actionable (wintermute + wrapper) or `None` |
| `live` | Non-zero session id — session is in its own namespace | `true` | `Resolve agentns-session-zeros` |
| `malformed` | Surface present but unparseable | `false` | `Report` |

### Init-inode detection

On Linux the agent-namespace init inode is `4026531996`. A process born in
the initial namespace (before any launch site is routed through `agentns-claude`)
will have `/proc/self/ns/agent -> agent:[4026531996]` and a session id of all
zeros. This is **not** a fault — it means the wrapper is installed but exec
lines have not been rewritten yet. The fix is `christen route`.

### Anti-regression invariant

The string `"registration failed"` **never** appears in any `prose` returned
by `christen probe`. This invariant is asserted by a dedicated test
(`probe_ac3_antiregression`) that iterates every `NsState` variant.

### `--emit` docket contract

When `--emit` is passed, `christen probe` shells `docket` with the mapped op:

- `live` → `docket resolve agentns-session-zeros`
- `init` (UnwrappedExpected, -wintermute) → `docket report --severity warn --title "agentns init NS — launches not routed through agentns-claude; run christen route" ...`
- `absent` (-wintermute) → `docket report --severity warn ...`
- any stock-kernel state → no docket op

A missing `docket` binary is non-fatal: the probe prints its classification
and exits on state, not on the docket failure.

### Usage

```sh
# Classify current process (human-readable text)
christen probe

# Classify current process and apply docket edge-trigger
christen probe --emit

# Classify a specific PID as JSON
christen probe --pid 1234 --format json

# JSON schema includes: state, ok, prose, session_hex?, intent?, docket_op
```

## Install

```sh
cargo install --path .
```

## License

MIT © Joe Yen
