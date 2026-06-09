//! AC1: cap_plan is pure — operates only on injected (path, CapState) pairs,
//! asserted to read nothing from the filesystem.

use std::path::PathBuf;

use christen::{cap_plan, CapPlanEntry, CapState};

#[test]
fn cap_plan_pure_no_io() {
    // Using completely synthetic paths — no filesystem access is performed.
    let path_a = PathBuf::from("/nonexistent/agentns-claude");
    let path_b = PathBuf::from("/nonexistent/agent-wrap");

    let binaries = vec![
        (path_a.clone(), CapState::Absent),
        (path_b.clone(), CapState::Present),
    ];

    let plan = cap_plan(&binaries);
    assert_eq!(plan.entries.len(), 2);

    // First entry: Absent → Grant.
    match &plan.entries[0] {
        CapPlanEntry::Grant { path, command } => {
            assert_eq!(path, &path_a);
            assert_eq!(
                command,
                "sudo setcap cap_sys_admin+ep /nonexistent/agentns-claude"
            );
        }
        other => panic!("expected Grant, got {other:?}"),
    }

    // Second entry: Present → AlreadyGranted.
    match &plan.entries[1] {
        CapPlanEntry::AlreadyGranted { path } => {
            assert_eq!(path, &path_b);
        }
        other => panic!("expected AlreadyGranted, got {other:?}"),
    }
}
