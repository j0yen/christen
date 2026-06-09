//! AC4: verdict maps states to the documented DocketOp.
//!
//! - Live → Resolve{id: "agentns-session-zeros"}
//! - Init{UnwrappedExpected} on -wintermute → Report (actionable title)
//! - Absent on -wintermute → Report
//! - Any stock-kernel state → None

use christen::probe::{verdict, DocketOp, InitReason, NsState};

#[test]
fn live_gives_resolve() {
    let state = NsState::Live {
        session_hex: "deadbeef00000000000000000000beef".to_owned(),
        intent: None,
    };
    let v = verdict(&state, true);
    match &v.docket {
        DocketOp::Resolve { id } => {
            assert_eq!(id, "agentns-session-zeros");
        }
        other => panic!("expected Resolve, got {other:?}"),
    }
}

#[test]
fn init_unwrapped_wintermute_gives_report() {
    let state = NsState::Init {
        reason: InitReason::UnwrappedExpected,
    };
    let v = verdict(&state, true);
    match &v.docket {
        DocketOp::Report { title, .. } => {
            // Title must be actionable (reference christen route).
            assert!(
                title.contains("agentns") || title.contains("christen"),
                "report title should reference agentns or christen: {title:?}"
            );
            assert!(
                !title.contains("registration failed"),
                "title must not contain 'registration failed'"
            );
        }
        other => panic!("expected Report, got {other:?}"),
    }
}

#[test]
fn absent_wintermute_gives_report() {
    let state = NsState::Absent;
    let v = verdict(&state, true);
    match &v.docket {
        DocketOp::Report { .. } => {}
        other => panic!("expected Report for Absent on wintermute, got {other:?}"),
    }
}

#[test]
fn absent_stock_gives_none() {
    let state = NsState::Absent;
    let v = verdict(&state, false);
    assert_eq!(
        v.docket,
        DocketOp::None,
        "Absent on stock kernel should give DocketOp::None"
    );
}

#[test]
fn live_on_stock_kernel_also_gives_resolve() {
    // Live state is always a good signal regardless of kernel.
    let state = NsState::Live {
        session_hex: "aabbccdd00000000000000001234abcd".to_owned(),
        intent: None,
    };
    let v = verdict(&state, false);
    match &v.docket {
        DocketOp::Resolve { .. } => {}
        other => panic!("expected Resolve for Live, got {other:?}"),
    }
}

#[test]
fn init_no_wrapper_gives_none() {
    let state = NsState::Init {
        reason: InitReason::NoWrapperInstalled,
    };
    let v = verdict(&state, false);
    assert_eq!(
        v.docket,
        DocketOp::None,
        "Init{{NoWrapperInstalled}} on stock should give DocketOp::None"
    );
}

#[test]
fn init_unwrapped_stock_gives_none() {
    let state = NsState::Init {
        reason: InitReason::UnwrappedExpected,
    };
    let v = verdict(&state, false);
    assert_eq!(
        v.docket,
        DocketOp::None,
        "Init on stock kernel should give DocketOp::None"
    );
}
