//! Ledger module — records per-session identity and syscall footprint.
//!
//! At session birth, `christen ledger open` writes a [`LedgerEntry`] with the
//! session's id, intent, budget, and starting counters. At session end,
//! `christen ledger close` patches the entry with `closed_at` and final
//! counters. Entries persist under `~/.claude/christen/ledger/<session_id>.json`.
//!
//! ## Design
//!
//! - [`LedgerStore`] abstracts reads/writes; [`FsStore`] is the real
//!   implementation; [`FakeStore`] is for tests.
//! - Writes are idempotent. Closing an unknown or already-closed session is a
//!   logged no-op, not an error.
//! - Sessions that never `close` (e.g. killed by SIGKILL) leave an open-only
//!   entry — which is itself a signal, not a bug.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// ── Counters ──────────────────────────────────────────────────────────────────

/// Counters from `/proc/$PID/agent_counters`.
///
/// Fields mirror the ledger-specific counter surface (distinct from the probe
/// module's simpler two-field variant).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Counters {
    /// Total syscalls recorded in this namespace.
    pub total_syscalls: u64,
    /// `openat(2)` calls.
    pub openat_count: u64,
    /// Bytes written via `write(2)` / `pwrite(2)`.
    pub write_bytes: u64,
    /// `connect(2)` calls.
    pub connect_count: u64,
    /// `unlink(2)` / `unlinkat(2)` calls.
    pub unlink_count: u64,
    /// `fork(2)` / `clone(2)` calls.
    pub fork_count: u64,
    /// Wall-clock time elapsed in the namespace (nanoseconds).
    pub elapsed_ns: u64,
}

/// Compute the per-field difference between two counter snapshots.
///
/// Each field is saturating-subtracted so that a counter that unexpectedly
/// decreased (e.g. counter reset) yields zero, not a wrapped value.
#[must_use]
pub fn delta(start: &Counters, end: &Counters) -> Counters {
    Counters {
        total_syscalls: end.total_syscalls.saturating_sub(start.total_syscalls),
        openat_count: end.openat_count.saturating_sub(start.openat_count),
        write_bytes: end.write_bytes.saturating_sub(start.write_bytes),
        connect_count: end.connect_count.saturating_sub(start.connect_count),
        unlink_count: end.unlink_count.saturating_sub(start.unlink_count),
        fork_count: end.fork_count.saturating_sub(start.fork_count),
        elapsed_ns: end.elapsed_ns.saturating_sub(start.elapsed_ns),
    }
}

// ── LedgerEntry ───────────────────────────────────────────────────────────────

/// A per-session ledger entry.
///
/// One JSON file per session at `~/.claude/christen/ledger/<session_id>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// The session id (hex string from `/proc/$PID/agent_session`).
    pub session_id: String,
    /// The intent tag (e.g. `"build"`, `"review"`), if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The budget label (e.g. `"tight"`, `"normal"`), if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<String>,
    /// Unix timestamp (seconds) when the session opened.
    pub opened_at: u64,
    /// Unix timestamp (seconds) when the session closed, or `None` if still open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<u64>,
    /// Counter snapshot at session open.
    pub start: Counters,
    /// Counter snapshot at session close, or `None` if still open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<Counters>,
    /// Kernel release string at open time (for attribution).
    pub kernel: String,
}

/// A human-readable summary line for a ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrySummary {
    /// Short prefix of the session id (first 8 hex chars).
    pub id_prefix: String,
    /// Intent tag, or `"—"` if absent.
    pub intent: String,
    /// Wall time in milliseconds, or `None` if the session is still open.
    pub wall_ms: Option<u64>,
    /// Whether the session closed normally (vs. still open / SIGKILL casualty).
    pub closed: bool,
    /// Top syscall mover description (e.g. `"write_bytes=4096"`).
    pub top_mover: String,
}

/// Produce a human-readable [`EntrySummary`] from a [`LedgerEntry`].
///
/// The summary is pure — it only reads the entry.
#[must_use]
pub fn summarize(entry: &LedgerEntry) -> EntrySummary {
    let id_prefix = entry
        .session_id
        .get(..8)
        .unwrap_or(&entry.session_id)
        .to_owned();

    let intent = entry
        .intent
        .as_deref()
        .unwrap_or("—")
        .to_owned();

    let wall_ms = match (entry.closed_at, entry.opened_at) {
        (Some(closed), opened) => {
            let diff_secs = closed.saturating_sub(opened);
            Some(diff_secs.saturating_mul(1000))
        }
        (None, _) => None,
    };

    let closed = entry.closed_at.is_some();

    let top_mover = entry.end.as_ref().map_or_else(
        || "—".to_owned(),
        |end| {
            let d = delta(&entry.start, end);
            // Pick the most-moved counter (excluding elapsed_ns which is always big).
            let candidates = [
                ("write_bytes", d.write_bytes),
                ("total_syscalls", d.total_syscalls),
                ("openat_count", d.openat_count),
                ("connect_count", d.connect_count),
                ("fork_count", d.fork_count),
                ("unlink_count", d.unlink_count),
            ];
            candidates
                .iter()
                .max_by_key(|&&(_, v)| v)
                .map(|&(name, v)| format!("{name}={v}"))
                .unwrap_or_else(|| "—".to_owned())
        },
    );

    EntrySummary {
        id_prefix,
        intent,
        wall_ms,
        closed,
        top_mover,
    }
}

// ── LedgerStore trait ─────────────────────────────────────────────────────────

/// Trait for persisting ledger entries.
pub trait LedgerStore: Send + Sync {
    /// Write an open entry (session birth). Idempotent if already present.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the backing store cannot be written.
    fn open(&self, entry: LedgerEntry) -> Result<(), LedgerError>;

    /// Patch an entry with `closed_at` and final counters.
    ///
    /// If the session id is unknown or the entry is already closed, logs and
    /// returns `Ok(())` (no-op).
    ///
    /// # Errors
    ///
    /// Returns `Err` on I/O failure.
    fn close(&self, session_id: &str, end: CloseInfo) -> Result<(), LedgerError>;

    /// List all entries.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the store cannot be read.
    fn list(&self) -> Result<Vec<LedgerEntry>, LedgerError>;

    /// Retrieve a single entry by session id.
    ///
    /// Returns `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the store cannot be read.
    fn get(&self, session_id: &str) -> Result<Option<LedgerEntry>, LedgerError>;
}

/// Information supplied when closing a session.
#[derive(Debug, Clone)]
pub struct CloseInfo {
    /// Timestamp to record as `closed_at` (Unix seconds).
    pub closed_at: u64,
    /// Final counter snapshot.
    pub end: Counters,
}

// ── LedgerError ───────────────────────────────────────────────────────────────

/// Errors from ledger operations.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    /// I/O failure reading or writing the store.
    #[error("ledger I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization/deserialization failure.
    #[error("ledger JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Store directory could not be determined.
    #[error("ledger store dir unknown: {0}")]
    StoreDir(String),
}

// ── FsStore ───────────────────────────────────────────────────────────────────

/// Filesystem-backed [`LedgerStore`].
///
/// Entries are stored as `<dir>/<session_id>.json`.
pub struct FsStore {
    dir: PathBuf,
}

impl FsStore {
    /// Create a new `FsStore` rooted at `dir`, creating the directory if absent.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::Io`] if the directory cannot be created.
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self, LedgerError> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Return the default store dir: `~/.claude/christen/ledger/`.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::StoreDir`] if the home directory cannot be
    /// determined.
    pub fn default_dir() -> Result<PathBuf, LedgerError> {
        dirs_next::home_dir()
            .map(|h| h.join(".claude/christen/ledger"))
            .ok_or_else(|| LedgerError::StoreDir("home directory unknown".to_owned()))
    }

    fn entry_path(&self, session_id: &str) -> PathBuf {
        self.dir.join(format!("{session_id}.json"))
    }

    fn read_entry(&self, path: &Path) -> Result<LedgerEntry, LedgerError> {
        let s = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&s)?)
    }
}

impl LedgerStore for FsStore {
    fn open(&self, entry: LedgerEntry) -> Result<(), LedgerError> {
        let path = self.entry_path(&entry.session_id);
        // Idempotent: don't overwrite an existing entry.
        if path.exists() {
            return Ok(());
        }
        let json = serde_json::to_string_pretty(&entry)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    fn close(&self, session_id: &str, info: CloseInfo) -> Result<(), LedgerError> {
        let path = self.entry_path(session_id);
        if !path.exists() {
            eprintln!("christen ledger close: unknown session {session_id} — no-op");
            return Ok(());
        }
        let mut entry = self.read_entry(&path)?;
        if entry.closed_at.is_some() {
            eprintln!("christen ledger close: session {session_id} already closed — no-op");
            return Ok(());
        }
        entry.closed_at = Some(info.closed_at);
        entry.end = Some(info.end);
        let json = serde_json::to_string_pretty(&entry)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<LedgerEntry>, LedgerError> {
        let mut entries = Vec::new();
        let read_dir = match std::fs::read_dir(&self.dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(LedgerError::Io(e)),
        };
        for item in read_dir.flatten() {
            let path = item.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match self.read_entry(&path) {
                    Ok(e) => entries.push(e),
                    Err(err) => {
                        eprintln!("christen ledger list: skipping {:?}: {err}", path.display());
                    }
                }
            }
        }
        Ok(entries)
    }

    fn get(&self, session_id: &str) -> Result<Option<LedgerEntry>, LedgerError> {
        let path = self.entry_path(session_id);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(self.read_entry(&path)?))
    }
}

// ── FakeStore ─────────────────────────────────────────────────────────────────

/// In-memory [`LedgerStore`] for tests.
#[derive(Debug, Default, Clone)]
pub struct FakeStore {
    entries: Arc<Mutex<HashMap<String, LedgerEntry>>>,
}

impl FakeStore {
    /// Create an empty `FakeStore`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl LedgerStore for FakeStore {
    fn open(&self, entry: LedgerEntry) -> Result<(), LedgerError> {
        let mut map = self
            .entries
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        map.entry(entry.session_id.clone()).or_insert(entry);
        Ok(())
    }

    fn close(&self, session_id: &str, info: CloseInfo) -> Result<(), LedgerError> {
        let mut map = self
            .entries
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = map.get_mut(session_id) {
            if entry.closed_at.is_some() {
                eprintln!("christen ledger close: session {session_id} already closed — no-op");
            } else {
                entry.closed_at = Some(info.closed_at);
                entry.end = Some(info.end);
            }
        } else {
            eprintln!("christen ledger close: unknown session {session_id} — no-op");
        }
        Ok(())
    }

    fn list(&self) -> Result<Vec<LedgerEntry>, LedgerError> {
        let map = self
            .entries
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        Ok(map.values().cloned().collect())
    }

    fn get(&self, session_id: &str) -> Result<Option<LedgerEntry>, LedgerError> {
        let map = self
            .entries
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        Ok(map.get(session_id).cloned())
    }
}

// ── CounterReader trait ───────────────────────────────────────────────────────

/// Trait for reading agent counters from `/proc`.
///
/// The real implementation reads `/proc/self/agent_counters` and
/// `/proc/self/agent_session`. The [`FakeCounterReader`] is for tests.
pub trait CounterReader: Send + Sync {
    /// Read the session id (hex string), or `None` if absent.
    fn session_id(&self) -> Option<String>;
    /// Read the current counter snapshot, or `None` if absent.
    fn counters(&self) -> Option<Counters>;
    /// Read the intent tag, or `None` if not set.
    fn intent(&self) -> Option<String>;
    /// Read the budget label, or `None` if not set.
    fn budget(&self) -> Option<String>;
    /// Read the kernel release string.
    fn kernel(&self) -> String;
}

/// Real [`CounterReader`] that reads `/proc/self`.
pub struct RealCounterReader;

impl CounterReader for RealCounterReader {
    fn session_id(&self) -> Option<String> {
        std::fs::read_to_string("/proc/self/agent_session")
            .ok()
            .map(|s| s.trim().to_owned())
    }

    fn counters(&self) -> Option<Counters> {
        let s = std::fs::read_to_string("/proc/self/agent_counters").ok()?;
        parse_ledger_counters(&s)
    }

    fn intent(&self) -> Option<String> {
        std::fs::read_to_string("/proc/self/agent_intent")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    fn budget(&self) -> Option<String> {
        std::fs::read_to_string("/proc/self/agent_budget")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    fn kernel(&self) -> String {
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_default()
            .trim()
            .to_owned()
    }
}

/// Parse the ledger counter format from `/proc/$PID/agent_counters`.
///
/// Expected format (one `key: value` per line):
/// ```text
/// total_syscalls: 12345
/// openat_count: 100
/// write_bytes: 4096
/// connect_count: 5
/// unlink_count: 0
/// fork_count: 1
/// elapsed_ns: 9000000
/// ```
fn parse_ledger_counters(s: &str) -> Option<Counters> {
    let mut c = Counters::default();
    let mut found = false;
    for line in s.lines() {
        let Some((key, val)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let Ok(v) = val.trim().parse::<u64>() else {
            continue;
        };
        found = true;
        match key {
            "total_syscalls" => c.total_syscalls = v,
            "openat_count" => c.openat_count = v,
            "write_bytes" => c.write_bytes = v,
            "connect_count" => c.connect_count = v,
            "unlink_count" => c.unlink_count = v,
            "fork_count" => c.fork_count = v,
            "elapsed_ns" => c.elapsed_ns = v,
            _ => {}
        }
    }
    if found { Some(c) } else { None }
}

/// A fake [`CounterReader`] for tests.
pub struct FakeCounterReader {
    /// Session id to return.
    pub session_id: Option<String>,
    /// Counters to return.
    pub counters: Option<Counters>,
    /// Intent to return.
    pub intent: Option<String>,
    /// Budget to return.
    pub budget: Option<String>,
    /// Kernel release to return.
    pub kernel: String,
}

impl FakeCounterReader {
    /// Create a `FakeCounterReader` with sane defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            session_id: Some("abcdef1234567890".to_owned()),
            counters: Some(Counters::default()),
            intent: None,
            budget: None,
            kernel: "6.9.0-test".to_owned(),
        }
    }
}

impl Default for FakeCounterReader {
    fn default() -> Self {
        Self::new()
    }
}

impl CounterReader for FakeCounterReader {
    fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    fn counters(&self) -> Option<Counters> {
        self.counters.clone()
    }

    fn intent(&self) -> Option<String> {
        self.intent.clone()
    }

    fn budget(&self) -> Option<String> {
        self.budget.clone()
    }

    fn kernel(&self) -> String {
        self.kernel.clone()
    }
}

// ── Ledger commands ───────────────────────────────────────────────────────────

/// Open a ledger entry for the current session.
///
/// Reads the session id, intent, budget, and counters from `reader`, then
/// calls [`LedgerStore::open`] on `store`.
///
/// # Errors
///
/// Returns an error if the store write fails, or if the session id is absent
/// (no agent namespace).
pub fn cmd_open(
    reader: &dyn CounterReader,
    store: &dyn LedgerStore,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = reader
        .session_id()
        .filter(|s| !s.is_empty() && s.chars().any(|c| c != '0'))
        .ok_or("no live session id — is this session running inside agentns-claude?")?;

    let start = reader.counters().unwrap_or_default();
    let intent = reader.intent();
    let budget = reader.budget();
    let kernel = reader.kernel();
    let opened_at = now_secs();

    let entry = LedgerEntry {
        session_id,
        intent,
        budget,
        opened_at,
        closed_at: None,
        start,
        end: None,
        kernel,
    };

    store.open(entry)?;
    Ok(())
}

/// Close the ledger entry for the current session.
///
/// Reads the session id and final counters from `reader`, then calls
/// [`LedgerStore::close`]. Closing an unknown or already-closed session is a
/// logged no-op.
///
/// # Errors
///
/// Returns an error if the store write fails.
pub fn cmd_close(
    reader: &dyn CounterReader,
    store: &dyn LedgerStore,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = reader
        .session_id()
        .filter(|s| !s.is_empty())
        .ok_or("no session id — cannot close")?;

    let end = reader.counters().unwrap_or_default();
    let closed_at = now_secs();

    store.close(&session_id, CloseInfo { closed_at, end })?;
    Ok(())
}

/// Return the current time as Unix seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Print the SessionStart + SessionEnd hook JSON for `settings.json`.
///
/// Never writes anything to disk or shells any command.
pub fn cmd_install() {
    println!(
        r#"# christen ledger install — hook entries for ~/.claude/settings.json
#
# NOTE: these hooks are INERT until christen-route and christen-cap have
# made the session id real (non-zero). On an unwrapped session,
# `christen ledger open` will exit with an error and the hook will be a no-op.
#
# Add the following entries to your settings.json hooks array:

{{
  "hooks": {{
    "SessionStart": [
      {{
        "matcher": "",
        "hooks": [
          {{
            "type": "command",
            "command": "christen ledger open"
          }}
        ]
      }}
    ],
    "SessionEnd": [
      {{
        "matcher": "",
        "hooks": [
          {{
            "type": "command",
            "command": "christen ledger close"
          }}
        ]
      }}
    ]
  }}
}}"#
    );
}
