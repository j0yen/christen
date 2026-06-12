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

## christen cap — capability grant advisor

`christen cap` detects whether `agentns-claude` / `agent-wrap` carry the
`cap_sys_admin+ep` file capability required for `unshare(CLONE_NEWAGENT)` to
succeed.  It **prints** the exact `sudo setcap` line the user must run and
explains the precise scope of the grant.  It **never** runs `setcap`
automatically — the grant is the user's choice.

### Why the capability is required

`agent-wrap.c` and `agentns-claude` both document: "Needs CAP_SYS_ADMIN.
The intended deployment is file caps: `sudo setcap cap_sys_admin+ep
/home/jsy/.local/bin/agent-wrap`."  Without this capability,
`unshare(CLONE_NEWAGENT)` fails with `EPERM` and the launcher silently falls
back to an unwrapped exec — every session is still born with
`agent_session = 0…0` and the `agentns-session-zeros` docket item never
clears.  `christen route`'s wiring (the systemd drop-ins) is inert until
this cap is granted: the drop-ins correctly route through `agentns-claude`,
but `agentns-claude` itself cannot create the namespace without the cap.

### Scope mitigation — why a file capability is safe

`CAP_SYS_ADMIN` is broad (mounts, namespaces, many privileged kernel calls).
The scoped mitigation applied here:

- **File-scoped**: only the one audited binary receives the capability — not
  the system, not your shell, not any other process.
- **Not setuid-root**: the binary runs as your user; the capability is only
  available to that specific binary on exec.
- **+ep (Effective + Permitted)**: the kernel sets the capability in the
  process's effective set on exec, allowing the `unshare` call to succeed
  without any other privilege escalation.
- **Exec boundary reset**: `agent-wrap` calls `exec(2)` into `agentns-claude`
  which calls `exec(2)` into `claude` — file capability rules reset at each
  exec, so `CAP_SYS_ADMIN` does not propagate into the final `claude` process.

### Print-only grant flow

```sh
# Print scope explainer + per-binary state + the setcap command to run
christen cap

# JSON output (machine-readable)
christen cap --format json

# After granting — verify the cap worked without re-running setcap
christen cap --verify
```

`christen cap` never executes `setcap`.  The output includes a fixed scope
explainer block (see `SCOPE_EXPLAINER` in `src/cap.rs`) that must appear
before any `setcap` line — asserted by tests.

### Post-grant verification (`--verify`)

`christen cap --verify` spawns `agentns-claude` (or `agent-wrap`) under `sbx`
and reads the child's `/proc/$pid/agent_session`:

| Result | Meaning |
|--------|---------|
| `Live { session_id }` | Nonzero session id — cap granted and unshare succeeded |
| `EPERM-fallback` | Session id is all-zeros — cap not yet granted (or unshare fell back) |
| `Absent` | Kernel does not have `CONFIG_AGENT_NS=y` |
| `Error { detail }` | Launcher not found or spawn failed |

This is an I/O-performing function; all other cap logic (`cap_plan`) is pure.
AC6 (verify on `-wintermute` kernel) is marked **deferred** — it requires the
live kernel + real launcher + `sbx`, none of which are available in CI.

### Open decision — setuid-helper alternative

An alternative narrower-privilege approach exists: a minimal **setuid-root
helper** binary that calls `unshare(CLONE_NEWAGENT)` and immediately drops
all capabilities before exec-ing into the actual launcher.

| | File cap on launcher | Setuid helper |
|---|---|---|
| **Privilege scope** | `CAP_SYS_ADMIN` on one audited binary | Setuid-root on a tiny helper; drops caps before exec |
| **Attack surface** | Larger binary, but no setuid bit | Smaller binary, but setuid-root |
| **Auditability** | Straightforward — `getcap` shows the grant | Requires auditing the drop sequence |
| **Complexity** | One `sudo setcap` line | Extra helper binary to build + install |
| **Deployment** | Already documented in `agent-wrap.c` | Not yet implemented |

**Current recommendation**: use the file capability on `agent-wrap` /
`agentns-claude` as documented in the source.  The setuid-helper path is
recorded here as an open decision; no code path auto-installs either.

## Install

```sh
cargo install --path .
```

## License

MIT © Joe Yen
