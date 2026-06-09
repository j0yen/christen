//! AC6: christen probe --emit shells `docket` with the mapped op.
//!
//! Uses a fake `docket` binary on PATH that records its argv to a temp file.
//! Also verifies a missing `docket` binary is handled non-fatally.

// std::env::set_var is unsafe in Rust 1.85+ (UB in multi-threaded programs).
// These tests are single-threaded integration tests; no other threads touch PATH.
#![allow(unsafe_code)]
#![allow(clippy::undocumented_unsafe_blocks)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use christen::probe::{apply_docket_op, DocketOp};

/// Write a fake `docket` script that appends its argv to a record file.
fn make_fake_docket(tmpdir: &PathBuf, record_file: &PathBuf) {
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> {}\n",
        record_file.display()
    );
    let docket_path = tmpdir.join("docket");
    fs::write(&docket_path, script).expect("write fake docket");
    let mut perms = fs::metadata(&docket_path)
        .expect("metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&docket_path, perms).expect("chmod");
}

#[test]
fn emit_resolve_shells_docket_resolve() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let tmpdir_path = tmpdir.path().to_path_buf();
    let record = tmpdir_path.join("docket-calls.txt");
    make_fake_docket(&tmpdir_path, &record);

    // Prepend tmpdir to PATH so our fake docket is found.
    let orig_path = std::env::var_os("PATH").unwrap_or_default();
    let new_path = {
        let mut p = std::ffi::OsString::from(tmpdir_path.as_os_str());
        p.push(":");
        p.push(&orig_path);
        p
    };
    // SAFETY: single-threaded test; no other threads reading PATH concurrently.
    unsafe { std::env::set_var("PATH", &new_path) };

    let op = DocketOp::Resolve {
        id: "agentns-session-zeros".to_owned(),
    };
    apply_docket_op(&op).expect("apply non-fatal");

    // Restore PATH.
    // SAFETY: same test, single-threaded.
    unsafe { std::env::set_var("PATH", orig_path) };

    let recorded = fs::read_to_string(&record).unwrap_or_default();
    assert!(
        recorded.contains("resolve") && recorded.contains("agentns-session-zeros"),
        "fake docket should have been called with 'resolve agentns-session-zeros', got: {recorded:?}"
    );
}

#[test]
fn emit_report_shells_docket_report() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let tmpdir_path = tmpdir.path().to_path_buf();
    let record = tmpdir_path.join("docket-calls.txt");
    make_fake_docket(&tmpdir_path, &record);

    let orig_path = std::env::var_os("PATH").unwrap_or_default();
    let new_path = {
        let mut p = std::ffi::OsString::from(tmpdir_path.as_os_str());
        p.push(":");
        p.push(&orig_path);
        p
    };
    // SAFETY: single-threaded test.
    unsafe { std::env::set_var("PATH", &new_path) };

    let op = DocketOp::Report {
        severity: "warn".to_owned(),
        title: "agentns init NS — run christen route".to_owned(),
        evidence: "session zeros; wrapper installed".to_owned(),
    };
    apply_docket_op(&op).expect("apply non-fatal");

    // SAFETY: single-threaded test.
    unsafe { std::env::set_var("PATH", orig_path) };

    let recorded = fs::read_to_string(&record).unwrap_or_default();
    assert!(
        recorded.contains("report"),
        "fake docket should have been called with 'report', got: {recorded:?}"
    );
}

#[test]
fn missing_docket_binary_is_nonfatal() {
    // Use a PATH that has no docket binary.
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let tmpdir_path = tmpdir.path().to_path_buf();

    let orig_path = std::env::var_os("PATH").unwrap_or_default();
    // SAFETY: single-threaded test.
    unsafe { std::env::set_var("PATH", tmpdir_path.as_os_str()) };

    let op = DocketOp::Resolve {
        id: "agentns-session-zeros".to_owned(),
    };
    // Must not panic or return Err.
    let result = apply_docket_op(&op);
    // SAFETY: single-threaded test.
    unsafe { std::env::set_var("PATH", orig_path) };
    assert!(result.is_ok(), "missing docket binary must be non-fatal");
}
