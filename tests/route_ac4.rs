//! AC4 (route): christen route --apply writes 10-christen.conf under each unit's
//! .d directory with the rendered contents; prints (does not execute) the
//! daemon-reload + restart lines; re-running --apply is idempotent.

use std::fs;

use christen::route::SystemdSource;
use christen::{apply_route, plan, ChristenConfig, KernelInfo, LaunchSiteSource};
use tempfile::TempDir;

fn write_unit(dir: &std::path::Path, name: &str, exec_start: &str) {
    let content = format!(
        "[Unit]\nDescription=Test\n\n[Service]\nExecStart={exec_start}\n\n[Install]\nWantedBy=default.target\n"
    );
    fs::write(dir.join(name), content).expect("write unit");
}

#[test]
fn apply_writes_dropin_file() {
    let tmp = TempDir::new().expect("tempdir");
    let exec = "/home/jsy/.local/bin/claude-build-headless.sh";
    write_unit(tmp.path(), "claude-build.service", exec);

    let source = SystemdSource::new(tmp.path().to_owned());
    let raw = source.sites().expect("sites");

    let kernel = KernelInfo {
        agent_ns: true,
        release: "6.9.0-arch1-5-wintermute".to_owned(),
    };
    let mut config = ChristenConfig::default();
    config.systemd_dir = tmp.path().to_owned();
    let route_plan = plan(&raw, &kernel, true, &config);

    let mut out: Vec<u8> = Vec::new();
    apply_route(&route_plan, &config, false, None, &mut out).expect("apply_route");

    let dropin_path = tmp
        .path()
        .join("claude-build.service.d/10-christen.conf");
    assert!(
        dropin_path.exists(),
        "drop-in file must exist after --apply: {dropin_path:?}"
    );

    let content = fs::read_to_string(&dropin_path).expect("read dropin");
    assert!(content.contains("[Service]"), "must have [Service]");
    assert!(content.contains("ExecStart=\n"), "must have clear line");
    assert!(
        content.contains("agentns-claude"),
        "must reference agentns-claude"
    );
}

#[test]
fn apply_prints_daemon_reload_line() {
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

    let mut out: Vec<u8> = Vec::new();
    apply_route(&route_plan, &config, false, None, &mut out).expect("apply_route");

    let text = String::from_utf8(out).expect("utf8");
    assert!(
        text.contains("daemon-reload"),
        "apply output must print the daemon-reload line; got:\n{text}"
    );
    assert!(
        text.contains("christen cap"),
        "apply output must remind about christen cap; got:\n{text}"
    );
}

#[test]
fn apply_is_idempotent() {
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

    // First apply.
    apply_route(&route_plan, &config, false, None, &mut Vec::new()).expect("first apply");
    let dropin_path = tmp
        .path()
        .join("claude-build.service.d/10-christen.conf");
    let first_bytes = fs::read(&dropin_path).expect("read after first apply");

    // Second apply.
    apply_route(&route_plan, &config, false, None, &mut Vec::new()).expect("second apply");
    let second_bytes = fs::read(&dropin_path).expect("read after second apply");

    assert_eq!(
        first_bytes, second_bytes,
        "re-applying must produce identical file contents (idempotent)"
    );
}
