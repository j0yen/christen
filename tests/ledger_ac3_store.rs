//! AC3: `LedgerStore` against `FakeStore`:
//! - open then close yields a complete entry with `end` set;
//! - close on an unknown/already-closed id is a no-op (not an error);
//! - list --open-only returns only never-closed entries.

use christen::{CloseInfo, Counters, FakeStore, LedgerEntry, LedgerStore};

fn make_entry(session_id: &str) -> LedgerEntry {
    LedgerEntry {
        session_id: session_id.to_owned(),
        intent: Some("test".to_owned()),
        budget: None,
        opened_at: 1_000_000,
        closed_at: None,
        start: Counters::default(),
        end: None,
        kernel: "6.9.0-test".to_owned(),
    }
}

fn close_info(ts: u64, syscalls: u64) -> CloseInfo {
    CloseInfo {
        closed_at: ts,
        end: Counters { total_syscalls: syscalls, ..Counters::default() },
    }
}

#[test]
fn open_then_close_produces_complete_entry() {
    let store = FakeStore::new();
    let session = "aabbccdd00112233";
    store.open(make_entry(session)).expect("open");
    store.close(session, close_info(1_000_060, 200)).expect("close");

    let entry = store.get(session).expect("get").expect("present");
    assert_eq!(entry.session_id, session);
    assert!(entry.closed_at.is_some(), "closed_at must be set");
    let end = entry.end.expect("end counters must be set");
    assert_eq!(end.total_syscalls, 200);
}

#[test]
fn close_unknown_session_is_noop() {
    let store = FakeStore::new();
    // Should not return an error.
    store.close("nonexistent", close_info(1_000_000, 0)).expect("close unknown must be no-op");
    // Nothing in the store.
    assert!(store.list().expect("list").is_empty());
}

#[test]
fn close_already_closed_is_noop() {
    let store = FakeStore::new();
    let session = "112233445566";
    store.open(make_entry(session)).expect("open");
    store.close(session, close_info(1_000_010, 50)).expect("first close");
    // Second close should not error.
    store.close(session, close_info(1_000_020, 100)).expect("second close must be no-op");

    // Entry should still have the first close's info.
    let entry = store.get(session).expect("get").expect("present");
    let end = entry.end.expect("end set");
    assert_eq!(end.total_syscalls, 50, "second close must not overwrite");
}

#[test]
fn list_open_only_returns_never_closed() {
    let store = FakeStore::new();

    let s1 = "aaaabbbbccccdddd";
    let s2 = "11112222333344";
    let s3 = "zzzzzzzz";

    store.open(make_entry(s1)).expect("open s1");
    store.open(make_entry(s2)).expect("open s2");
    store.open(make_entry(s3)).expect("open s3");

    // Close s2.
    store.close(s2, close_info(1_000_030, 10)).expect("close s2");

    let all = store.list().expect("list");
    assert_eq!(all.len(), 3);

    let open_only: Vec<_> = all.iter().filter(|e| e.closed_at.is_none()).collect();
    assert_eq!(open_only.len(), 2, "s1 and s3 should be open");

    let ids: Vec<&str> = open_only.iter().map(|e| e.session_id.as_str()).collect();
    assert!(ids.contains(&s1));
    assert!(ids.contains(&s3));
    assert!(!ids.contains(&s2));
}

#[test]
fn open_is_idempotent() {
    let store = FakeStore::new();
    let session = "idempotent000001";
    let e = make_entry(session);
    store.open(e.clone()).expect("first open");
    store.open(e).expect("second open must not error");
    let all = store.list().expect("list");
    assert_eq!(all.len(), 1, "only one entry should exist");
}
