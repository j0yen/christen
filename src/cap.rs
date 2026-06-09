//! Capability detection and grant-plan for `christen cap`.
//!
//! This module is intentionally **read-only and declarative**: it detects
//! whether `agentns-claude` / `agent-wrap` carry the `cap_sys_admin+ep` file
//! capability, explains the precise scope of the grant, and *prints* the exact
//! `sudo setcap` line the user must run.  It never shells `setcap` itself.
//!
//! # Security note
//!
//! `CAP_SYS_ADMIN` is a broad capability.  The scoped mitigation is that this
//! is a **file capability** (`+ep`) on a single non-setuid, audited binary —
//! not a global change, not a setuid-root binary.  The user sees the exact
//! command and must choose to run it.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

// ── Scope explainer (printed before any setcap line) ─────────────────────────

/// The fixed scope-explainer block printed before any grant recommendation.
///
/// A test asserts this text appears before the `setcap` line in all output.
pub const SCOPE_EXPLAINER: &str = "\
=== CAP_SYS_ADMIN scope explainer ===
CAP_SYS_ADMIN is a broad capability: it allows operations such as mounting
filesystems, creating namespaces, and various privileged kernel calls.

Why a file capability is the right mitigation here:
  - The grant is FILE-SCOPED: only the one audited binary (agentns-claude or
    agent-wrap) receives the capability — not the system, not your shell, not
    any other process.
  - The binary is NOT setuid-root: it runs as your user; the capability is
    only available to that specific binary.
  - The scope is +ep (Effective + Permitted): the kernel sets the capability
    in the process's effective set on exec, allowing the unshare(CLONE_NEWAGENT)
    call to succeed without any other privilege escalation.
  - Once the namespace is created the binary does not retain CAP_SYS_ADMIN
    across the exec boundary into the child (agent-wrap calls exec(2) into
    agentns-claude which calls exec(2) into claude — capability rules reset).

The grant is YOURS to run.  christen cap only prints the command; it never
runs setcap automatically.
=====================================
";

// ── CapState ─────────────────────────────────────────────────────────────────

/// The capability state of a single launcher binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CapState {
    /// The binary has `cap_sys_admin` in its effective+permitted file capability set.
    Present,
    /// The binary exists but does not carry `cap_sys_admin`.
    Absent,
    /// The binary could not be read (missing, permission error, `getcap` absent).
    Unreadable {
        /// Human-readable detail (error message or tool absence note).
        detail: String,
    },
    /// The binary has the setuid bit set — a red flag to surface, not grant over.
    Setuid {
        /// Warning message.
        warn: String,
    },
}

// ── CapReader trait ───────────────────────────────────────────────────────────

/// Reads file capabilities from a binary path.
///
/// The trait exists so tests can inject a `FakeReader` without touching the
/// real filesystem or requiring `getcap` to be installed.
pub trait CapReader {
    /// Return the [`CapState`] for the binary at `path`.
    fn caps(&self, path: &Path) -> CapState;
}

// ── Real impl (shells `getcap`) ───────────────────────────────────────────────

/// Real [`CapReader`] that calls the system `getcap` tool.
pub struct GetcapReader;

impl CapReader for GetcapReader {
    fn caps(&self, path: &Path) -> CapState {
        // First check setuid bit — red flag.
        if let Ok(meta) = std::fs::metadata(path) {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = meta.permissions().mode();
            if mode & 0o4000 != 0 {
                return CapState::Setuid {
                    warn: format!(
                        "{} has the setuid bit set — resolve this before granting file caps",
                        path.display()
                    ),
                };
            }
        } else {
            return CapState::Unreadable {
                detail: format!("binary not found or not accessible: {}", path.display()),
            };
        }

        // Call `getcap <path>`.
        let output = match Command::new("getcap").arg(path).output() {
            Ok(o) => o,
            Err(e) => {
                return CapState::Unreadable {
                    detail: format!("getcap not available or failed to launch: {e}"),
                }
            }
        };

        if !output.status.success() && output.stdout.is_empty() {
            // getcap exits non-zero when there are no caps; empty stdout also means absent.
            return CapState::Absent;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // getcap output format: "<path> = cap_sys_admin+ep" (may omit trailing newline).
        // We look for `cap_sys_admin` with `+ep` or `+eip` (effective + permitted).
        let has_sys_admin = stdout.to_lowercase().contains("cap_sys_admin");
        let has_effective = stdout.contains("+ep") || stdout.contains("+eip") || stdout.contains("+pie");

        if has_sys_admin && has_effective {
            CapState::Present
        } else if stdout.trim().is_empty() || !has_sys_admin {
            CapState::Absent
        } else {
            // Has sys_admin in name but not in effective set.
            CapState::Absent
        }
    }
}

// ── FakeReader (for tests) ────────────────────────────────────────────────────

/// Injected [`CapReader`] for unit tests.
///
/// Reads from a pre-seeded map; never touches the filesystem or shells out.
pub struct FakeReader(Vec<(PathBuf, CapState)>);

impl FakeReader {
    /// Create a `FakeReader` pre-seeded with `(path, state)` pairs.
    #[must_use]
    pub const fn new(entries: Vec<(PathBuf, CapState)>) -> Self {
        Self(entries)
    }
}

impl CapReader for FakeReader {
    fn caps(&self, path: &Path) -> CapState {
        self.0
            .iter()
            .find(|(p, _)| p == path)
            .map_or_else(
                || CapState::Unreadable {
                    detail: format!("FakeReader: no entry for {}", path.display()),
                },
                |(_, s)| s.clone(),
            )
    }
}

// ── CapPlan ───────────────────────────────────────────────────────────────────

/// A single entry in the grant plan for one binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CapPlanEntry {
    /// The binary needs `cap_sys_admin+ep`; here is the exact `setcap` line to run.
    Grant {
        /// Binary path.
        path: PathBuf,
        /// Exact command the user must run (never executed by christen).
        command: String,
    },
    /// The binary already carries `cap_sys_admin+ep`; no action needed.
    AlreadyGranted {
        /// Binary path.
        path: PathBuf,
    },
    /// The binary cannot be granted due to a blocking condition.
    Blocked {
        /// Binary path.
        path: PathBuf,
        /// Human-readable reason (unreadable / setuid / missing binary).
        reason: String,
    },
}

/// The full declarative grant plan across all launcher binaries.
///
/// `cap_plan` is pure: it operates only on the injected `(path, CapState)`
/// pairs and never reads the filesystem or shells out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapPlan {
    /// Per-binary grant decisions.
    pub entries: Vec<CapPlanEntry>,
}

/// Build a [`CapPlan`] from pre-computed `(path, CapState)` pairs.
///
/// **Pure function** — no I/O.  Pass the output of a [`CapReader`] to populate
/// `binaries`.
#[must_use]
pub fn cap_plan(binaries: &[(PathBuf, CapState)]) -> CapPlan {
    let entries = binaries
        .iter()
        .map(|(path, state)| match state {
            CapState::Present => CapPlanEntry::AlreadyGranted { path: path.clone() },
            CapState::Absent => CapPlanEntry::Grant {
                path: path.clone(),
                command: format!("sudo setcap cap_sys_admin+ep {}", path.display()),
            },
            CapState::Unreadable { detail } => CapPlanEntry::Blocked {
                path: path.clone(),
                reason: format!("unreadable: {detail}"),
            },
            CapState::Setuid { warn } => CapPlanEntry::Blocked {
                path: path.clone(),
                reason: format!("setuid binary — resolve before granting caps: {warn}"),
            },
        })
        .collect();
    CapPlan { entries }
}

// ── Post-grant verification ───────────────────────────────────────────────────

/// Result of `christen cap --verify`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum VerifyResult {
    /// The launcher spawned successfully and the child has a nonzero `agent_session`.
    Live {
        /// The nonzero session id read from `/proc/$pid/agent_session`.
        session_id: String,
    },
    /// The launcher ran but `agent_session` is still all-zeros — cap not yet granted
    /// (or the unshare fell back due to `EPERM`).
    EpermFallback,
    /// The running kernel does not have `CONFIG_AGENT_NS=y` (no `/proc/*/agent_session`).
    Absent,
    /// The launcher binary could not be found or could not be spawned under sbx.
    Error {
        /// Error detail.
        detail: String,
    },
}

/// Run `--verify`: spawn `launcher` under `sbx`, read the child's `agent_session`.
///
/// This is the only function in this module that performs I/O (process spawn +
/// `/proc` read).  Deferred AC6: requires a `-wintermute` kernel + real launcher.
#[must_use]
pub fn verify_cap(launcher: &Path) -> VerifyResult {
    // Check that the -wintermute kernel is present (proxy: /proc/self/agent_session exists).
    if !Path::new("/proc/self/agent_session").exists() {
        return VerifyResult::Absent;
    }

    if !launcher.exists() {
        return VerifyResult::Error {
            detail: format!("launcher not found: {}", launcher.display()),
        };
    }

    // Spawn: sbx -- <launcher> /proc/self/agent_session
    // We ask the launcher to cat its own agent_session via sbx isolation.
    // If agent-wrap is the launcher it forks/execs; we read the file from the child.
    // Simpler: spawn `sbx -- <launcher> true` and read /proc/<pid>/agent_session.
    let sbx = which_sbx();
    let mut cmd = sbx.map_or_else(
        || Command::new(launcher),
        |sbx_path| {
            let mut c = Command::new(sbx_path);
            c.arg("--").arg(launcher);
            c
        },
    );
    // Ask the wrapper to exec `cat /proc/self/agent_session` so we capture its output.
    // Note: agent-wrap/agentns-claude take the final command as trailing args.
    cmd.arg("cat").arg("/proc/self/agent_session");

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            return VerifyResult::Error {
                detail: format!("spawn failed: {e}"),
            }
        }
    };

    let raw = String::from_utf8_lossy(&output.stdout);
    let session = raw.trim();

    // All-zeros UUID or empty → EPERM fallback.
    if session.is_empty() || session.bytes().all(|b| b == b'0' || b == b'-') {
        return VerifyResult::EpermFallback;
    }

    VerifyResult::Live {
        session_id: session.to_owned(),
    }
}

/// Find `sbx` on `$PATH`.
fn which_sbx() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p).find_map(|dir| {
            let candidate = dir.join("sbx");
            if candidate.exists() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}

// ── Default launcher paths ────────────────────────────────────────────────────

/// Return the default paths for `agentns-claude` and `agent-wrap` by searching `$PATH`.
///
/// Returns `(agentns_claude_path, agent_wrap_path)` — either may be `None` if not found.
#[must_use]
pub fn default_launcher_paths() -> (Option<PathBuf>, Option<PathBuf>) {
    let path_var = std::env::var_os("PATH");
    let mut agentns: Option<PathBuf> = None;
    let mut agent_wrap: Option<PathBuf> = None;

    if let Some(p) = path_var {
        for dir in std::env::split_paths(&p) {
            if agentns.is_none() {
                let c = dir.join("agentns-claude");
                if c.exists() {
                    agentns = Some(c);
                }
            }
            if agent_wrap.is_none() {
                let c = dir.join("agent-wrap");
                if c.exists() {
                    agent_wrap = Some(c);
                }
            }
            if agentns.is_some() && agent_wrap.is_some() {
                break;
            }
        }
    }

    (agentns, agent_wrap)
}
