//! AC3: ChristenConfig::load parses config/christen.example.toml; a fixture
//! yields expected default_budget + systemd_dir + one intent_overrides entry,
//! and an absent file yields documented defaults.

use std::path::Path;

use christen::ChristenConfig;

#[test]
fn test_christen_config_load() {
    // Load the example config (relative to workspace root at build time).
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let example_path = Path::new(manifest_dir).join("config/christen.example.toml");

    let config = ChristenConfig::load(&example_path).expect("example config must parse");

    assert_eq!(
        config.default_budget, "wall=7200s,fork=2000",
        "default_budget mismatch"
    );
    assert!(
        config
            .systemd_dir
            .to_string_lossy()
            .contains(".config/systemd/user"),
        "systemd_dir should contain .config/systemd/user, got {:?}",
        config.systemd_dir
    );
    assert_eq!(
        config.intent_overrides.get("claude-build.service"),
        Some(&"/build".to_owned()),
        "intent_overrides must contain claude-build.service -> /build"
    );

    // Absent file yields defaults.
    let absent = Path::new("/nonexistent/path/christen.toml");
    let defaults = ChristenConfig::load(absent).expect("absent file must yield defaults");
    assert_eq!(defaults.default_budget, "wall=7200s,fork=2000");
    assert!(defaults.intent_overrides.is_empty());
}
