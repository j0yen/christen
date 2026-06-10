//! AC2: `LedgerEntry` and `Counters` round-trip through `serde_json`;
//! a fixture entry serializes to the documented one-file-per-session shape.

use christen::{Counters, LedgerEntry};

fn fixture_entry() -> LedgerEntry {
    LedgerEntry {
        session_id: "deadbeefcafe0001".to_owned(),
        intent: Some("review".to_owned()),
        budget: Some("normal".to_owned()),
        opened_at: 1_700_000_000,
        closed_at: Some(1_700_000_120),
        start: Counters {
            total_syscalls: 0,
            openat_count: 0,
            write_bytes: 0,
            connect_count: 0,
            unlink_count: 0,
            fork_count: 0,
            elapsed_ns: 0,
        },
        end: Some(Counters {
            total_syscalls: 500,
            openat_count: 40,
            write_bytes: 65536,
            connect_count: 3,
            unlink_count: 0,
            fork_count: 1,
            elapsed_ns: 120_000_000_000,
        }),
        kernel: "6.9.0-wintermute".to_owned(),
    }
}

#[test]
fn counters_round_trip() {
    let c = Counters {
        total_syscalls: 42,
        openat_count: 7,
        write_bytes: 1024,
        connect_count: 2,
        unlink_count: 0,
        fork_count: 1,
        elapsed_ns: 999_000,
    };
    let json = serde_json::to_string(&c).expect("serialize");
    let c2: Counters = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(c, c2);
}

#[test]
fn ledger_entry_round_trip() {
    let e = fixture_entry();
    let json = serde_json::to_string_pretty(&e).expect("serialize");
    let e2: LedgerEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(e.session_id, e2.session_id);
    assert_eq!(e.intent, e2.intent);
    assert_eq!(e.budget, e2.budget);
    assert_eq!(e.opened_at, e2.opened_at);
    assert_eq!(e.closed_at, e2.closed_at);
    assert_eq!(e.start, e2.start);
    assert_eq!(e.end, e2.end);
    assert_eq!(e.kernel, e2.kernel);
}

#[test]
fn entry_serializes_to_expected_fields() {
    let e = fixture_entry();
    let json = serde_json::to_string_pretty(&e).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    let obj = v.as_object().expect("object");
    assert!(obj.contains_key("session_id"), "must have session_id");
    assert!(obj.contains_key("opened_at"), "must have opened_at");
    assert!(obj.contains_key("start"), "must have start");
    assert!(obj.contains_key("end"), "must have end");
    assert!(obj.contains_key("kernel"), "must have kernel");
    assert_eq!(v["session_id"], "deadbeefcafe0001");
    assert_eq!(v["intent"], "review");
    assert_eq!(v["budget"], "normal");
    assert_eq!(v["opened_at"], 1_700_000_000u64);
    assert_eq!(v["closed_at"], 1_700_000_120u64);
}

#[test]
fn open_entry_omits_closed_fields() {
    let mut e = fixture_entry();
    e.closed_at = None;
    e.end = None;
    let json = serde_json::to_string(&e).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    let obj = v.as_object().expect("object");
    assert!(!obj.contains_key("closed_at"), "open entry must omit closed_at");
    assert!(!obj.contains_key("end"), "open entry must omit end");
}
