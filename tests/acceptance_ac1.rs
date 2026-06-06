//! AC1: cargo build and cargo test succeed offline; a test asserts plan makes
//! zero source calls (it operates only on the passed &[RawSite] + injected
//! KernelInfo + wrapper_installed).

use christen::{plan, ChristenConfig, KernelInfo, RawSite, SiteKind};

/// plan() operates purely on its arguments; it never calls any source.
/// We verify this by passing a fixed slice of RawSites and asserting
/// the resulting RoutePlan reflects only those inputs.
#[test]
fn test_plan_no_source_calls() {
    let sites: Vec<RawSite> = Vec::new();
    let kernel = KernelInfo {
        agent_ns: true,
        release: "6.9.0-arch1-5-wintermute".to_owned(),
    };
    let config = ChristenConfig::default();

    // An empty input must produce an empty plan — no magic population.
    let route_plan = plan(&sites, &kernel, true, &config);
    assert_eq!(
        route_plan.actions.len(),
        0,
        "plan with no input sites must produce no actions"
    );
    assert_eq!(route_plan.to_wire, 0);
    assert_eq!(route_plan.advised, 0);
    assert_eq!(route_plan.already, 0);
    assert_eq!(route_plan.skipped, 0);

    // A single input site must produce exactly one action.
    let sites_one = vec![RawSite {
        id: "claude-build.service".to_owned(),
        kind: SiteKind::SystemdUnit {
            unit: "claude-build.service".to_owned(),
            exec_start: "/usr/bin/claude-build-headless.sh".to_owned(),
        },
        exec_line: "/usr/bin/claude-build-headless.sh".to_owned(),
    }];
    let plan_one = plan(&sites_one, &kernel, true, &config);
    assert_eq!(
        plan_one.actions.len(),
        1,
        "plan with one input site must produce exactly one action"
    );
}
