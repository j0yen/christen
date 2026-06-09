//! AC1 (route): render_dropin is pure (no filesystem calls) and emits the
//! clear-then-set ExecStart= / ExecStart=… pair in order.

use std::path::Path;

use christen::render_dropin;

#[test]
fn render_dropin_pure_returns_correct_path() {
    let dir = Path::new("/tmp/fake-systemd");
    let dropin = render_dropin(
        dir,
        "claude-build.service",
        "/home/jsy/.local/bin/claude-build-headless.sh",
        "/build",
        "wall=7200s,fork=2000",
    );
    assert_eq!(
        dropin.path,
        dir.join("claude-build.service.d/10-christen.conf"),
        "drop-in path must be <systemd_dir>/<unit>.d/10-christen.conf"
    );
}

#[test]
fn render_dropin_clear_then_set_ordered() {
    let dropin = render_dropin(
        Path::new("/tmp/sd"),
        "claude-build.service",
        "/home/jsy/.local/bin/claude-build-headless.sh",
        "/build",
        "wall=7200s,fork=2000",
    );
    let lines: Vec<&str> = dropin.contents.lines().collect();

    let clear_pos = lines
        .iter()
        .position(|l| *l == "ExecStart=")
        .expect("must have a bare ExecStart= clear line");

    let set_pos = lines
        .iter()
        .position(|l| l.starts_with("ExecStart=") && l.len() > "ExecStart=".len())
        .expect("must have a non-empty ExecStart= set line");

    assert!(
        clear_pos < set_pos,
        "clear ExecStart= (pos {clear_pos}) must come before the set line (pos {set_pos})"
    );
}

#[test]
fn render_dropin_contains_intent_budget_original() {
    let dropin = render_dropin(
        Path::new("/tmp/sd"),
        "claude-build.service",
        "/home/jsy/.local/bin/claude-build-headless.sh",
        "/build",
        "wall=7200s,fork=2000",
    );
    assert!(
        dropin.contents.contains("--intent /build"),
        "drop-in must contain --intent /build"
    );
    assert!(
        dropin.contents.contains("--budget wall=7200s,fork=2000"),
        "drop-in must contain --budget spec"
    );
    assert!(
        dropin.contents.contains("claude-build-headless.sh"),
        "drop-in must contain the original exec"
    );
}

#[test]
fn render_dropin_contains_service_section() {
    let dropin = render_dropin(
        Path::new("/tmp/sd"),
        "claude-dream.service",
        "/usr/bin/claude-dream.sh",
        "/dream",
        "wall=3600s,fork=500",
    );
    assert!(
        dropin.contents.contains("[Service]"),
        "drop-in must have a [Service] section header"
    );
}
