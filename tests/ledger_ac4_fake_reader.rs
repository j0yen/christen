//! AC4: `christen ledger open`/`close` drive the store from injected counters
//! in a `FakeCounterReader` integration test (no real `/proc` dependency);
//! the entry's `delta` matches the injected movement.

use christen::{cmd_close, cmd_open, delta, Counters, FakeCounterReader, FakeStore, LedgerStore};

fn make_reader(session_id: &str, counters: Counters) -> FakeCounterReader {
    FakeCounterReader {
        session_id: Some(session_id.to_owned()),
        counters: Some(counters),
        intent: Some("build".to_owned()),
        budget: Some("normal".to_owned()),
        kernel: "6.9.0-test".to_owned(),
    }
}

#[test]
fn open_close_delta_matches_injected_movement() {
    let session = "cafe0123beef4567";
    let start_counters = Counters {
        total_syscalls: 100,
        openat_count: 10,
        write_bytes: 1024,
        connect_count: 2,
        unlink_count: 0,
        fork_count: 1,
        elapsed_ns: 5_000_000,
    };
    let end_counters = Counters {
        total_syscalls: 250,
        openat_count: 25,
        write_bytes: 16384,
        connect_count: 5,
        unlink_count: 1,
        fork_count: 2,
        elapsed_ns: 10_000_000,
    };

    let open_reader = make_reader(session, start_counters.clone());
    let store = FakeStore::new();

    // Open
    cmd_open(&open_reader, &store).expect("open");

    // Verify open entry exists.
    let entry = store.get(session).expect("get").expect("present after open");
    assert_eq!(entry.session_id, session);
    assert_eq!(entry.start, start_counters);
    assert!(entry.closed_at.is_none());
    assert!(entry.end.is_none());

    // Close (simulate with direct store close, since cmd_close reads from proc)
    let close_reader = make_reader(session, end_counters.clone());
    cmd_close(&close_reader, &store).expect("close");

    // Verify closed entry.
    let entry = store.get(session).expect("get").expect("present after close");
    assert!(entry.closed_at.is_some(), "closed_at must be set");
    let end = entry.end.clone().expect("end must be set");
    assert_eq!(end, end_counters);

    // Verify delta.
    let d = delta(&entry.start, &end);
    assert_eq!(d.total_syscalls, 150);
    assert_eq!(d.openat_count, 15);
    assert_eq!(d.write_bytes, 15360);
    assert_eq!(d.connect_count, 3);
    assert_eq!(d.unlink_count, 1);
    assert_eq!(d.fork_count, 1);
    assert_eq!(d.elapsed_ns, 5_000_000);
}

#[test]
fn cmd_open_fails_on_all_zero_session_id() {
    let reader = FakeCounterReader {
        session_id: Some("00000000000000000000000000000000".to_owned()),
        counters: Some(Counters::default()),
        intent: None,
        budget: None,
        kernel: "6.9.0-test".to_owned(),
    };
    let store = FakeStore::new();
    let result = cmd_open(&reader, &store);
    assert!(result.is_err(), "all-zero session id should fail");
}

#[test]
fn cmd_open_fails_on_absent_session_id() {
    let reader = FakeCounterReader {
        session_id: None,
        counters: Some(Counters::default()),
        intent: None,
        budget: None,
        kernel: "6.9.0-test".to_owned(),
    };
    let store = FakeStore::new();
    let result = cmd_open(&reader, &store);
    assert!(result.is_err(), "missing session id should fail");
}

#[test]
fn cmd_open_stores_intent_and_budget() {
    let session = "1234abcd5678ef90";
    let reader = FakeCounterReader {
        session_id: Some(session.to_owned()),
        counters: Some(Counters::default()),
        intent: Some("review".to_owned()),
        budget: Some("tight".to_owned()),
        kernel: "6.9.0-test".to_owned(),
    };
    let store = FakeStore::new();
    cmd_open(&reader, &store).expect("open");

    let entry = store.get(session).expect("get").expect("present");
    assert_eq!(entry.intent.as_deref(), Some("review"));
    assert_eq!(entry.budget.as_deref(), Some("tight"));
}
