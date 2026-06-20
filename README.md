# christen

A typed model of where agent sessions are born, and a plan for routing each one through the agent-namespace wrapper.

## Why it exists

The wintermute agent-namespace substrate is built and boots, but it's inert: every session is born in the *initial* namespace with session id `0…0`, because no launch path routes through `agentns-claude`. Fixing that safely starts with a question you have to answer before you edit anything — *where do sessions actually get launched, which are already wrapped, and what edit would route each one through the wrapper?* christen answers it as data: a typed, testable model of launch sites and a pure function that turns them into a route plan. It plans; it does not edit. The detection, routing, and capability tooling that act on the plan extend this crate.

## Install

```sh
cargo install --path .
```

## Quickstart

```sh
christen plan                              # table of launch sites + the action each needs
christen plan --format json                # the RoutePlan as JSON
christen plan --config ./christen.toml     # use a specific config
```

`christen plan` discovers systemd user units, classifies each launch site, and prints what would route it through `agentns-claude`. It **exits non-zero** when at least one site is `Unwrapped` on a `-wintermute` kernel with the wrapper installed — so it works as a CI or boot-time check that the fleet is fully wired.

## The model

A launch site is classified into an action by a pure function — `plan(sites, kernel, wrapper_installed, config)` — that makes zero discovery calls; it operates only on data passed in, which is what makes it testable.

| Type | What it captures |
|---|---|
| `SiteKind` | how a session is launched: `SystemdUnit`, `ShellRc`, `Hook`, `Other` |
| `WrapState` | whether the site already routes through the wrapper |
| `RouteAction` | the action a site needs: `Wire`, `Advise`, `AlreadyWrapped`, `Skip` |
| `RoutePlan` | the full set of actions plus tallies (`to_wire` / `advised` / `already` / `skipped`) |
| `KernelInfo` | the kernel's `agent_ns` support and release string |

The action rules:

- A systemd site whose exec line lacks `agentns-claude`, on a `-wintermute` kernel with the wrapper installed → `Wire` (the proposed line injects `agentns-claude --intent <derived> --budget <default> --`).
- An exec line already containing `agentns-claude` → `AlreadyWrapped`.
- A shell-rc site → `Advise` (printed guidance, not an automatic edit).
- Any site on a kernel without agent-ns support, or with the wrapper absent → `Skip`, with a documented reason.

All types are `serde`-serializable and round-trip tested.

## Intent derivation

Each site gets an intent tag, derived from its id and overridable in config:

| Site id | Intent |
|---|---|
| `claude-build.service` | `/build` |
| `claude-dream.service` | `/dream` |
| `claude-self-review.service` | `/self-review` |
| `interactive` | `interactive` |
| anything else | `unknown` |

An `[intent_overrides]` entry in config wins over the derivation.

## Configuration

Config is optional; every field has a documented default. Place it at `~/.config/christen/christen.toml` or pass `--config <path>`. See [`config/christen.example.toml`](config/christen.example.toml):

```toml
# Budget string injected into every Wire action.
default_budget = "wall=7200s,fork=2000"

# Directory scanned for systemd user units.
systemd_dir = "/home/jsy/.config/systemd/user"

# Override the derived intent tag for a given site id.
[intent_overrides]
"claude-build.service" = "/build"
```

## Status

This is the foundation crate, not the whole system. It establishes the types, the `LaunchSiteSource` trait that abstracts discovery, a `FakeSource` for tests, and the pure `plan()` planner. `christen plan` ships a working planner over a naive systemd scan; the production `SystemdSource` and the tools that act on a plan (christen-detect / christen-route / christen-cap / christen-ledger) extend this contract. Acceptance criteria AC1–AC8 and proptest invariants pass under `cargo test`.

## License

MIT or Apache-2.0, at your option.
