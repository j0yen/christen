//! The [`LaunchSiteSource`] trait and the in-memory [`FakeSource`] for testing.

use crate::model::RawSite;

/// Error type for source discovery failures.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// An I/O error occurred during discovery.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A parse error occurred during discovery.
    #[error("parse error: {0}")]
    Parse(String),
}

/// Abstracts the discovery of launch sites.
///
/// `plan()` is pure and does not call this trait directly — the caller
/// resolves sites via a concrete source and passes `&[RawSite]` to `plan()`.
/// The trait exists so `christen-route`, `christen-cap`, and `christen-detect`
/// all share one contract.
pub trait LaunchSiteSource {
    /// Returns the raw discovered sites.
    ///
    /// # Errors
    /// Returns [`SourceError`] if discovery fails (e.g. I/O error).
    fn sites(&self) -> Result<Vec<RawSite>, SourceError>;
}

/// An in-memory source backed by a fixed list of [`RawSite`]s.
///
/// Used in tests and fixtures. Never touches the filesystem.
#[derive(Debug, Clone)]
pub struct FakeSource {
    sites: Vec<RawSite>,
}

impl FakeSource {
    /// Creates a new `FakeSource` from the given list of sites.
    #[must_use]
    pub const fn new(sites: Vec<RawSite>) -> Self {
        Self { sites }
    }
}

impl LaunchSiteSource for FakeSource {
    fn sites(&self) -> Result<Vec<RawSite>, SourceError> {
        Ok(self.sites.clone())
    }
}
