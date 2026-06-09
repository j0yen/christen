//! AC6 (route): The interactive ShellRc site yields an Advise action whose
//! snippet is a copy-pasteable alias; it is printed, never written.

use christen::route::SystemdSource;
use christen::{apply_route, plan, ChristenConfig, KernelInfo, LaunchSiteSource, RouteAction};
use tempfile::TempDir;

#[test]
fn interactive_site_produces_advise_action() {
    // SystemdSource always appends the interactive ShellRc site.
    let tmp = TempDir::new().expect("tempdir");
    // Scanning an empty directory → only the synthetic interactive site.
    let source = SystemdSource::new(tmp.path().to_owned());
    let raw = source.sites().expect("sites");

    let kernel = KernelInfo {
        agent_ns: true,
        release: "6.9.0-arch1-5-wintermute".to_owned(),
    };
    let config = ChristenConfig::default();
    let route_plan = plan(&raw, &kernel, true, &config);

    let advise = route_plan.actions.iter().find_map(|a| {
        if let RouteAction::Advise { site, snippet } = a {
            if site == "interactive" {
                return Some(snippet.as_str());
            }
        }
        None
    });

    let snippet = advise.expect("interactive site must produce an Advise action");
    assert!(
        snippet.contains("alias claude="),
        "snippet must be an alias definition; got: {snippet}"
    );
    assert!(
        snippet.contains("agentns-claude"),
        "snippet must reference agentns-claude; got: {snippet}"
    );
    assert!(
        snippet.contains("--intent interactive"),
        "snippet must reference --intent interactive; got: {snippet}"
    );
}

#[test]
fn interactive_advise_is_printed_not_written() {
    let tmp = TempDir::new().expect("tempdir");
    let source = SystemdSource::new(tmp.path().to_owned());
    let raw = source.sites().expect("sites");

    let kernel = KernelInfo {
        agent_ns: true,
        release: "6.9.0-arch1-5-wintermute".to_owned(),
    };
    let mut config = ChristenConfig::default();
    config.systemd_dir = tmp.path().to_owned();
    let route_plan = plan(&raw, &kernel, true, &config);

    // In dry-run mode: printed but nothing written.
    let mut out: Vec<u8> = Vec::new();
    apply_route(&route_plan, &config, true, None, &mut out).expect("apply_route");
    let text = String::from_utf8(out).expect("utf8");
    assert!(
        text.contains("alias claude="),
        "dry-run output must contain the alias snippet; got:\n{text}"
    );

    // In apply mode: alias is still printed and no file is written for it.
    let mut out2: Vec<u8> = Vec::new();
    apply_route(&route_plan, &config, false, None, &mut out2).expect("apply_route apply");
    let text2 = String::from_utf8(out2).expect("utf8");
    assert!(
        text2.contains("alias claude="),
        "apply output must print the alias snippet; got:\n{text2}"
    );

    // No .d directory should be created for the interactive site.
    let interactive_d = tmp.path().join("interactive.d");
    assert!(
        !interactive_d.exists(),
        "must not create interactive.d directory"
    );
}
