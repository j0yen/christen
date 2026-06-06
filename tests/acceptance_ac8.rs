//! AC8: README documents the config format, the type surface, the intent_for
//! derivation table, and the LaunchSiteSource trait so christen-detect /
//! christen-route / christen-cap / christen-ledger have a contract to extend.

/// Verify README exists and documents the required sections.
#[test]
fn test_readme_documents_public_api() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let readme_path = std::path::Path::new(manifest_dir).join("README.md");

    let readme = std::fs::read_to_string(&readme_path)
        .unwrap_or_else(|_| panic!("README.md must exist at {:?}", readme_path));

    // Config format documented.
    assert!(
        readme.contains("christen.toml") || readme.contains("ChristenConfig"),
        "README must document the config format (christen.toml or ChristenConfig)"
    );

    // Type surface documented.
    for type_name in &["LaunchSite", "SiteKind", "WrapState", "RouteAction", "RoutePlan"] {
        assert!(
            readme.contains(type_name),
            "README must document type {type_name}"
        );
    }

    // intent_for derivation table documented.
    assert!(
        readme.contains("intent_for") || readme.contains("intent_overrides"),
        "README must document the intent_for derivation table"
    );

    // LaunchSiteSource trait documented.
    assert!(
        readme.contains("LaunchSiteSource"),
        "README must document the LaunchSiteSource trait for downstream crates"
    );
}
