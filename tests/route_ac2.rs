//! AC2 (route): SystemdSource parses a fixture unit directory into RawSites with
//! the correct exec_line; a claude-build.service fixture yields a site whose
//! derived intent is /build and whose Wire.to wraps the original ExecStart.

use std::fs;

use christen::{plan, ChristenConfig, KernelInfo, LaunchSiteSource, RouteAction};
use christen::route::SystemdSource;
use tempfile::TempDir;

fn write_unit(dir: &std::path::Path, name: &str, exec_start: &str) {
    let content = format!(
        "[Unit]\nDescription=Test {name}\n\n[Service]\nExecStart={exec_start}\n\n[Install]\nWantedBy=default.target\n"
    );
    fs::write(dir.join(name), content).expect("write unit fixture");
}

#[test]
fn systemd_source_parses_claude_build_unit() {
    let tmp = TempDir::new().expect("tempdir");
    let exec = "/home/jsy/.local/bin/claude-build-headless.sh";
    write_unit(tmp.path(), "claude-build.service", exec);

    let source = SystemdSource::new(tmp.path().to_owned());
    let sites = source.sites().expect("sites() must not fail on readable dir");

    // Should find claude-build.service + the synthetic interactive site.
    let build_site = sites
        .iter()
        .find(|s| s.id == "claude-build.service")
        .expect("claude-build.service site must be present");

    assert_eq!(
        build_site.exec_line, exec,
        "exec_line must match the ExecStart value from the fixture"
    );
}

#[test]
fn systemd_source_derived_intent_is_build() {
    let tmp = TempDir::new().expect("tempdir");
    write_unit(
        tmp.path(),
        "claude-build.service",
        "/home/jsy/.local/bin/claude-build-headless.sh",
    );

    let source = SystemdSource::new(tmp.path().to_owned());
    let raw = source.sites().expect("sites");

    let kernel = KernelInfo {
        agent_ns: true,
        release: "6.9.0-arch1-5-wintermute".to_owned(),
    };
    let mut config = ChristenConfig::default();
    config.systemd_dir = tmp.path().to_owned();
    let route_plan = plan(&raw, &kernel, true, &config);

    // Find the Wire action for claude-build.service.
    let wire = route_plan.actions.iter().find_map(|a| {
        if let RouteAction::Wire { site, .. } = a {
            if site == "claude-build.service" {
                return Some(a);
            }
        }
        None
    });

    let wire = wire.expect("claude-build.service must produce a Wire action");
    if let RouteAction::Wire { to, .. } = wire {
        assert!(
            to.contains("--intent /build"),
            "Wire.to must contain '--intent /build', got: {to}"
        );
        assert!(
            to.contains("agentns-claude"),
            "Wire.to must contain 'agentns-claude', got: {to}"
        );
        assert!(
            to.contains("claude-build-headless.sh"),
            "Wire.to must contain the original exec, got: {to}"
        );
    }
}

#[test]
fn systemd_source_ignores_non_claude_units() {
    let tmp = TempDir::new().expect("tempdir");
    // A unit that doesn't reference claude in ExecStart.
    write_unit(tmp.path(), "sshd.service", "/usr/bin/sshd -D");
    write_unit(
        tmp.path(),
        "claude-build.service",
        "/home/jsy/.local/bin/claude-build-headless.sh",
    );

    let source = SystemdSource::new(tmp.path().to_owned());
    let sites = source.sites().expect("sites");

    assert!(
        !sites.iter().any(|s| s.id == "sshd.service"),
        "sshd.service (no claude in ExecStart) must be excluded"
    );
    assert!(
        sites.iter().any(|s| s.id == "claude-build.service"),
        "claude-build.service must be included"
    );
}
