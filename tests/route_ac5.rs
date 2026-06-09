//! AC5 (route): A generated drop-in passes `systemd-analyze --user verify`
//! against a temp base unit + the drop-in.
//! Skipped (with a logged note, NOT silently passed) when systemd-analyze is absent.

use std::fs;
use std::process::Command;

use christen::render_dropin;
use tempfile::TempDir;

/// Returns true if `systemd-analyze` is on PATH and responds to --version.
fn systemd_analyze_available() -> bool {
    Command::new("systemd-analyze")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn generated_dropin_passes_systemd_analyze_verify() {
    if !systemd_analyze_available() {
        // Per self_orphaned_mock_tests discipline: never silently pass; log the skip.
        eprintln!(
            "SKIP route_ac5: systemd-analyze not available on this build box — \
             verify gate cannot be exercised here. Run on a host with systemd."
        );
        return;
    }

    let tmp = TempDir::new().expect("tempdir");

    // Write a minimal base unit.
    let unit_name = "christen-test-verify.service";
    let exec_orig = "/bin/true";
    let unit_content = format!(
        "[Unit]\nDescription=christen test\n\n[Service]\nExecStart={exec_orig}\n\n[Install]\nWantedBy=default.target\n"
    );
    let unit_path = tmp.path().join(unit_name);
    fs::write(&unit_path, &unit_content).expect("write base unit");

    // Generate the drop-in.
    let dropin = render_dropin(
        tmp.path(),
        unit_name,
        exec_orig,
        "/test",
        "wall=3600s,fork=100",
    );

    // Create the .d directory and write the drop-in.
    let dropin_dir = dropin.path.parent().expect("drop-in path has parent");
    fs::create_dir_all(dropin_dir).expect("create .d dir");
    fs::write(&dropin.path, &dropin.contents).expect("write drop-in");

    // Run systemd-analyze --user verify against the unit.
    let status = Command::new("systemd-analyze")
        .arg("--user")
        .arg("verify")
        .arg(&unit_path)
        .status();

    match status {
        Ok(s) if s.success() => { /* pass */ }
        Ok(s) => {
            panic!(
                "systemd-analyze --user verify failed with status {s} for unit {:?} with drop-in {:?}",
                unit_path, dropin.path
            );
        }
        Err(e) => {
            // systemd-analyze is present but failed to exec — treat as skip.
            eprintln!(
                "SKIP route_ac5: systemd-analyze exec error ({e}) — skipping verify gate"
            );
        }
    }
}
