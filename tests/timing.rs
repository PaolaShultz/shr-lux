use serde_json::{Value as Json, json};
use shr_lux::{
    authority::*,
    fixture::*,
    wire::{SCOPE, Service},
};
const SHOW: &str = "11111111-1111-4111-8111-111111111111";
fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn value(f: &str, v: i32) -> Value {
    Value {
        fixture: id(f),
        attribute: Attribute::Intensity,
        value: v,
    }
}
fn held() -> Authority {
    let patch = Patch::validate(PatchSpec {
        version: 1,
        patch_revision: 1,
        fixtures: vec![
            FixtureSpec::synthetic(id("f1"), "Synthetic", SyntheticMode::Dimmer, 1),
            FixtureSpec::synthetic(id("f2"), "Synthetic", SyntheticMode::Dimmer, 2),
        ],
    })
    .unwrap();
    let mut e = Authority::new(patch, SHOW, 9)
        .unwrap()
        .with_stores(vec![StoredLook {
            kind: shr_lux::lighting_contract::StoredKind::Cue,
            id: id("cue"),
            values: vec![value("f1", 500), value("f2", 200)],
        }])
        .unwrap();
    e.execute(Command::Go {
        cue: id("cue"),
        playback: id("playback"),
    })
    .unwrap();
    e.execute(Command::Touch(vec![value("f1", 700), value("f2", 800)]))
        .unwrap();
    e.execute(Command::ClearToHold).unwrap();
    e
}
fn current(e: &Authority) -> i32 {
    e.snapshot().fixtures[0].attributes[0].resolved
}
#[test]
fn e05_midpoint_end_late_and_nearest_rounding() {
    let mut e = held();
    let token = e.preview_release(&[value("f1", 0)]).unwrap();
    assert_eq!(token.values()[0].current, 700);
    assert_eq!(token.values()[0].destination, 500);
    e.release_commit(&token).unwrap();
    assert_eq!(current(&e), 700);
    let revision = e.revision();
    e.advance(25).unwrap();
    assert_eq!(current(&e), 600);
    assert_eq!(e.revision(), revision + 1);
    assert_eq!(e.transition_status()["targets"][0]["current"], 600);
    e.advance(50).unwrap();
    assert_eq!(current(&e), 500);
    assert_eq!(e.snapshot().fixtures[0].attributes[0].hold, None);
    assert!(e.transition_status().is_null());
    let mut late = held();
    let t = late.preview_release(&[value("f1", 0)]).unwrap();
    late.release_commit(&t).unwrap();
    let rev = late.revision();
    late.advance(500).unwrap();
    assert_eq!(current(&late), 500);
    assert_eq!(late.revision(), rev + 1);
    let mut round = held();
    round
        .execute(Command::Touch(vec![value("f1", 501)]))
        .unwrap();
    round.execute(Command::ClearToHold).unwrap();
    let t = round.preview_release(&[value("f1", 0)]).unwrap();
    round.release_commit(&t).unwrap();
    round.advance(25).unwrap();
    assert_eq!(current(&round), 501);
    round.advance(50).unwrap();
    assert_eq!(current(&round), 500);
}
#[test]
fn stale_expired_partial_failure_and_manual_scoped_takeover() {
    let mut e = held();
    let before = e.clone();
    assert!(
        e.preview_release(&[value("f1", 0), value("missing", 0)])
            .is_err()
    );
    assert_eq!(e, before);
    let t = e.preview_release(&[value("f1", 0)]).unwrap();
    e.execute(Command::Master(500)).unwrap();
    let before = e.clone();
    assert!(e.release_commit(&t).is_err());
    assert_eq!(e, before);
    let mut e = held();
    let t = e.preview_release(&[value("f1", 0)]).unwrap();
    e.advance(200).unwrap();
    let before = e.clone();
    assert!(e.release_commit(&t).is_err());
    assert_eq!(e, before);
    assert_eq!(current(&e), 700);
    let mut e = held();
    let t = e
        .preview_release(&[value("f1", 0), value("f2", 0)])
        .unwrap();
    e.release_commit(&t).unwrap();
    e.advance(25).unwrap();
    assert_eq!(current(&e), 600);
    e.execute(Command::Touch(vec![value("f1", 333)])).unwrap();
    e.advance(50).unwrap();
    assert_eq!(current(&e), 333);
    assert_eq!(e.snapshot().fixtures[1].attributes[0].resolved, 200);
    e.execute(Command::ClearToHold).unwrap();
    assert_eq!(current(&e), 333);
}
fn envelope(
    writer: &str,
    lease: Option<&str>,
    id: u64,
    revision: u64,
    kind: &str,
    body: Json,
) -> Json {
    json!({"contract":"C-LIGHT","version":1,"show_id":SHOW,"module":"lighting","epoch":"9","writer":writer,"lease":lease,"request_id":id.to_string(),"expected_revision":revision.to_string(),"kind":kind,"body":body})
}
fn send(s: &mut Service, id: u64, action: Json) -> Json {
    let input = envelope(
        "writer-a",
        Some("1"),
        id,
        s.engine().revision(),
        "command",
        json!({"scope":SCOPE,"command":action}),
    );
    s.handle(&serde_json::to_vec(&input).unwrap())[0].clone()
}
fn service() -> Service {
    let mut s = Service::timed(held()).unwrap();
    let grant = envelope(
        "writer-a",
        None,
        1,
        s.engine().revision(),
        "grant",
        json!({"scope":SCOPE}),
    );
    assert_eq!(
        s.handle(&serde_json::to_vec(&grant).unwrap())[0]["kind"],
        "applied"
    );
    s
}
#[test]
fn preview_identity_cancel_expiry_conflict_and_lease_loss_continues_applied() {
    let mut s = service();
    let response = send(
        &mut s,
        2,
        json!({"action":"release_preview","values":[value("f1",0)]}),
    );
    let token = response["body"]["token"].clone();
    assert_eq!(response["body"]["application"], "preview_only");
    let before = s.engine().clone();
    let mut fake = token.clone();
    fake["preview"]["values"][0]["destination"] = json!(999);
    assert_eq!(
        send(&mut s, 3, json!({"action":"release_commit","token":fake}))["reason"],
        "lease"
    );
    assert_eq!(s.engine(), &before);
    send(&mut s, 4, json!({"action":"release_cancel"}));
    assert_eq!(current(s.engine()), 700);
    assert_eq!(
        send(&mut s, 5, json!({"action":"release_commit","token":token}))["reason"],
        "target"
    );
    let response = send(
        &mut s,
        6,
        json!({"action":"release_preview","values":[value("f1",0)]}),
    );
    let token = response["body"]["token"].clone();
    s.advance(199).unwrap();
    assert_eq!(
        send(&mut s, 7, json!({"action":"release_commit","token":token}))["kind"],
        "applied"
    );
    s.advance(224).unwrap();
    assert_eq!(current(s.engine()), 600);
    s.advance(249).unwrap();
    assert_eq!(current(s.engine()), 500); // lease expired200, transition continued
    let mut s = service();
    let response = send(
        &mut s,
        2,
        json!({"action":"release_preview","values":[value("f1",0)]}),
    );
    let token = response["body"]["token"].clone();
    send(&mut s, 3, json!({"action":"master","level":700}));
    assert_eq!(
        send(&mut s, 4, json!({"action":"release_commit","token":token}))["reason"],
        "target"
    );
    assert_eq!(current(s.engine()), 700);
    let mut s = service();
    send(
        &mut s,
        2,
        json!({"action":"release_preview","values":[value("f1",0)]}),
    );
    s.advance(200).unwrap();
    assert_eq!(
        s.inventory().unwrap()["release"]["preview_available"],
        false
    );
    assert_eq!(current(s.engine()), 700);
}
#[test]
fn canonical_token_wrong_lease_and_cached_commit_no_rollback() {
    let mut s = service();
    let response = send(
        &mut s,
        2,
        json!({"action":"release_preview","values":[value("f1",0)]}),
    );
    let token = response["body"]["token"].clone();
    let revision = s.engine().revision();
    let request = envelope(
        "writer-a",
        Some("1"),
        3,
        revision,
        "command",
        json!({"scope":SCOPE,"command":{"action":"release_commit","token":token}}),
    );
    let bytes = serde_json::to_vec(&request).unwrap();
    let original = s.handle(&bytes)[0].clone();
    s.advance(25).unwrap();
    let current_revision = s.engine().revision();
    assert_eq!(s.handle(&bytes)[0], original);
    assert_eq!(s.engine().revision(), current_revision);
    assert_eq!(current(s.engine()), 600);
    let mut wrong = request.clone();
    wrong["lease"] = json!("2");
    assert_eq!(
        s.handle(&serde_json::to_vec(&wrong).unwrap())[0]["reason"],
        "lease"
    );
    assert_eq!(current(s.engine()), 600);
}

#[test]
fn normative_e05_zero_hold_to_seven_hundred_distinct_release_owner() {
    let mut e = held();
    e.execute(Command::Touch(vec![value("f1", 0)])).unwrap();
    e.execute(Command::ClearToHold).unwrap();
    // Real engine store+GO with programmer masks copied, then restore zeroHold.
    e.execute(Command::Touch(vec![value("f1", 700)])).unwrap();
    e.execute(Command::Update {
        kind: shr_lux::lighting_contract::StoredKind::Cue,
        id: id("cue"),
    })
    .unwrap();
    e.execute(Command::Go {
        cue: id("cue"),
        playback: id("playback"),
    })
    .unwrap();
    e.execute(Command::Touch(vec![value("f1", 0)])).unwrap();
    e.execute(Command::ClearToHold).unwrap();
    e.execute(Command::Touch(vec![value("f2", 700)])).unwrap(); // unrelated programmer remains
    let token = e.preview_release(&[value("f1", 0)]).unwrap();
    assert_eq!(token.values()[0].destination, 700);
    e.release_commit(&token).unwrap();
    let start = e.snapshot();
    assert_eq!(start.fixtures[0].attributes[0].hold, None);
    assert_eq!(start.fixtures[0].attributes[0].release, Some(0));
    assert_eq!(
        start.fixtures[0].attributes[0].source,
        shr_lux::lighting_contract::Source::Release
    );
    assert_eq!(start.fixtures[1].attributes[0].programmer, Some(700));
    e.advance(25).unwrap();
    assert_eq!(current(&e), 350);
    assert_eq!(e.snapshot().fixtures[0].attributes[0].hold, None);
    e.advance(50).unwrap();
    assert_eq!(current(&e), 700);
    assert_eq!(e.snapshot().fixtures[0].attributes[0].release, None);
}

#[test]
fn lx04_encoded_provider_corpus() {
    let mut s = Service::timed(held()).unwrap();
    let mut cases = Vec::new();
    let mut transact = |input: Json, tick: u64| {
        s.advance(tick).unwrap();
        let prior = s.inventory().unwrap();
        let replies = s.handle(&serde_json::to_vec(&input).unwrap());
        cases.push(json!({"input":input,"tick":tick.to_string(),"prior":prior,"replies":replies,"after":s.inventory().unwrap()}));
        replies[0].clone()
    };
    let revision = 3;
    transact(
        envelope(
            "writer-a",
            None,
            1,
            revision,
            "grant",
            json!({"scope":SCOPE}),
        ),
        0,
    );
    let preview = transact(
        envelope(
            "writer-a",
            Some("1"),
            2,
            revision,
            "command",
            json!({"scope":SCOPE,"command":{"action":"release_preview","values":[value("f1",0)]}}),
        ),
        0,
    );
    let commit = envelope(
        "writer-a",
        Some("1"),
        3,
        revision,
        "command",
        json!({"scope":SCOPE,"command":{"action":"release_commit","token":preview["body"]["token"]}}),
    );
    transact(commit.clone(), 0);
    let read = json!({"contract":"C-LIGHT","version":1,"show_id":SHOW,"module":"lighting","epoch":"9","writer":null,"lease":null,"request_id":null,"expected_revision":null,"kind":"snapshot","body":{}});
    transact(read.clone(), 25);
    transact(commit, 25);
    transact(read.clone(), 50);
    transact(read, 200);
    let corpus = json!({"contract":"C-LIGHT:1","wire_schema":"lx04-v1","cases":cases});
    let path = "tests/fixtures/lx04/v1/timing.json";
    fn normalise(v: &mut Json) {
        match v {
            Json::Object(m) => {
                for (k, v) in m {
                    if k == "nonce" {
                        *v = json!("engine-issued-nonce");
                    } else {
                        normalise(v);
                    }
                }
            }
            Json::Array(a) => {
                for v in a {
                    normalise(v)
                }
            }
            _ => {}
        }
    }
    if std::env::var_os("LUX_GENERATE_LX04").is_some() {
        std::fs::write(path, serde_json::to_string_pretty(&corpus).unwrap() + "\n").unwrap();
    } else {
        let mut expected: Json = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut actual = corpus;
        normalise(&mut expected);
        normalise(&mut actual);
        assert_eq!(actual, expected);
    }
}
