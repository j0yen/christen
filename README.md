# christen

Launch-site model and route plan for agent-namespace wiring.

`christen` is the foundational crate of the christen fleet. It provides:

- **Shared types** — `LaunchSite`, `SiteKind`, `WrapState`, `RouteAction`, `RoutePlan`
- **`LaunchSiteSource` trait** — the contract for discovery that downstream crates implement
- **`FakeSource`** — in-memory fixture source for tests
- **`plan()`** — a pure function that turns `&[RawSite]` + `KernelInfo` + `wrapper_installed` into a `RoutePlan`
- **`christen plan`** — subcommand that prints (or JSON-emits) the route plan for all discovered launch sites

## Why this exists

The wintermute agent-namespace substrate is built and booted but inert:
every Claude session reads `agent_session = 00000000...` because no launch path
routes through `agentns-claude`. `christen-plan` is the prerequisite — it
models where sessions are born and what change would fix each site — so that
`christen-route` can apply the changes, `christen-cap` can set capabilities,
`christen-detect` can measure live state, and `christen-ledger` can audit history.

## Usage

```
christen plan [--format table|json] [--config <path>]
```

Exits non-zero when ≥1 site is `Unwrapped` on a `-wintermute` kernel with the
wrapper installed (so a hook can gate on it).

## Configuration

Config lives at `~/.config/christen/christen.toml` (optional). An example:

```toml
default_budget = "wall=7200s,fork=2000"
systemd_dir = "/home/jsy/.config/systemd/user"

[intent_overrides]
"claude-build.service" = "/build"
```

All fields are optional. `ChristenConfig::load(path)` returns defaults silently
when the file is absent. Loading is pure — no filesystem scan, no unit parse,
no `/proc` read.

## Type surface

### `SiteKind`

Where a session is spawned:

```rust
pub enum SiteKind {
    SystemdUnit { unit: String, exec_start: String },
    ShellRc { path: PathBuf },
    Hook,
    Other { note: String },
}
```

### `WrapState`

Whether a site routes through the agent-namespace wrapper:

```rust
pub enum WrapState {
    Unwrapped,
    Wrapped { via: String },
    Uncertain,
}
```

Detection is pure string-match on the exec line — no live `/proc` read.
That is `christen-detect`'s job.

### `LaunchSite`

A discovered site with its classification:

```rust
pub struct LaunchSite {
    pub id: String,
    pub kind: SiteKind,
    pub wrap: WrapState,
    pub intent: String,    // derived intent tag, e.g. "/build"
}
```

### `RouteAction`

A declarative change (no side effects):

```rust
pub enum RouteAction {
    Wire { site, from, to },        // rewrite exec line to use agentns-claude
    Advise { site, snippet },       // shell snippet for user sites
    AlreadyWrapped { site },        // no-op
    Skip { site, reason },          // kernel or wrapper absent
}
```

### `RoutePlan`

```rust
pub struct RoutePlan {
    pub actions: Vec<RouteAction>,
    pub to_wire: usize,
    pub advised: usize,
    pub already: usize,
    pub skipped: usize,
}
```

## `intent_for` derivation table

| Site id | Derived intent |
|---|---|
| `claude-build.service` | `/build` |
| `claude-dream.service` | `/dream` |
| `claude-self-review.service` | `/self-review` |
| `interactive` | `interactive` |
| _(any other)_ | `unknown` |

`intent_overrides` in `christen.toml` wins over the built-in table.

## `LaunchSiteSource` trait

```rust
pub trait LaunchSiteSource {
    fn sites(&self) -> Result<Vec<RawSite>, SourceError>;
}
```

`plan()` does **not** call this trait — the caller resolves sites and passes
`&[RawSite]` directly. The trait exists so `christen-route` (real `SystemdSource`),
`christen-detect` (reads `/proc`), and `christen-cap` all share one contract.

`FakeSource` is the in-memory implementation for tests.

## Downstream crates

| Crate | Role |
|---|---|
| `christen-route` | Implements `SystemdSource`, applies `Wire` actions |
| `christen-cap` | Sets `cap_sys_admin+ep` on wrapper binaries |
| `christen-detect` | Reads live `/proc/self/ns/agent` and `agent_session` |
| `christen-ledger` | Audits session-id history against the plan |

All extend the `LaunchSiteSource` trait and accept a `RoutePlan` from `plan()`.

## MSRV

Rust 1.85. No let-chains.

## License

MIT OR Apache-2.0
