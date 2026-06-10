//! AC5: christen probe --format json against a FakeReader emits the documented
//! schema including docket_op; a Live fixture and an Init fixture each covered.

use christen::probe::{
    classify, verdict, FakeNsReader, InitReason, NsReading, NsState, ProbeOutput, ProcReader,
    INIT_AGENT_INODE,
};

fn make_live_reading() -> NsReading {
    NsReading {
        ns_inode: Some(4_026_531_997),
        session_hex: Some("deadbeefcafebabe0123456789abcdef".to_owned()),
        counters: None,
        kernel_is_wintermute: true,
        wrapper_installed: true,
    }
}

fn make_init_reading() -> NsReading {
    NsReading {
        ns_inode: Some(INIT_AGENT_INODE),
        session_hex: Some("00000000000000000000000000000000".to_owned()),
        counters: None,
        kernel_is_wintermute: true,
        wrapper_installed: true,
    }
}

#[test]
fn live_fixture_json_schema() {
    let reader = FakeNsReader::new(make_live_reading());
    let reading = reader.read(None).expect("fake reader ok");
    let state = classify(&reading);
    let v = verdict(&state, reading.kernel_is_wintermute);
    let output = ProbeOutput::from_state_verdict(&state, &v);

    let json = serde_json::to_string_pretty(&output).expect("serialize ok");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse ok");

    // Top-level keys.
    assert!(parsed.get("state").is_some(), "missing 'state' key");
    assert!(parsed.get("ok").is_some(), "missing 'ok' key");
    assert!(parsed.get("prose").is_some(), "missing 'prose' key");
    assert!(parsed.get("docket_op").is_some(), "missing 'docket_op' key");

    // Live state should include session_hex.
    assert!(
        parsed.get("session_hex").is_some(),
        "Live state should include 'session_hex' in JSON"
    );
    assert_eq!(
        parsed["session_hex"].as_str(),
        Some("deadbeefcafebabe0123456789abcdef")
    );

    // ok should be true for Live.
    assert_eq!(parsed["ok"].as_bool(), Some(true));

    // docket_op for Live should be resolve.
    assert_eq!(parsed["docket_op"]["op"].as_str(), Some("resolve"));
    assert_eq!(
        parsed["docket_op"]["id"].as_str(),
        Some("agentns-session-zeros")
    );

    // Prose must not contain the banned phrase.
    let prose = parsed["prose"].as_str().unwrap_or("");
    assert!(!prose.contains("registration failed"));
}

#[test]
fn init_fixture_json_schema() {
    let reader = FakeNsReader::new(make_init_reading());
    let reading = reader.read(None).expect("fake reader ok");
    let state = classify(&reading);
    let v = verdict(&state, reading.kernel_is_wintermute);
    let output = ProbeOutput::from_state_verdict(&state, &v);

    let json = serde_json::to_string_pretty(&output).expect("serialize ok");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse ok");

    // Top-level keys present.
    assert!(parsed.get("state").is_some());
    assert!(parsed.get("ok").is_some());
    assert!(parsed.get("prose").is_some());
    assert!(parsed.get("docket_op").is_some());

    // Init state: session_hex should not be present (it's skip_serializing_if None).
    // (Init states don't have a session_hex in the output.)
    assert!(
        parsed.get("session_hex").is_none(),
        "Init state should not include 'session_hex' in JSON"
    );

    // ok should be true for Init{UnwrappedExpected}.
    assert_eq!(parsed["ok"].as_bool(), Some(true));

    // docket_op for Init/wintermute should be report.
    assert_eq!(parsed["docket_op"]["op"].as_str(), Some("report"));

    let title = parsed["docket_op"]["title"].as_str().unwrap_or("");
    assert!(!title.contains("registration failed"), "title must not contain banned phrase");

    let prose = parsed["prose"].as_str().unwrap_or("");
    assert!(!prose.contains("registration failed"), "prose must not contain banned phrase");
}

#[test]
fn json_state_tag_values() {
    // Verify the serde tag values are as documented.
    let absent = christen::probe::NsState::Absent;
    let v = serde_json::to_value(&absent).expect("serialize");
    assert_eq!(v["state"].as_str(), Some("absent"));

    let live = NsState::Live {
        session_hex: "aabb".to_owned(),
        intent: None,
    };
    let v = serde_json::to_value(&live).expect("serialize");
    assert_eq!(v["state"].as_str(), Some("live"));

    let init = NsState::Init {
        reason: InitReason::UnwrappedExpected,
    };
    let v = serde_json::to_value(&init).expect("serialize");
    assert_eq!(v["state"].as_str(), Some("init"));
    assert_eq!(v["reason"].as_str(), Some("unwrapped_expected"));
}
