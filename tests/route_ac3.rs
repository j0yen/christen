//! AC3 (route): christen route (no flag) prints the plan + every drop-in's path
//! and contents and writes NOTHING to disk.

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
fn route_dry_run_writes_nothing() {
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

    let mut output: Vec<u8> = Vec::new();
    // dry_run = true → must NOT write any files.
    apply_route(&route_plan, &config, true, None, &mut output)
        .expect("apply_route dry run must not fail");

    // Assert no .d directories were created under the temp systemd dir.
    let d_dirs: Vec<_> = fs::read_dir(tmp.path())
        .expect("read_dir")
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".d"))
        .collect();

    assert!(
        d_dirs.is_empty(),
        "dry run must not create any .d directories, found: {d_dirs:?}"
    );

    // Output must be non-empty (something was printed).
    assert!(
        !output.is_empty(),
        "dry run must print something to the output writer"
    );
}

#[test]
fn route_dry_run_output_contains_dropin_path_hint() {
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

    let mut output: Vec<u8> = Vec::new();
    apply_route(&route_plan, &config, true, None, &mut output).expect("apply_route");

    let text = String::from_utf8(output).expect("utf8");
    // The dry-run output must mention the drop-in file name.
    assert!(
        text.contains("10-christen.conf"),
        "dry run output must mention the drop-in file; got:\n{text}"
    );
}
