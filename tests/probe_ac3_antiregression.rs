//! AC3: Anti-regression — no NsState variant renders prose containing
//! "registration failed".

use christen::probe::{classify, verdict, InitReason, NsReading, NsState, INIT_AGENT_INODE};

fn all_variants() -> Vec<(NsState, bool)> {
    vec![
        // Absent on wintermute
        (NsState::Absent, true),
        // Absent on stock
        (NsState::Absent, false),
        // Init — UnwrappedExpected on wintermute
        (
            NsState::Init {
                reason: InitReason::UnwrappedExpected,
            },
            true,
        ),
        // Init — UnwrappedExpected on stock
        (
            NsState::Init {
                reason: InitReason::UnwrappedExpected,
            },
            false,
        ),
        // Init — NoWrapperInstalled
        (
            NsState::Init {
                reason: InitReason::NoWrapperInstalled,
            },
            false,
        ),
        // Live
        (
            NsState::Live {
                session_hex: "deadbeef0000000000000000cafebabe".to_owned(),
                intent: None,
            },
            true,
        ),
        // Malformed
        (
            NsState::Malformed {
                detail: "test malformed".to_owned(),
            },
            true,
        ),
    ]
}

#[test]
fn no_prose_contains_registration_failed() {
    for (state, kernel_is_wintermute) in all_variants() {
        let v = verdict(&state, kernel_is_wintermute);
        assert!(
            !v.prose.contains("registration failed"),
            "verdict prose for state={state:?} kernel_is_wintermute={kernel_is_wintermute} \
             must not contain 'registration failed', got: {:?}",
            v.prose
        );
    }
}

/// Extra: verify the all-zeros reading via classify also avoids the banned string.
#[test]
fn classify_all_zeros_no_registration_failed() {
    let reading = NsReading {
        ns_inode: Some(INIT_AGENT_INODE),
        session_hex: Some("00000000000000000000000000000000".to_owned()),
        counters: None,
        kernel_is_wintermute: true,
        wrapper_installed: true,
    };
    let state = classify(&reading);
    let v = verdict(&state, true);
    assert!(
        !v.prose.contains("registration failed"),
        "prose for all-zeros reading must not contain 'registration failed'"
    );
}
