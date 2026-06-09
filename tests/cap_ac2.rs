//! AC2: cap_plan maps all CapState variants to the correct CapPlanEntry;
//! the generated setcap line is byte-exact.

use std::path::PathBuf;

use christen::{cap_plan, CapPlanEntry, CapState};

#[test]
fn absent_yields_grant_with_exact_setcap_line() {
    let path = PathBuf::from("/home/jsy/.local/bin/agentns-claude");
    let plan = cap_plan(&[(path.clone(), CapState::Absent)]);

    match &plan.entries[0] {
        CapPlanEntry::Grant { path: p, command } => {
            assert_eq!(p, &path);
            assert_eq!(
                command,
                "sudo setcap cap_sys_admin+ep /home/jsy/.local/bin/agentns-claude",
                "setcap line must be byte-exact"
            );
        }
        other => panic!("expected Grant, got {other:?}"),
    }
}

#[test]
fn present_yields_already_granted() {
    let path = PathBuf::from("/home/jsy/.local/bin/agentns-claude");
    let plan = cap_plan(&[(path.clone(), CapState::Present)]);

    match &plan.entries[0] {
        CapPlanEntry::AlreadyGranted { path: p } => assert_eq!(p, &path),
        other => panic!("expected AlreadyGranted, got {other:?}"),
    }
}

#[test]
fn unreadable_yields_blocked() {
    let path = PathBuf::from("/home/jsy/.local/bin/agentns-claude");
    let plan = cap_plan(&[(
        path.clone(),
        CapState::Unreadable {
            detail: "getcap not available".to_owned(),
        },
    )]);

    match &plan.entries[0] {
        CapPlanEntry::Blocked { path: p, reason } => {
            assert_eq!(p, &path);
            assert!(
                reason.contains("unreadable"),
                "reason must mention 'unreadable': {reason}"
            );
        }
        other => panic!("expected Blocked, got {other:?}"),
    }
}

#[test]
fn setuid_yields_blocked() {
    let path = PathBuf::from("/home/jsy/.local/bin/agent-wrap");
    let plan = cap_plan(&[(
        path.clone(),
        CapState::Setuid {
            warn: "setuid bit detected".to_owned(),
        },
    )]);

    match &plan.entries[0] {
        CapPlanEntry::Blocked { path: p, reason } => {
            assert_eq!(p, &path);
            assert!(
                reason.contains("setuid"),
                "reason must mention 'setuid': {reason}"
            );
        }
        other => panic!("expected Blocked, got {other:?}"),
    }
}

#[test]
fn setcap_line_format_agent_wrap() {
    let path = PathBuf::from("/home/jsy/.local/bin/agent-wrap");
    let plan = cap_plan(&[(path, CapState::Absent)]);

    match &plan.entries[0] {
        CapPlanEntry::Grant { command, .. } => {
            assert_eq!(
                command,
                "sudo setcap cap_sys_admin+ep /home/jsy/.local/bin/agent-wrap"
            );
        }
        other => panic!("expected Grant, got {other:?}"),
    }
}
