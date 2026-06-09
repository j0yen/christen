//! Probe module — reads the `/proc` agent-namespace surface and classifies it.
//!
//! The classifier `classify` is pure: it accepts an [`NsReading`] and returns
//! an [`NsState`]. No filesystem access, no shell calls, no side effects. The
//! [`ProcReader`] trait abstracts the impure `/proc` reads so that tests can
//! inject a [`FakeNsReader`].
//!
//! ## Anti-regression invariant
//!
//! The literal string `"registration failed"` **must never** appear in any
//! `prose` returned by [`verdict`]. This invariant is asserted by a test over
//! every [`NsState`] variant.
//!
//! ## Init-inode detection
//!
//! On Linux the agent-namespace init inode is typically `4026531996` (the
//! constant used when no wrapper has been applied). Processes born in the init
//! namespace will have `/proc/self/ns/agent -> agent:[4026531996]`. This is
//! expected — not a fault — when the wrapper binary is installed but launch
//! sites have not yet been routed through `agentns-claude`.
//!
//! ## Docket edge-trigger
//!
//! When `--emit` is requested, `probe` shells `docket` with the mapped
//! [`DocketOp`]. A missing `docket` binary is non-fatal.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// The init-namespace agent inode. Processes born outside any agent namespace
/// see this inode in `/proc/self/ns/agent`. This is expected — not a fault —
/// until launch sites are routed through the wrapper.
pub const INIT_AGENT_INODE: u64 = 4_026_531_996;

/// Raw counters from `/proc/$pid/agent_counters`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters {
    /// Total events recorded in this namespace.
    pub events: u64,
    /// Active session count.
    pub sessions: u64,
}

/// The raw `/proc` observation injected into [`classify`].
///
/// All fields are `Option` because the surface may be absent (stock kernel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NsReading {
    /// The agent namespace inode from `/proc/$pid/ns/agent`, if present.
    pub ns_inode: Option<u64>,
    /// The raw hex session id from `/proc/$pid/agent_session`, if present.
    pub session_hex: Option<String>,
    /// The counters from `/proc/$pid/agent_counters`, if present.
    pub counters: Option<Counters>,
    /// Whether the running kernel has `-wintermute` in its release string.
    pub kernel_is_wintermute: bool,
    /// Whether an agent-namespace wrapper binary is installed on `$PATH`.
    pub wrapper_installed: bool,
}

/// The reason a session is in the init namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitReason {
    /// Wrapper is installed and the kernel supports agent namespaces, but
    /// launch sites have not been routed through `agentns-claude` yet.
    /// Run `christen route` to fix this. Not a fault.
    UnwrappedExpected,
    /// Wrapper is not installed; the init namespace is fully expected.
    NoWrapperInstalled,
}

/// The classified state of the agent namespace for a given process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum NsState {
    /// Agent-namespace surface is absent (files missing in `/proc`).
    ///
    /// On a `-wintermute` kernel this is a fault (driver missing/not booted).
    /// On a stock kernel this is fully expected.
    Absent,
    /// The session id is all-zeros or the namespace inode is the init inode.
    ///
    /// Not a fault. Means the session was born in the init namespace — the
    /// wrapper is installed but launch sites are not yet routed.
    Init {
        /// Why this process is in the init namespace.
        reason: InitReason,
    },
    /// The session has a live, non-zero id — the namespace is healthy.
    Live {
        /// The hex session id.
        session_hex: String,
        /// Optional intent tag derived from the session (future use).
        intent: Option<String>,
    },
    /// The agent-namespace surface is present but cannot be parsed.
    Malformed {
        /// Human-readable detail of the parse failure.
        detail: String,
    },
}

/// The docket side-effect to apply when `--emit` is requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DocketOp {
    /// File a new finding (or update an existing one) via `docket report`.
    Report {
        /// Finding severity (e.g. `"warn"`, `"crit"`).
        severity: String,
        /// Finding title.
        title: String,
        /// Free-form evidence string.
        evidence: String,
    },
    /// Resolve an open finding via `docket resolve`.
    Resolve {
        /// The finding id to resolve.
        id: String,
    },
    /// No docket action required for this state.
    None,
}

/// The verdict for a classified namespace state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// Whether this state is considered healthy (no action required).
    pub ok: bool,
    /// Human-readable description (never contains "registration failed").
    pub prose: String,
    /// The docket operation to apply if `--emit` is requested.
    pub docket: DocketOp,
}

/// Classify a raw `/proc` observation into an [`NsState`].
///
/// This function is **pure**: it only inspects `reading` and never accesses
/// the filesystem, shells any command, or produces side effects.
#[must_use]
pub fn classify(reading: &NsReading) -> NsState {
    // Surface absent: no ns/agent symlink.
    let Some(inode) = reading.ns_inode else {
        return NsState::Absent;
    };

    // Surface present but session_hex missing → malformed.
    let Some(ref hex) = reading.session_hex else {
        return NsState::Malformed {
            detail: "ns/agent symlink present but agent_session missing".to_owned(),
        };
    };

    // Validate hex string (must be a 32-char all-hex string or "0"-filled equivalent).
    let trimmed = hex.trim();
    if !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return NsState::Malformed {
            detail: format!("agent_session contains non-hex characters: {trimmed:?}"),
        };
    }

    // All-zeros check (either the inode is the init inode, or the session id is all zeros).
    let all_zero = trimmed.chars().all(|c| c == '0') || trimmed.is_empty();
    if all_zero || inode == INIT_AGENT_INODE {
        let reason = if reading.wrapper_installed {
            InitReason::UnwrappedExpected
        } else {
            InitReason::NoWrapperInstalled
        };
        return NsState::Init { reason };
    }

    // Non-zero session — live namespace.
    NsState::Live {
        session_hex: trimmed.to_owned(),
        intent: None,
    }
}

/// Compute the [`Verdict`] for a classified state, given kernel context.
///
/// The `kernel_is_wintermute` flag affects whether `Absent` and non-expected
/// states are treated as faults.
#[must_use]
pub fn verdict(state: &NsState, kernel_is_wintermute: bool) -> Verdict {
    match state {
        NsState::Absent => {
            if kernel_is_wintermute {
                Verdict {
                    ok: false,
                    prose: "Agent-namespace surface is absent. On this -wintermute kernel the \
                            agentns driver should be present. Check that the module is loaded \
                            and the kernel was booted correctly."
                        .to_owned(),
                    docket: DocketOp::Report {
                        severity: "warn".to_owned(),
                        title: "agentns surface absent on wintermute kernel".to_owned(),
                        evidence: "proc/self/ns/agent missing; driver may not be loaded".to_owned(),
                    },
                }
            } else {
                Verdict {
                    ok: true,
                    prose: "Agent-namespace surface is absent. This is expected on a stock kernel \
                            without CONFIG_AGENT_NS. No action needed."
                        .to_owned(),
                    docket: DocketOp::None,
                }
            }
        }
        NsState::Init { reason } => match reason {
            InitReason::UnwrappedExpected => {
                if kernel_is_wintermute {
                    Verdict {
                        ok: true,
                        prose: "Agent namespace is present but this session is in the init \
                                namespace (all-zeros session id). The wrapper binary is installed \
                                but launch sites have not been routed through agentns-claude yet. \
                                Run `christen route` to wire them up. This is expected until \
                                routing is applied."
                            .to_owned(),
                        docket: DocketOp::Report {
                            severity: "warn".to_owned(),
                            title: "agentns init NS — launches not routed through agentns-claude; \
                                    run `christen route`"
                                .to_owned(),
                            evidence: "session id all-zeros; ns inode is init inode; \
                                       wrapper installed but exec lines not yet rewritten"
                                .to_owned(),
                        },
                    }
                } else {
                    Verdict {
                        ok: true,
                        prose: "Agent namespace is in the init namespace. On a stock kernel this \
                                is expected. No action needed."
                            .to_owned(),
                        docket: DocketOp::None,
                    }
                }
            }
            InitReason::NoWrapperInstalled => Verdict {
                ok: true,
                prose: "Agent namespace is in the init namespace. The wrapper binary is not \
                        installed, so all sessions run in the init namespace by design."
                    .to_owned(),
                docket: DocketOp::None,
            },
        },
        NsState::Live { session_hex, .. } => Verdict {
            ok: true,
            prose: format!(
                "Agent namespace is live with session id {session_hex}. The session is correctly \
                 isolated in a non-init agent namespace."
            ),
            docket: DocketOp::Resolve {
                id: "agentns-session-zeros".to_owned(),
            },
        },
        NsState::Malformed { detail } => Verdict {
            ok: false,
            prose: format!(
                "Agent-namespace surface is present but malformed: {detail}. \
                 Check the kernel driver or /proc entries."
            ),
            docket: DocketOp::Report {
                severity: "warn".to_owned(),
                title: "agentns surface malformed".to_owned(),
                evidence: detail.clone(),
            },
        },
    }
}

/// Trait for reading the `/proc` agent-namespace surface.
///
/// The real implementation reads `/proc/$pid/{ns/agent,agent_session,agent_counters}`
/// and inspects `uname`. The [`FakeNsReader`] is used in tests.
pub trait ProcReader {
    /// Read the namespace surface for `pid` (or `None` for the current process).
    ///
    /// # Errors
    ///
    /// Returns an error if a critical I/O failure occurs (as opposed to
    /// the surface being simply absent).
    fn read(&self, pid: Option<u32>) -> Result<NsReading, Box<dyn std::error::Error>>;
}

/// Real implementation of [`ProcReader`] that reads `/proc`.
pub struct RealProcReader;

impl ProcReader for RealProcReader {
    fn read(&self, pid: Option<u32>) -> Result<NsReading, Box<dyn std::error::Error>> {
        let pid_str = match pid {
            Some(p) => p.to_string(),
            None => "self".to_owned(),
        };

        let ns_link = PathBuf::from(format!("/proc/{pid_str}/ns/agent"));
        let session_path = PathBuf::from(format!("/proc/{pid_str}/agent_session"));
        let counters_path = PathBuf::from(format!("/proc/{pid_str}/agent_counters"));

        // Detect init inode from symlink target.
        let ns_inode = read_ns_inode(&ns_link);

        // Read session hex (may be absent on stock kernel).
        let session_hex = if session_path.exists() {
            let raw = std::fs::read_to_string(&session_path)?;
            Some(raw.trim().to_owned())
        } else {
            None
        };

        // Read counters (optional; ignore parse errors gracefully).
        let counters = if counters_path.exists() {
            std::fs::read_to_string(&counters_path)
                .ok()
                .and_then(|s| parse_counters(&s))
        } else {
            None
        };

        // Detect wintermute kernel.
        let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_default();
        let kernel_is_wintermute = release.trim().contains("-wintermute");

        // Check wrapper presence.
        let wrapper_installed = is_wrapper_installed();

        Ok(NsReading {
            ns_inode,
            session_hex,
            counters,
            kernel_is_wintermute,
            wrapper_installed,
        })
    }
}

/// Parse the agent namespace inode from the symlink target
/// (e.g. `agent:[4026531996]` → `4026531996`).
fn read_ns_inode(link: &Path) -> Option<u64> {
    let target = std::fs::read_link(link).ok()?;
    let s = target.to_string_lossy();
    // Format: `agent:[<inode>]`
    let inner = s.strip_prefix("agent:[")?.strip_suffix(']')?;
    inner.parse().ok()
}

/// Parse counters from `/proc/$pid/agent_counters` content.
fn parse_counters(s: &str) -> Option<Counters> {
    let mut events = None;
    let mut sessions = None;
    for line in s.lines() {
        if let Some(v) = line.strip_prefix("events:") {
            events = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("sessions:") {
            sessions = v.trim().parse().ok();
        }
    }
    Some(Counters {
        events: events?,
        sessions: sessions?,
    })
}

/// Returns `true` if any agent-namespace wrapper binary is on `$PATH`.
fn is_wrapper_installed() -> bool {
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|dir| {
            dir.join("agentns-claude").exists() || dir.join("agent-wrap").exists()
        })
    })
}

/// A fake [`ProcReader`] for tests; returns a pre-built [`NsReading`].
pub struct FakeNsReader {
    reading: NsReading,
}

impl FakeNsReader {
    /// Create a new `FakeNsReader` that will return `reading`.
    #[must_use]
    pub fn new(reading: NsReading) -> Self {
        Self { reading }
    }
}

impl ProcReader for FakeNsReader {
    fn read(&self, _pid: Option<u32>) -> Result<NsReading, Box<dyn std::error::Error>> {
        Ok(self.reading.clone())
    }
}

/// The JSON output schema for `christen probe --format json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeOutput {
    /// The classified namespace state.
    pub state: NsState,
    /// Whether the state is considered healthy.
    pub ok: bool,
    /// Human-readable prose (never "registration failed").
    pub prose: String,
    /// The hex session id, if the namespace is live.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_hex: Option<String>,
    /// The intent tag, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The docket operation that would be applied with `--emit`.
    pub docket_op: DocketOp,
}

impl ProbeOutput {
    /// Build a `ProbeOutput` from state + verdict.
    #[must_use]
    pub fn from_state_verdict(state: &NsState, v: &Verdict) -> Self {
        let (session_hex, intent) = match state {
            NsState::Live {
                session_hex,
                intent,
            } => (Some(session_hex.clone()), intent.clone()),
            _ => (None, None),
        };
        Self {
            state: state.clone(),
            ok: v.ok,
            prose: v.prose.clone(),
            session_hex,
            intent,
            docket_op: v.docket.clone(),
        }
    }
}

/// Apply a [`DocketOp`] by shelling `docket`. Non-fatal — errors are printed
/// to stderr and the caller continues normally.
///
/// # Errors
///
/// Always returns `Ok(())`. Errors shelling `docket` are printed to stderr
/// and swallowed so the caller can proceed.
pub fn apply_docket_op(op: &DocketOp) -> Result<(), Box<dyn std::error::Error>> {
    match op {
        DocketOp::None => {}
        DocketOp::Resolve { id } => {
            let status = Command::new("docket").args(["resolve", id]).status();
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => eprintln!("christen probe: docket resolve exited {s}"),
                Err(e) => eprintln!("christen probe: docket not available ({e}); continuing"),
            }
        }
        DocketOp::Report {
            severity,
            title,
            evidence,
        } => {
            let status = Command::new("docket")
                .args(["report", "--severity", severity, "--title", title, "--evidence", evidence])
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => eprintln!("christen probe: docket report exited {s}"),
                Err(e) => eprintln!("christen probe: docket not available ({e}); continuing"),
            }
        }
    }
    Ok(())
}
