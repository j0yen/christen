//! AC5: intent_for derives /build//dream//self-review/interactive for the
//! four canonical site ids, and intent_overrides from config wins over the
//! derivation; both covered by tests.

use christen::{plan, ChristenConfig, FakeSource, KernelInfo, LaunchSiteSource, RawSite, RouteAction, SiteKind};
use christen::planner::intent_for;

#[test]
fn test_intent_for_derivation() {
    // Built-in derivation table.
    assert_eq!(intent_for("claude-build.service"), "/build");
    assert_eq!(intent_for("claude-dream.service"), "/dream");
    assert_eq!(intent_for("claude-self-review.service"), "/self-review");
    assert_eq!(intent_for("interactive"), "interactive");
    assert_eq!(intent_for("unknown-unit.service"), "unknown");
}

#[test]
fn test_intent_overrides_win_over_derivation() {
    let mut config = ChristenConfig::default();
    config
        .intent_overrides
        .insert("claude-build.service".to_owned(), "/custom-build".to_owned());

    let kernel = KernelInfo {
        agent_ns: true,
        release: "6.9.0-arch1-5-wintermute".to_owned(),
    };
    let sites = vec![RawSite {
        id: "claude-build.service".to_owned(),
        kind: SiteKind::SystemdUnit {
            unit: "claude-build.service".to_owned(),
            exec_start: "/usr/bin/build.sh".to_owned(),
        },
        exec_line: "/usr/bin/build.sh".to_owned(),
    }];
    let source = FakeSource::new(sites);
    let raw = source.sites().expect("fake source");
    let rp = plan(&raw, &kernel, true, &config);

    // The Wire action's `to` field must use /custom-build (from override), not /build.
    match &rp.actions[0] {
        RouteAction::Wire { to, .. } => {
            assert!(
                to.contains("--intent /custom-build"),
                "override must win over built-in derivation, got: {to}"
            );
        }
        other => panic!("expected Wire, got {:?}", other),
    }
}
