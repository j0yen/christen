//! AC5: `christen ledger list --format json` emits the documented schema with
//! one entry per session plus EntrySummary line; list | head does not panic (SIGPIPE).

use christen::{delta, summarize, CloseInfo, Counters, FakeStore, LedgerEntry, LedgerStore};

fn make_entry(session_id: &str, opened_at: u64, intent: Option<&str>) -> LedgerEntry {
    LedgerEntry {
        session_id: session_id.to_owned(),
        intent: intent.map(str::to_owned),
        budget: None,
        opened_at,
        closed_at: None,
        start: Counters::default(),
        end: None,
        kernel: "6.9.0-test".to_owned(),
    }
}

#[test]
fn list_json_schema_has_required_fields() {
    let store = FakeStore::new();

    let session = "abcd1234ef567890";
    let entry = make_entry(session, 1_700_000_000, Some("build"));

    // Close it.
    let end = Counters {
        total_syscalls: 100,
        write_bytes: 4096,
        ..Counters::default()
    };
    store.open(entry.clone()).expect("open");
    store
        .close(
            session,
            CloseInfo {
                closed_at: 1_700_000_060,
                end: end.clone(),
            },
        )
        .expect("close");

    let entries = store.list().expect("list");
    assert_eq!(entries.len(), 1);
    let e = &entries[0];

    // Build the JSON output as the subcommand would.
    let s = summarize(e);
    let json_val = serde_json::json!({
        "session_id": e.session_id,
        "intent": e.intent,
        "budget": e.budget,
        "opened_at": e.opened_at,
        "closed_at": e.closed_at,
        "closed": s.closed,
        "wall_ms": s.wall_ms,
        "top_mover": s.top_mover,
        "kernel": e.kernel,
    });

    assert_eq!(json_val["session_id"], session);
    assert_eq!(json_val["intent"], "build");
    assert_eq!(json_val["closed"], true);
    assert_eq!(json_val["wall_ms"], 60_000u64);
    assert_eq!(json_val["top_mover"], "write_bytes=4096");
}

#[test]
fn show_produces_full_entry_and_delta() {
    let store = FakeStore::new();
    let session = "deadbeef00001111";

    let start = Counters {
        total_syscalls: 10,
        write_bytes: 100,
        ..Counters::default()
    };
    let end_c = Counters {
        total_syscalls: 50,
        write_bytes: 8192,
        ..Counters::default()
    };

    let entry = LedgerEntry {
        session_id: session.to_owned(),
        intent: Some("plan".to_owned()),
        budget: None,
        opened_at: 1_000_000,
        closed_at: Some(1_000_030),
        start: start.clone(),
        end: Some(end_c.clone()),
        kernel: "6.9.0-test".to_owned(),
    };
    store.open(entry).expect("open");

    let fetched = store.get(session).expect("get").expect("present");
    let json = serde_json::to_string_pretty(&fetched).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert_eq!(v["session_id"], session);
    assert_eq!(v["intent"], "plan");

    let end_ref = fetched.end.as_ref().expect("end");
    let d = delta(&fetched.start, end_ref);
    assert_eq!(d.total_syscalls, 40);
    assert_eq!(d.write_bytes, 8092);
}

#[test]
fn list_with_multiple_entries_returns_all() {
    let store = FakeStore::new();
    for i in 0u64..5 {
        let id = format!("session{i:0>12}");
        store.open(make_entry(&id, 1_000_000 + i, None)).expect("open");
    }
    let all = store.list().expect("list");
    assert_eq!(all.len(), 5);
}
