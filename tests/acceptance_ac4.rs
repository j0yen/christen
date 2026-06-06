//! AC4: plan classifies correctly against FakeSource fixtures.
//!
//! Cases:
//! - Systemd site whose exec line lacks agentns-claude on a -wintermute kernel
//!   with wrapper installed → Unwrapped + Wire (to line contains agentns-claude)
//! - Site whose exec line already contains agentns-claude → Wrapped + AlreadyWrapped
//! - Shell-rc site → Advise
//! - Site on kernel with agent_ns:false → Skip (no agent-ns support)
//! - Site on kernel where wrapper_installed:false → Skip (wrapper absent)

use std::path::PathBuf;

use christen::{plan, ChristenConfig, FakeSource, KernelInfo, LaunchSiteSource, RawSite, RouteAction, SiteKind};

fn wintermute_kernel() -> KernelInfo {
    KernelInfo {
        agent_ns: true,
        release: "6.9.0-arch1-5-wintermute".to_owned(),
    }
}

fn non_wintermute_kernel() -> KernelInfo {
    KernelInfo {
        agent_ns: false,
        release: "6.9.0-arch1-5".to_owned(),
    }
}

fn systemd_site(id: &str, exec: &str) -> RawSite {
    RawSite {
        id: id.to_owned(),
        kind: SiteKind::SystemdUnit {
            unit: id.to_owned(),
            exec_start: exec.to_owned(),
        },
        exec_line: exec.to_owned(),
    }
}

fn shell_rc_site() -> RawSite {
    RawSite {
        id: "interactive".to_owned(),
        kind: SiteKind::ShellRc {
            path: PathBuf::from("/home/jsy/.zshrc"),
        },
        exec_line: "claude".to_owned(),
    }
}

#[test]
fn test_plan_classify_all_cases() {
    let config = ChristenConfig::default();
    let kernel = wintermute_kernel();

    // Case 1: Systemd site, exec lacks agentns-claude → Wire
    let unwrapped = systemd_site("claude-build.service", "/usr/bin/claude-build-headless.sh");
    let source = FakeSource::new(vec![unwrapped.clone()]);
    let raw = source.sites().expect("fake source");
    let rp = plan(&raw, &kernel, true, &config);
    assert_eq!(rp.to_wire, 1, "should wire one unwrapped site");
    match &rp.actions[0] {
        RouteAction::Wire { site, to, .. } => {
            assert_eq!(site, "claude-build.service");
            assert!(
                to.contains("agentns-claude"),
                "Wire.to must contain agentns-claude, got: {to}"
            );
            assert!(
                to.contains("--intent /build"),
                "Wire.to must contain intent, got: {to}"
            );
            assert!(
                to.contains("--budget wall=7200s,fork=2000"),
                "Wire.to must contain budget, got: {to}"
            );
        }
        other => panic!("expected Wire, got {:?}", other),
    }

    // Case 2: exec line already contains agentns-claude → AlreadyWrapped
    let wrapped = systemd_site(
        "claude-dream.service",
        "agentns-claude --intent /dream --budget wall=7200s,fork=2000 -- /usr/bin/claude-dream.sh",
    );
    let source2 = FakeSource::new(vec![wrapped]);
    let raw2 = source2.sites().expect("fake source");
    let rp2 = plan(&raw2, &kernel, true, &config);
    assert_eq!(rp2.already, 1, "should see one already-wrapped site");
    assert!(
        matches!(&rp2.actions[0], RouteAction::AlreadyWrapped { site } if site == "claude-dream.service")
    );

    // Case 3: shell-rc site → Advise
    let rc = shell_rc_site();
    let source3 = FakeSource::new(vec![rc]);
    let raw3 = source3.sites().expect("fake source");
    let rp3 = plan(&raw3, &kernel, true, &config);
    assert_eq!(rp3.advised, 1, "should advise one shell-rc site");
    assert!(matches!(&rp3.actions[0], RouteAction::Advise { .. }));

    // Case 4: non-wintermute kernel → Skip (no agent-ns)
    let nwk = non_wintermute_kernel();
    let source4 = FakeSource::new(vec![unwrapped.clone()]);
    let raw4 = source4.sites().expect("fake source");
    let rp4 = plan(&raw4, &nwk, true, &config);
    assert_eq!(rp4.skipped, 1, "should skip on non-wintermute kernel");
    match &rp4.actions[0] {
        RouteAction::Skip { reason, .. } => {
            assert!(
                reason.contains("kernel"),
                "skip reason should mention kernel, got: {reason}"
            );
        }
        other => panic!("expected Skip, got {:?}", other),
    }

    // Case 5: wrapper not installed → Skip
    let source5 = FakeSource::new(vec![unwrapped]);
    let raw5 = source5.sites().expect("fake source");
    let rp5 = plan(&raw5, &kernel, false, &config);
    assert_eq!(rp5.skipped, 1, "should skip when wrapper not installed");
    match &rp5.actions[0] {
        RouteAction::Skip { reason, .. } => {
            assert!(
                reason.contains("wrapper"),
                "skip reason should mention wrapper, got: {reason}"
            );
        }
        other => panic!("expected Skip, got {:?}", other),
    }
}
