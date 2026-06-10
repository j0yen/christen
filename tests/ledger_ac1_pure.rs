//! AC1: `delta` and `summarize` are pure; delta of equal counters is all-zero;
//! a wall-time summarize covers a fixture with known movers.

use christen::{delta, summarize, Counters, LedgerEntry};

fn zero() -> Counters {
    Counters::default()
}

fn make_entry(start: Counters, end: Option<Counters>, opened_at: u64, closed_at: Option<u64>) -> LedgerEntry {
    LedgerEntry {
        session_id: "aabbccdd11223344".to_owned(),
        intent: Some("build".to_owned()),
        budget: None,
        opened_at,
        closed_at,
        start,
        end,
        kernel: "6.9.0-test".to_owned(),
    }
}

#[test]
fn delta_equal_counters_is_all_zero() {
    let c = Counters {
        total_syscalls: 100,
        openat_count: 20,
        write_bytes: 4096,
        connect_count: 3,
        unlink_count: 1,
        fork_count: 2,
        elapsed_ns: 1_000_000,
    };
    let d = delta(&c, &c);
    assert_eq!(d, zero());
}

#[test]
fn delta_computes_differences() {
    let start = Counters {
        total_syscalls: 10,
        openat_count: 5,
        write_bytes: 100,
        connect_count: 1,
        unlink_count: 0,
        fork_count: 0,
        elapsed_ns: 500,
    };
    let end = Counters {
        total_syscalls: 25,
        openat_count: 12,
        write_bytes: 8192,
        connect_count: 3,
        unlink_count: 1,
        fork_count: 1,
        elapsed_ns: 2_000_000,
    };
    let d = delta(&start, &end);
    assert_eq!(d.total_syscalls, 15);
    assert_eq!(d.openat_count, 7);
    assert_eq!(d.write_bytes, 8092);
    assert_eq!(d.connect_count, 2);
    assert_eq!(d.unlink_count, 1);
    assert_eq!(d.fork_count, 1);
    assert_eq!(d.elapsed_ns, 1_999_500);
}

#[test]
fn delta_saturates_on_counter_decrease() {
    let start = Counters { total_syscalls: 100, ..Counters::default() };
    let end = Counters { total_syscalls: 50, ..Counters::default() };
    let d = delta(&start, &end);
    assert_eq!(d.total_syscalls, 0, "saturating_sub must not wrap");
}

#[test]
fn summarize_open_session_has_none_wall_ms() {
    let entry = make_entry(zero(), None, 1_000_000, None);
    let s = summarize(&entry);
    assert_eq!(s.closed, false);
    assert_eq!(s.wall_ms, None);
    assert_eq!(s.intent, "build");
}

#[test]
fn summarize_closed_session_computes_wall_ms() {
    let start = Counters { write_bytes: 0, ..Counters::default() };
    let end = Counters { write_bytes: 4096, ..Counters::default() };
    let entry = make_entry(start, Some(end), 1_000_000, Some(1_000_005));
    let s = summarize(&entry);
    assert_eq!(s.closed, true);
    assert_eq!(s.wall_ms, Some(5_000)); // 5 seconds * 1000
    assert_eq!(s.top_mover, "write_bytes=4096");
}

#[test]
fn summarize_id_prefix_is_8_chars() {
    let entry = make_entry(zero(), None, 0, None);
    let s = summarize(&entry);
    assert_eq!(s.id_prefix, "aabbccdd");
}

#[test]
fn summarize_no_intent_shows_dash() {
    let mut entry = make_entry(zero(), None, 0, None);
    entry.intent = None;
    let s = summarize(&entry);
    assert_eq!(s.intent, "—");
}
