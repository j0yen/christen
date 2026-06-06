//! Core types for the christen launch-site model.
//!
//! All types are `serde`-(de)serializable and `Clone`+`Debug`+`PartialEq`.
//! No filesystem or `/proc` access is performed here.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The kind of launch site — how sessions are spawned at this location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SiteKind {
    /// A systemd user unit (e.g. `~/.config/systemd/user/claude-build.service`).
    SystemdUnit {
        /// The unit filename, e.g. `claude-build.service`.
        unit: String,
        /// The raw `ExecStart=` line from the unit file.
        exec_start: String,
    },
    /// A shell RC file that sources or aliases `claude`.
    ShellRc {
        /// Path to the RC file.
        path: PathBuf,
    },
    /// A Claude Code hook (`SessionStart`, etc.).
    Hook,
    /// Any other discovered launch path.
    Other {
        /// Human-readable note.
        note: String,
    },
}

/// Whether a launch site already routes through the agent-namespace wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WrapState {
    /// The site's exec line does not invoke `agentns-claude` or `agent-wrap`.
    Unwrapped,
    /// The site's exec line already invokes the wrapper.
    Wrapped {
        /// The wrapper binary detected in the exec line.
        via: String,
    },
    /// Wrap state cannot be determined statically (e.g. a shell alias).
    Uncertain,
}

/// A discovered launch site with its classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchSite {
    /// Stable identifier for this site (e.g. `claude-build.service`).
    pub id: String,
    /// How sessions are spawned here.
    pub kind: SiteKind,
    /// Whether this site is already wrapped.
    pub wrap: WrapState,
    /// Derived intent tag (e.g. `/build`, `/dream`, `interactive`).
    pub intent: String,
}

/// A declarative change action — no edits are performed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RouteAction {
    /// Rewrite the exec line to route through `agentns-claude`.
    Wire {
        /// Site id.
        site: String,
        /// Original exec line.
        from: String,
        /// Rewritten exec line including `agentns-claude --intent <intent> --budget <budget> --`.
        to: String,
    },
    /// For shell/user sites: print a snippet they can apply manually.
    Advise {
        /// Site id.
        site: String,
        /// A shell snippet (e.g. an alias) to advise.
        snippet: String,
    },
    /// The site already routes through the wrapper; no change needed.
    AlreadyWrapped {
        /// Site id.
        site: String,
    },
    /// The site is skipped (e.g. no agent-ns kernel or wrapper not installed).
    Skip {
        /// Site id.
        site: String,
        /// Explanation of why the site is skipped.
        reason: String,
    },
}

/// A declarative plan summarising what needs to happen across all sites.
///
/// No side effects are performed when creating a `RoutePlan`; it is a pure
/// description of the diff between the current state and the desired state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePlan {
    /// The ordered list of actions for each discovered site.
    pub actions: Vec<RouteAction>,
    /// Number of sites that need to be wired.
    pub to_wire: usize,
    /// Number of sites that can only be advised (shell/user sites).
    pub advised: usize,
    /// Number of sites already wrapped.
    pub already: usize,
    /// Number of sites skipped (no kernel support or wrapper absent).
    pub skipped: usize,
}

/// Information about the running kernel, injected by the caller.
///
/// `plan()` is pure; it never reads `/proc/version` or `/proc/sys/kernel/osrelease` itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelInfo {
    /// Whether the kernel was built with `CONFIG_AGENT_NS=y`.
    pub agent_ns: bool,
    /// The full kernel release string (e.g. `6.9.0-arch1-5-wintermute`).
    pub release: String,
}

/// Raw site data as discovered by a [`crate::LaunchSiteSource`].
///
/// `plan()` turns `&[RawSite]` into typed [`LaunchSite`]s and a [`RoutePlan`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSite {
    /// Stable identifier for this site.
    pub id: String,
    /// The kind of launch site (may be a rough classification from the source).
    pub kind: SiteKind,
    /// The raw exec/command line to inspect for wrapper presence.
    pub exec_line: String,
}
