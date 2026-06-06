//! Property-based invariant tests for christen.
//! READ-ONLY: the edit-agent must not modify this file.

use christen::{plan, ChristenConfig, KernelInfo, RawSite, RouteAction, SiteKind};

use proptest::prelude::*;

fn arb_exec_line() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-z/][a-z0-9/._-]{0,40}")
        .expect("valid regex")
}

fn arb_raw_site() -> impl Strategy<Value = RawSite> {
    (
        prop::string::string_regex(r"[a-z][a-z0-9-]{0,30}\.service")
            .expect("valid regex"),
        arb_exec_line(),
    )
        .prop_map(|(id, exec)| RawSite {
            id: id.clone(),
            kind: SiteKind::SystemdUnit {
                unit: id,
                exec_start: exec.clone(),
            },
            exec_line: exec,
        })
}

proptest! {
    /// Invariant: tallies must always sum to the number of input sites.
    #[test]
    fn prop_tallies_sum_to_input_count(sites in prop::collection::vec(arb_raw_site(), 0..10)) {
        let config = ChristenConfig::default();
        let kernel = KernelInfo {
            agent_ns: true,
            release: "6.9.0-arch1-5-wintermute".to_owned(),
        };
        let rp = plan(&sites, &kernel, true, &config);
        prop_assert_eq!(
            rp.to_wire + rp.advised + rp.already + rp.skipped,
            sites.len()
        );
    }

    /// Invariant: the number of actions equals the number of input sites.
    #[test]
    fn prop_actions_len_equals_sites_len(sites in prop::collection::vec(arb_raw_site(), 0..10)) {
        let config = ChristenConfig::default();
        let kernel = KernelInfo {
            agent_ns: true,
            release: "6.9.0-arch1-5-wintermute".to_owned(),
        };
        let rp = plan(&sites, &kernel, true, &config);
        prop_assert_eq!(rp.actions.len(), sites.len());
    }

    /// Invariant: when kernel has no agent-ns support, all sites must be skipped.
    #[test]
    fn prop_no_agent_ns_means_all_skipped(sites in prop::collection::vec(arb_raw_site(), 0..10)) {
        let config = ChristenConfig::default();
        let kernel = KernelInfo {
            agent_ns: false,
            release: "6.9.0-generic".to_owned(),
        };
        let rp = plan(&sites, &kernel, true, &config);
        for action in &rp.actions {
            prop_assert!(
                matches!(action, RouteAction::Skip { .. }),
                "non-agent-ns kernel: all actions should be Skip, got {:?}", action
            );
        }
    }
}
