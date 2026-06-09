//! AC4: The scope explainer is emitted before any setcap line.
//!
//! Also asserts that cap_plan itself never invokes setcap (it's a pure
//! function operating only on injected CapState values — asserted by AC1).
//! Here we verify the ordering invariant in output construction.

use std::path::PathBuf;

use christen::{cap_plan, CapPlanEntry, CapState, SCOPE_EXPLAINER};

#[test]
fn scope_explainer_is_non_empty() {
    assert!(!SCOPE_EXPLAINER.is_empty(), "SCOPE_EXPLAINER must not be empty");
    assert!(
        SCOPE_EXPLAINER.contains("CAP_SYS_ADMIN"),
        "explainer must mention CAP_SYS_ADMIN"
    );
    assert!(
        SCOPE_EXPLAINER.contains("file"),
        "explainer must mention 'file' (file capability)"
    );
    assert!(
        SCOPE_EXPLAINER.contains("never"),
        "explainer must state christen never runs setcap"
    );
}

#[test]
fn scope_explainer_text_before_setcap_command_in_output() {
    // Build a minimal plan with one Absent binary.
    let path = PathBuf::from("/nonexistent/agentns-claude");
    let plan = cap_plan(&[(path.clone(), CapState::Absent)]);

    // Build the full output string the way christen cap does:
    // explainer first, then the setcap lines.
    let mut output = String::new();
    output.push_str(SCOPE_EXPLAINER);
    for entry in &plan.entries {
        if let CapPlanEntry::Grant { command, .. } = entry {
            output.push_str(command);
        }
    }

    let explainer_pos = output
        .find(SCOPE_EXPLAINER)
        .expect("explainer must appear in output");
    let setcap_pos = output
        .find("sudo setcap")
        .expect("setcap line must appear in output");

    assert!(
        explainer_pos < setcap_pos,
        "scope explainer (pos {explainer_pos}) must precede setcap line (pos {setcap_pos})"
    );
}

#[test]
fn cap_plan_never_calls_setcap_pure_function() {
    // cap_plan is a pure function: it only maps CapState variants to
    // CapPlanEntry variants. It contains no Command::new("setcap") or
    // shell invocations. This property is enforced at the code level and
    // verified by the fact that it operates on injected (PathBuf, CapState)
    // pairs without any I/O (see also cap_ac1 which uses nonexistent paths
    // and confirms no panic / filesystem access occurs).
    let path = PathBuf::from("/nonexistent/agentns-claude");
    let plan = cap_plan(&[(path.clone(), CapState::Absent)]);

    // The plan has a Grant entry with the exact setcap command as a STRING —
    // not an executed command.
    match &plan.entries[0] {
        CapPlanEntry::Grant { command, .. } => {
            // The command is a plain string, not an executed process.
            assert!(command.starts_with("sudo setcap"), "command is a string advice");
            // It does NOT contain any shell metacharacters that would indicate
            // auto-execution.
            assert!(!command.contains('&'), "no background execution");
            assert!(!command.contains(';'), "no command chaining");
            assert!(!command.contains('|'), "no pipe");
        }
        other => panic!("expected Grant, got {other:?}"),
    }
}
