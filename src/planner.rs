//! Pure planning logic: `&[RawSite]` + [`KernelInfo`] + `wrapper_installed` → [`RoutePlan`].
//!
//! This module intentionally has **no** I/O, no filesystem access, and no
//! calls to any [`crate::LaunchSiteSource`]. All inputs are passed as plain
//! Rust values so the logic is trivially testable.

use crate::config::ChristenConfig;
use crate::model::{KernelInfo, LaunchSite, RawSite, RouteAction, RoutePlan, SiteKind, WrapState};

/// Marker strings that indicate a site's exec line already uses a wrapper.
const WRAPPER_MARKERS: &[&str] = &["agentns-claude", "agent-wrap"];

/// Returns `true` if `exec_line` already invokes a known wrapper binary.
fn detect_wrap(exec_line: &str) -> Option<String> {
    for marker in WRAPPER_MARKERS {
        if exec_line.contains(marker) {
            return Some((*marker).to_owned());
        }
    }
    None
}

/// Returns `true` if the kernel release string indicates a wintermute kernel.
///
/// A wintermute kernel is identified by the `-wintermute` suffix in the
/// release string (e.g. `6.9.0-arch1-5-wintermute`).
fn is_wintermute_kernel(release: &str) -> bool {
    release.contains("-wintermute")
}

/// Derives the intent tag for a site id from the built-in derivation table.
///
/// The table covers the four canonical site ids:
/// - `"claude-build.service"` → `"/build"`
/// - `"claude-dream.service"` → `"/dream"`
/// - `"claude-self-review.service"` → `"/self-review"`
/// - `"interactive"` → `"interactive"`
///
/// Any unknown site id falls back to `"unknown"`.
#[must_use]
pub fn intent_for(site_id: &str) -> &'static str {
    match site_id {
        "claude-build.service" => "/build",
        "claude-dream.service" => "/dream",
        "claude-self-review.service" => "/self-review",
        "interactive" => "interactive",
        _ => "unknown",
    }
}

/// Classifies a single raw site into a [`LaunchSite`].
fn classify_site(raw: &RawSite, config: &ChristenConfig) -> LaunchSite {
    let wrap = match detect_wrap(&raw.exec_line) {
        Some(via) => WrapState::Wrapped { via },
        None => match &raw.kind {
            SiteKind::ShellRc { .. } | SiteKind::Hook => WrapState::Uncertain,
            SiteKind::SystemdUnit { .. } | SiteKind::Other { .. } => WrapState::Unwrapped,
        },
    };

    // Prefer config override, then builtin table.
    let intent = config
        .intent_overrides
        .get(&raw.id)
        .cloned()
        .unwrap_or_else(|| intent_for(&raw.id).to_owned());

    LaunchSite {
        id: raw.id.clone(),
        kind: raw.kind.clone(),
        wrap,
        intent,
    }
}

/// Builds the [`RoutePlan`] for a slice of raw sites.
///
/// # Parameters
/// - `raw`: Sites as discovered by a [`crate::LaunchSiteSource`].
/// - `kernel`: Injected kernel information (not read from `/proc` here).
/// - `wrapper_installed`: Whether `agentns-claude` / `agent-wrap` are reachable.
/// - `config`: Loaded [`ChristenConfig`] (may be default).
///
/// # Purity guarantee
/// This function makes **zero** calls to any source, filesystem, or system
/// API. It operates exclusively on its arguments. AC1 tests this invariant.
#[must_use]
pub fn plan(
    raw: &[RawSite],
    kernel: &KernelInfo,
    wrapper_installed: bool,
    config: &ChristenConfig,
) -> RoutePlan {
    let winter = is_wintermute_kernel(&kernel.release) && kernel.agent_ns;

    let mut actions: Vec<RouteAction> = Vec::with_capacity(raw.len());
    let mut to_wire = 0usize;
    let mut advised = 0usize;
    let mut already = 0usize;
    let mut skipped = 0usize;

    for site_raw in raw {
        let site = classify_site(site_raw, config);

        let action = match &site.wrap {
            WrapState::Wrapped { .. } => {
                already += 1;
                RouteAction::AlreadyWrapped {
                    site: site.id.clone(),
                }
            }
            WrapState::Unwrapped => {
                if !winter {
                    skipped += 1;
                    let reason = if !kernel.agent_ns {
                        "kernel lacks CONFIG_AGENT_NS support".to_owned()
                    } else {
                        "not a -wintermute kernel".to_owned()
                    };
                    RouteAction::Skip {
                        site: site.id.clone(),
                        reason,
                    }
                } else if !wrapper_installed {
                    skipped += 1;
                    RouteAction::Skip {
                        site: site.id.clone(),
                        reason: "agentns-claude wrapper is not installed".to_owned(),
                    }
                } else {
                    to_wire += 1;
                    let from = site_raw.exec_line.clone();
                    let to = format!(
                        "agentns-claude --intent {} --budget {} -- {}",
                        site.intent, config.default_budget, from
                    );
                    RouteAction::Wire {
                        site: site.id.clone(),
                        from,
                        to,
                    }
                }
            }
            WrapState::Uncertain => {
                // Shell RC / Hook sites: we can only advise.
                if !winter || !wrapper_installed {
                    skipped += 1;
                    let reason = if !winter {
                        "not a -wintermute kernel or no agent-ns support".to_owned()
                    } else {
                        "agentns-claude wrapper is not installed".to_owned()
                    };
                    RouteAction::Skip {
                        site: site.id.clone(),
                        reason,
                    }
                } else {
                    advised += 1;
                    let snippet = format!(
                        "alias claude='agentns-claude --intent {} --budget {} -- claude'",
                        site.intent, config.default_budget
                    );
                    RouteAction::Advise {
                        site: site.id.clone(),
                        snippet,
                    }
                }
            }
        };
        actions.push(action);
    }

    RoutePlan {
        actions,
        to_wire,
        advised,
        already,
        skipped,
    }
}
