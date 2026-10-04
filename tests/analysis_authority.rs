use serde_json::{Value as Json, json};
use shr_lux::{
    authority::{Authority, Command, Mode, StoredLook},
    fixture::*,
    lighting_contract::{Source, StoredKind},
    wire::Service,
};
fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn value(n: i32) -> Value {
    Value {
        fixture: id("fixture-11"),
        attribute: Attribute::Intensity,
        value: n,
    }
}
fn engine() -> Authority {
    let patch = Patch::validate(PatchSpec {
        version: 1,
        patch_revision: 1,
        fixtures: vec![
            FixtureSpec::synthetic(id("fixture-11"), "Synthetic", SyntheticMode::Rgb, 1),
            FixtureSpec::synthetic(id("fixture-12"), "Other", SyntheticMode::Dimmer, 5),
        ],
    })
    .unwrap();
    let mut e = Authority::new(patch, "11111111-1111-4111-8111-111111111111", 9)
        .unwrap()
        .with_stores(vec![StoredLook {
            kind: StoredKind::Cue,
            id: id("cue-1"),
            values: vec![value(300)],
        }])
        .unwrap();
    e.enable_analysis();
    e
}
fn intensity(e: &Authority) -> shr_lux::lighting_contract::AttributeSnapshot {
    e.snapshot().fixtures[0].attributes[0].clone()
}
#[test]
fn auto_continuity_cap_priority_masked_human_and_mode_freeze() {
    let mut e = engine();
    e.execute(Command::Go {
        cue: id("cue-1"),
        playback: id("playback-1"),
    })
    .unwrap();
    e.execute(Command::Mode(Mode::Auto)).unwrap();
    let a = intensity(&e);
    assert_eq!(a.resolved, 300);
    assert_eq!(a.hold, None);
    assert_eq!(a.source, Source::Auto(id("mode-entry-continuity")));
    let before = e.clone();
    assert!(e.validate_auto_targets(&[id("fixture-11")], 299).is_err());
    assert_eq!(e, before);
    e.step_auto(&[id("fixture-11")], 500, 750).unwrap();
    assert_eq!(intensity(&e).resolved, 320);
    assert_eq!(intensity(&e).source, Source::Auto(id("analysis-active")));
    e.execute(Command::Touch(vec![value(900)])).unwrap();
    e.execute(Command::ClearToHold).unwrap();
    for _ in 0..30 {
        e.step_auto(&[id("fixture-11")], 500, 750).unwrap();
    }
    let a = intensity(&e);
    assert_eq!(a.hold, Some(900));
    assert_eq!(a.resolved, 900);
    assert_eq!(a.auto, Some(500));
    assert_eq!(a.source, Source::Hold);
    e.freeze_auto("loss").unwrap();
    let a = intensity(&e);
    assert!(
        a.contributors
            .iter()
            .any(|c| c.source == Source::Auto(id("analysis-held")) && !c.winner)
    );
    e.execute(Command::Touch(vec![value(0)])).unwrap();
    assert_eq!(intensity(&e).resolved, 0);
    assert_eq!(intensity(&e).source, Source::Programmer);
    e.execute(Command::Mode(Mode::Manual)).unwrap();
    assert_eq!(intensity(&e).hold, Some(0));
    assert_eq!(intensity(&e).auto, None);
}
#[test]
fn reviewed_release_freezes_underlying_auto_until_endpoint() {
    let mut e = engine();
    e.execute(Command::Mode(Mode::Auto)).unwrap();
    for _ in 0..10 {
        e.step_auto(&[id("fixture-11")], 700, 700).unwrap();
    }
    e.execute(Command::Touch(vec![value(0)])).unwrap();
    e.execute(Command::ClearToHold).unwrap();
    let preview = e.preview_release(&[value(0)]).unwrap();
    e.release_commit(&preview).unwrap();
    for tick in 1..50 {
        e.advance(tick).unwrap();
        e.step_auto(&[id("fixture-11")], 700, 700).unwrap();
        assert_eq!(intensity(&e).auto, Some(200));
    }
    assert!(intensity(&e).resolved < 200);
    e.advance(50).unwrap();
    assert_eq!(intensity(&e).resolved, 200);
    assert_eq!(intensity(&e).release, None);
    e.step_auto(&[id("fixture-11")], 700, 700).unwrap();
    assert_eq!(intensity(&e).resolved, 220);
}
#[test]
fn schema_gates_auto_and_provenance_is_per_fixture() {
    let mut e = engine();
    e.execute(Command::Mode(Mode::Auto)).unwrap();
    e.step_auto(&[id("fixture-11")], 300, 300).unwrap();
    let s = e.snapshot();
    assert!(s.validate(e.patch()).is_err());
    s.validate_for_schema(e.patch(), "lx05-v1").unwrap();
    assert_eq!(
        s.fixtures[1].attributes[0].source,
        Source::Auto(id("mode-entry-continuity"))
    );
    let mut malformed = s.clone();
    malformed.fixtures[0].attributes[1].auto = Some(1);
    assert!(malformed.validate_for_schema(e.patch(), "lx05-v1").is_err());
    let mut legacy =
        Authority::new(e.patch().clone(), "11111111-1111-4111-8111-111111111111", 9).unwrap();
    assert!(legacy.execute(Command::Mode(Mode::Auto)).is_err());
}
fn envelope(kind: &str) -> Json {
    json!({"contract":"C-LIGHT","version":1,"show_id":"11111111-1111-4111-8111-111111111111","module":"lighting","epoch":"9","writer":null,"lease":null,"request_id":null,"expected_revision":null,"kind":kind,"body":{}})
}
fn run(s: &mut Service, j: Json) -> Json {
    s.handle(&serde_json::to_vec(&j).unwrap())[0].clone()
}
fn command(s: &mut Service, n: u64, action: Json) -> Json {
    let mut j = envelope("command");
    j["writer"] = json!("writer-a");
    j["lease"] = json!("1");
    j["request_id"] = json!(n.to_string());
    j["expected_revision"] = json!(s.engine().revision().to_string());
    j["body"] = json!({"scope":"lighting-control","command":action});
    run(s, j)
}
fn ready(s: &mut Service) {
    use shr_lux::analysis_subscription::{Descriptor, monotonic_ms};
    let c: Json = serde_json::from_str(include_str!("fixtures/lx05/v1/e09.json")).unwrap();
    let d = Descriptor::decode(&serde_json::to_vec(&c["descriptor"]).unwrap()).unwrap();
    let a = s.analysis_state_mut().unwrap();
    a.attach(d);
    for n in 0..111u64 {
        let now = monotonic_ms().unwrap();
        let mut b = b"GAW1".to_vec();
        b.extend(0u64.to_be_bytes());
        b.extend(1u64.to_be_bytes());
        b.extend(2u64.to_be_bytes());
        b.extend(now.to_be_bytes());
        b.extend([0; 4]);
        for i in 0..10 {
            let hex = c["packets_hex"][i].as_str().unwrap();
            let mut p: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|j| u8::from_str_radix(&hex[j..j + 2], 16).unwrap())
                .collect();
            p[24..32].copy_from_slice(&(48000 + n * 480 + i as u64 * 48).to_be_bytes());
            p[32..36].copy_from_slice(&(n as u32 * 10 + i as u32).to_be_bytes());
            b.extend(p);
        }
        a.ingest(&b, now).unwrap();
        if n == 0 {
            a.calibrate("start", now).unwrap();
        }
        if n == 50 {
            a.calibrate("finish", now).unwrap();
        }
    }
    assert!(a.ready());
}
#[test]
fn actual_wire_grants_expire_loss_revokes_no_rearm_and_manual_continues() {
    let mut s = Service::timed(engine()).unwrap();
    s.enable_analysis().unwrap();
    let before = s.engine().clone();
    let reply = run(&mut s, envelope("analysis_status"));
    assert_eq!(reply["kind"], "analysis_status");
    assert_eq!(s.engine(), &before);
    let mut grant = envelope("grant");
    grant["writer"] = json!("writer-a");
    grant["request_id"] = json!("1");
    grant["expected_revision"] = json!("0");
    grant["body"] = json!({"scope":"lighting-control"});
    assert_eq!(run(&mut s, grant)["kind"], "applied");
    assert_eq!(
        command(&mut s, 2, json!({"action":"mode","mode":"auto"}))["kind"],
        "applied"
    );
    assert_eq!(
        command(
            &mut s,
            3,
            json!({"action":"analysis_grant","fixtures":["fixture-11"],"cap":700,"ttl_ms":50})
        )["reason"],
        "analysis_unavailable"
    );
    ready(&mut s);
    assert_eq!(
        command(
            &mut s,
            4,
            json!({"action":"analysis_grant","fixtures":["fixture-11"],"cap":700,"ttl_ms":50})
        )["kind"],
        "applied"
    );
    assert_eq!(intensity(s.engine()).resolved, 0);
    let now = shr_lux::analysis_subscription::monotonic_ms().unwrap();
    s.poll_analysis(None, now, 1).unwrap();
    assert_eq!(
        intensity(s.engine()).resolved,
        20,
        "{}",
        s.analysis_status()
    );
    s.poll_analysis(None, now, 1).unwrap();
    assert_eq!(
        intensity(s.engine()).resolved,
        20,
        "{}",
        s.analysis_status()
    );
    s.poll_analysis(None, now, 4).unwrap();
    assert_eq!(intensity(s.engine()).resolved, 40);
    s.poll_analysis(None, now, 5).unwrap();
    assert_eq!(intensity(s.engine()).resolved, 40);
    assert_eq!(
        intensity(s.engine()).source,
        Source::Auto(id("analysis-held"))
    );
    assert!(s.analysis_status()["grant"].is_null());
    assert_eq!(
        command(
            &mut s,
            5,
            json!({"action":"analysis_grant","fixtures":["fixture-11"],"cap":700,"ttl_ms":1000})
        )["kind"],
        "applied"
    );
    s.analysis_state_mut().unwrap().absent();
    s.poll_analysis(None, now, 6).unwrap();
    assert_eq!(intensity(s.engine()).resolved, 40);
    assert!(s.analysis_status()["grant"].is_null());
    assert_eq!(
        command(
            &mut s,
            6,
            json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":900}]})
        )["kind"],
        "applied"
    );
    assert_eq!(intensity(s.engine()).resolved, 900);
    ready(&mut s);
    s.poll_analysis(
        None,
        shr_lux::analysis_subscription::monotonic_ms().unwrap(),
        7,
    )
    .unwrap();
    assert!(s.analysis_status()["grant"].is_null());
    assert_eq!(intensity(s.engine()).resolved, 900);
}
#[test]
fn exact_lx05_owner_wire_corpus_preserves_unavailable_and_retained_auto() {
    let mut service = Service::timed(engine()).unwrap();
    service.enable_analysis().unwrap();
    let mut cases = Vec::new();
    let mut g = envelope("grant");
    g["writer"] = json!("writer-a");
    g["request_id"] = json!("1");
    g["expected_revision"] = json!("0");
    g["body"] = json!({"scope":"lighting-control"});
    cases.push(json!({"input":g,"reply":run(&mut service,g.clone()),"inventory":service.inventory().unwrap()}));
    for (n, action) in [
        json!({"action":"mode","mode":"auto"}),
        json!({"action":"analysis_grant","fixtures":["fixture-11"],"cap":500,"ttl_ms":1000}),
        json!({"action":"mode","mode":"assist"}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = command(&mut service, n as u64 + 2, action.clone());
        cases
            .push(json!({"command":action,"reply":reply,"inventory":service.inventory().unwrap()}));
    }
    let status = run(&mut service, envelope("analysis_status"));
    cases.push(json!({"input":envelope("analysis_status"),"reply":status}));
    let corpus = json!({"schema":"lx05-v1","provenance":"Actual Lux Service over Authority; synthetic patch, absent provider, no output. Unavailable grant and mode-entry retained AUTO are real owner results.","cases":cases});
    if std::env::var_os("LUX_GENERATE_LX05_CORPUS").is_some() {
        std::fs::write(
            "tests/fixtures/lx05/v1/commands.json",
            serde_json::to_string_pretty(&corpus).unwrap() + "\n",
        )
        .unwrap();
    } else {
        assert_eq!(
            corpus,
            serde_json::from_str::<Json>(include_str!("fixtures/lx05/v1/commands.json")).unwrap()
        );
    }
}
