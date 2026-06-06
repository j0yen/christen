//! AC2: LaunchSite, SiteKind, WrapState, RouteAction, RoutePlan are public and
//! serde-(de)serializable; a round-trip test covers each.

use std::path::PathBuf;

use christen::{LaunchSite, RouteAction, RoutePlan, SiteKind, WrapState};

fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(
    value: &T,
) {
    let json = serde_json::to_string(value).expect("serialize");
    let back: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(value, &back, "round-trip failed for {:?}", value);
}

#[test]
fn test_serde_roundtrip_all_types() {
    // SiteKind variants
    roundtrip(&SiteKind::SystemdUnit {
        unit: "claude-build.service".to_owned(),
        exec_start: "/usr/bin/foo".to_owned(),
    });
    roundtrip(&SiteKind::ShellRc {
        path: PathBuf::from("/home/jsy/.zshrc"),
    });
    roundtrip(&SiteKind::Hook);
    roundtrip(&SiteKind::Other {
        note: "some other site".to_owned(),
    });

    // WrapState variants
    roundtrip(&WrapState::Unwrapped);
    roundtrip(&WrapState::Wrapped {
        via: "agentns-claude".to_owned(),
    });
    roundtrip(&WrapState::Uncertain);

    // LaunchSite
    roundtrip(&LaunchSite {
        id: "claude-build.service".to_owned(),
        kind: SiteKind::SystemdUnit {
            unit: "claude-build.service".to_owned(),
            exec_start: "/usr/bin/foo".to_owned(),
        },
        wrap: WrapState::Unwrapped,
        intent: "/build".to_owned(),
    });

    // RouteAction variants
    roundtrip(&RouteAction::Wire {
        site: "claude-build.service".to_owned(),
        from: "/usr/bin/foo".to_owned(),
        to: "agentns-claude --intent /build --budget wall=7200s,fork=2000 -- /usr/bin/foo"
            .to_owned(),
    });
    roundtrip(&RouteAction::Advise {
        site: "interactive".to_owned(),
        snippet: "alias claude='agentns-claude --intent interactive -- claude'".to_owned(),
    });
    roundtrip(&RouteAction::AlreadyWrapped {
        site: "claude-dream.service".to_owned(),
    });
    roundtrip(&RouteAction::Skip {
        site: "claude-build.service".to_owned(),
        reason: "not a -wintermute kernel".to_owned(),
    });

    // RoutePlan
    roundtrip(&RoutePlan {
        actions: vec![RouteAction::AlreadyWrapped {
            site: "x".to_owned(),
        }],
        to_wire: 0,
        advised: 0,
        already: 1,
        skipped: 0,
    });
}
