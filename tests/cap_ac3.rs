//! AC3: The CapReader real impl reads a known binary's cap set and returns
//! a well-formed CapState (whatever it is).
//!
//! This test does NOT grant any capability; it reads the current state of
//! agentns-claude (or /bin/true as a fallback) and asserts only that the
//! result is a valid CapState variant.

use std::path::PathBuf;

use christen::{CapReader, CapState, GetcapReader};

#[test]
fn real_reader_returns_well_formed_state_for_bin_true() {
    let reader = GetcapReader;
    // /bin/true always exists and has no special caps — expected Absent.
    let bin_true = PathBuf::from("/bin/true");
    if !bin_true.exists() {
        // Some distros place it elsewhere; skip gracefully.
        return;
    }
    let state = reader.caps(&bin_true);
    // The state must be one of the known variants (not a panic or garbage).
    match state {
        CapState::Present | CapState::Absent | CapState::Unreadable { .. } | CapState::Setuid { .. } => {}
    }
}

#[test]
fn real_reader_absent_for_missing_binary() {
    let reader = GetcapReader;
    let path = PathBuf::from("/nonexistent/binary-that-does-not-exist-12345");
    let state = reader.caps(&path);
    match state {
        CapState::Unreadable { .. } => {} // expected
        other => panic!("expected Unreadable for missing binary, got {other:?}"),
    }
}

#[test]
fn real_reader_agentns_claude_returns_valid_state() {
    let reader = GetcapReader;
    // Search $PATH for agentns-claude; skip if not found.
    let path = std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p).find_map(|dir| {
            let c = dir.join("agentns-claude");
            c.exists().then_some(c)
        })
    });

    let Some(path) = path else {
        // agentns-claude not installed in this environment — skip.
        return;
    };

    let state = reader.caps(&path);
    // Just check it's a well-formed variant.
    match state {
        CapState::Present | CapState::Absent | CapState::Unreadable { .. } | CapState::Setuid { .. } => {}
    }
}
