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

pub mod config;
pub mod model;
pub mod planner;
pub mod source;

pub use config::ChristenConfig;
pub use model::{
    KernelInfo, LaunchSite, RawSite, RouteAction, RoutePlan, SiteKind, WrapState,
};
pub use planner::{intent_for, plan};
pub use source::{FakeSource, LaunchSiteSource};
