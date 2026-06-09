//! AC2: classify returns the right NsState for each FakeNsReader fixture.
//!
//! Fixtures:
//!  - all-zero id + init inode + wrapper-installed + -wintermute → Init{UnwrappedExpected}
//!  - nonzero hex id → Live
//!  - missing surface on -wintermute → Absent
//!  - missing surface on stock → Absent (but verdict.ok == true)
//!  - unparseable id → Malformed

use christen::probe::{classify, verdict, InitReason, NsReading, NsState, INIT_AGENT_INODE};

fn zeros_reading() -> NsReading {
    NsReading {
        ns_inode: Some(INIT_AGENT_INODE),
        session_hex: Some("00000000000000000000000000000000".to_owned()),
        counters: None,
        kernel_is_wintermute: true,
        wrapper_installed: true,
    }
}

#[test]
fn all_zeros_init_inode_wrapper_wintermute_gives_init_unwrapped_expected() {
    let reading = zeros_reading();
    let state = classify(&reading);
    assert_eq!(
        state,
        NsState::Init {
            reason: InitReason::UnwrappedExpected
        },
        "all-zeros + init-inode + wrapper + wintermute should give Init{{UnwrappedExpected}}"
    );
}

#[test]
fn nonzero_hex_gives_live() {
    let reading = NsReading {
        ns_inode: Some(4_026_531_997), // different from init inode
        session_hex: Some("deadbeefcafebabe0123456789abcdef".to_owned()),
        counters: None,
        kernel_is_wintermute: true,
        wrapper_installed: true,
    };
    let state = classify(&reading);
    match state {
        NsState::Live { ref session_hex, .. } => {
            assert_eq!(session_hex, "deadbeefcafebabe0123456789abcdef");
        }
        other => panic!("expected Live, got {other:?}"),
    }
}

#[test]
fn missing_surface_on_wintermute_gives_absent() {
    let reading = NsReading {
        ns_inode: None,
        session_hex: None,
        counters: None,
        kernel_is_wintermute: true,
        wrapper_installed: false,
    };
    let state = classify(&reading);
    assert_eq!(state, NsState::Absent);
}

#[test]
fn missing_surface_on_stock_gives_absent_ok() {
    let reading = NsReading {
        ns_inode: None,
        session_hex: None,
        counters: None,
        kernel_is_wintermute: false,
        wrapper_installed: false,
    };
    let state = classify(&reading);
    assert_eq!(state, NsState::Absent);
    // On stock kernel, Absent should be ok == true.
    let v = verdict(&state, false);
    assert!(v.ok, "Absent on stock kernel should be ok=true");
}

#[test]
fn unparseable_id_gives_malformed() {
    let reading = NsReading {
        ns_inode: Some(4_026_531_997),
        session_hex: Some("not-valid-hex!!".to_owned()),
        counters: None,
        kernel_is_wintermute: true,
        wrapper_installed: true,
    };
    let state = classify(&reading);
    match state {
        NsState::Malformed { .. } => {}
        other => panic!("expected Malformed, got {other:?}"),
    }
}
