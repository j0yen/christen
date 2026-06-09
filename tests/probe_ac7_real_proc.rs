//! AC7: Run against this machine's real /proc/self.
//!
//! This test is gated to a -wintermute kernel; on any other kernel it is
//! skipped (returns Ok immediately via a guard).
//! On a -wintermute kernel it asserts: Init{UnwrappedExpected} with ok == true.
//!
//! deferred_acs:[7] — only meaningful on the laptop; the cloud build box has
//! no -wintermute kernel and will skip this test.

use christen::probe::{classify, verdict, InitReason, NsState, ProcReader, RealProcReader};

#[test]
fn real_proc_self_on_wintermute() {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_default();
    if !release.trim().contains("-wintermute") {
        // Not on a wintermute kernel; skip.
        return;
    }

    let reader = RealProcReader;
    let reading = reader.read(None).expect("RealProcReader must succeed on /proc/self");

    let state = classify(&reading);
    let v = verdict(&state, reading.kernel_is_wintermute);

    match &state {
        NsState::Init {
            reason: InitReason::UnwrappedExpected,
        } => {
            assert!(
                v.ok,
                "Init{{UnwrappedExpected}} on wintermute should have ok=true"
            );
        }
        other => panic!(
            "On -wintermute kernel with all-zeros session, expected Init{{UnwrappedExpected}}, got {other:?}"
        ),
    }
}
