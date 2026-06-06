//! AC6: christen plan --format json emits one entry per site plus the
//! RoutePlan tallies (to_wire/advised/already/skipped);
//! schema matches the documented RoutePlan.

use christen::{plan, ChristenConfig, FakeSource, KernelInfo, LaunchSiteSource, RawSite, RoutePlan, SiteKind};

fn make_sites() -> Vec<RawSite> {
    vec![
        RawSite {
            id: "claude-build.service".to_owned(),
            kind: SiteKind::SystemdUnit {
                unit: "claude-build.service".to_owned(),
                exec_start: "/usr/bin/build.sh".to_owned(),
            },
            exec_line: "/usr/bin/build.sh".to_owned(),
        },
        RawSite {
            id: "claude-dream.service".to_owned(),
            kind: SiteKind::SystemdUnit {
                unit: "claude-dream.service".to_owned(),
                exec_start: "agentns-claude --intent /dream -- /usr/bin/dream.sh".to_owned(),
            },
            exec_line: "agentns-claude --intent /dream -- /usr/bin/dream.sh".to_owned(),
        },
    ]
}

#[test]
fn test_plan_json_output_schema() {
    let config = ChristenConfig::default();
    let kernel = KernelInfo {
        agent_ns: true,
        release: "6.9.0-arch1-5-wintermute".to_owned(),
    };
    let source = FakeSource::new(make_sites());
    let raw = source.sites().expect("fake source");
    let rp = plan(&raw, &kernel, true, &config);

    // The plan has one action per site.
    assert_eq!(rp.actions.len(), 2, "should have one action per site");

    // Tallies must sum to the number of sites.
    assert_eq!(
        rp.to_wire + rp.advised + rp.already + rp.skipped,
        raw.len(),
        "tallies must sum to number of sites"
    );

    // Serialize to JSON and back.
    let json = serde_json::to_string_pretty(&rp).expect("serialize");
    let back: RoutePlan = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(rp, back, "JSON round-trip must be lossless");

    // JSON must contain the tally fields.
    assert!(json.contains("\"to_wire\""), "JSON must contain to_wire");
    assert!(json.contains("\"advised\""), "JSON must contain advised");
    assert!(json.contains("\"already\""), "JSON must contain already");
    assert!(json.contains("\"skipped\""), "JSON must contain skipped");
    assert!(json.contains("\"actions\""), "JSON must contain actions");
}
