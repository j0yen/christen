//! `christen` — launch-site model and route plan for agent-namespace wiring.
//!
//! This crate provides the shared types, trait, and pure logic needed to
//! classify where Claude sessions are born and what change would route each
//! launch site through `agentns-claude`. It intentionally does **not** read
//! the filesystem, `/proc`, or systemd — all discovery is delegated to
//! callers via the [`LaunchSiteSource`] trait.
//!
//! # Quick start
//!
//! ```rust
//! use christen::{plan, FakeSource, KernelInfo, LaunchSiteSource, RawSite, SiteKind};
//!
//! let sites = vec![
//!     RawSite {
//!         id: "claude-build.service".to_owned(),
//!         kind: SiteKind::SystemdUnit {
//!             unit: "claude-build.service".to_owned(),
//!             exec_start: "/usr/bin/claude-build-headless.sh".to_owned(),
//!         },
//!         exec_line: "/usr/bin/claude-build-headless.sh".to_owned(),
//!     },
//! ];
//! let source = FakeSource::new(sites);
//! let kernel = KernelInfo { agent_ns: true, release: "6.9.0-wintermute".to_owned() };
//! let raw = source.sites().expect("fake source never fails");
//! let route_plan = plan(&raw, &kernel, true, &Default::default());
//! assert_eq!(route_plan.to_wire, 1);
//! ```

pub mod cap;
pub mod config;
pub mod ledger;
pub mod model;
pub mod planner;
pub mod probe;
pub mod route;
pub mod source;

pub use cap::{
    cap_plan, default_launcher_paths, verify_cap, CapPlan, CapPlanEntry, CapReader, CapState,
    FakeReader, GetcapReader, VerifyResult, SCOPE_EXPLAINER,
};
pub use config::ChristenConfig;
pub use model::{
    KernelInfo, LaunchSite, RawSite, RouteAction, RoutePlan, SiteKind, WrapState,
};
pub use planner::{intent_for, plan};
pub use probe::{
    apply_docket_op, classify, verdict, Counters as ProbeCounters, DocketOp, FakeNsReader,
    InitReason, NsReading, NsState, ProbeOutput, ProcReader, RealProcReader, Verdict,
    INIT_AGENT_INODE,
};
pub use route::{apply_route, render_dropin, DropIn, RouteError, SystemdSource};
pub use source::{FakeSource, LaunchSiteSource};

pub use ledger::{
    cmd_close, cmd_install, cmd_open, delta, summarize, CloseInfo, CounterReader, Counters,
    EntrySummary, FakeCounterReader, FakeStore, FsStore, LedgerEntry, LedgerError, LedgerStore,
    RealCounterReader,
};
