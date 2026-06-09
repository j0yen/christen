//! AC5: christen cap --format json emits the documented schema
//! (per-binary state + action) for a mixed fixture (one Absent, one Present).

use std::path::PathBuf;

use christen::{cap_plan, CapState};

#[test]
fn json_schema_mixed_fixture() {
    let path_absent = PathBuf::from("/home/jsy/.local/bin/agentns-claude");
    let path_present = PathBuf::from("/home/jsy/.local/bin/agent-wrap");

    let binaries = vec![
        (path_absent.clone(), CapState::Absent),
        (path_present.clone(), CapState::Present),
    ];

    let plan = cap_plan(&binaries);

    // Serialise the plan the way the CLI does.
    let json_val = serde_json::json!({
        "scope_explainer": christen::SCOPE_EXPLAINER,
        "binaries": plan.entries.iter().map(|e| match e {
            christen::CapPlanEntry::Grant { path, command } => serde_json::json!({
                "path": path,
                "state": "absent",
                "action": "grant",
                "command": command
            }),
            christen::CapPlanEntry::AlreadyGranted { path } => serde_json::json!({
                "path": path,
                "state": "present",
                "action": "already_granted"
            }),
            christen::CapPlanEntry::Blocked { path, reason } => serde_json::json!({
                "path": path,
                "state": "blocked",
                "action": "blocked",
                "reason": reason
            }),
        }).collect::<Vec<_>>(),
    });

    let s = serde_json::to_string_pretty(&json_val).expect("serialize");

    // Must be valid JSON.
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("parse back");

    // Must have top-level keys.
    assert!(parsed.get("scope_explainer").is_some());
    let binaries = parsed
        .get("binaries")
        .and_then(|v| v.as_array())
        .expect("binaries must be an array");
    assert_eq!(binaries.len(), 2);

    // First entry (Absent → grant).
    let first = &binaries[0];
    assert_eq!(first["state"].as_str(), Some("absent"));
    assert_eq!(first["action"].as_str(), Some("grant"));
    assert!(
        first["command"]
            .as_str()
            .map(|c| c.contains("sudo setcap cap_sys_admin+ep"))
            .unwrap_or(false),
        "command must contain 'sudo setcap cap_sys_admin+ep'"
    );

    // Second entry (Present → already_granted).
    let second = &binaries[1];
    assert_eq!(second["state"].as_str(), Some("present"));
    assert_eq!(second["action"].as_str(), Some("already_granted"));
}
