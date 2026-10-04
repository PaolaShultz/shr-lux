use serde_json::{Value as Json, json};
use shr_lux::{
    authority::{Authority, Command},
    fixture::*,
    wire::*,
};
const SHOW: &str = "11111111-1111-4111-8111-111111111111";
fn engine() -> Authority {
    Authority::new(
        Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![FixtureSpec::synthetic(
                Id::new("fixture-11").unwrap(),
                "Synthetic",
                SyntheticMode::RgbPosition,
                1,
            )],
        })
        .unwrap(),
        SHOW,
        9,
    )
    .unwrap()
}
fn read() -> Json {
    json!({"contract":"C-LIGHT","version":1,"show_id":SHOW,"module":"lighting","epoch":"9","writer":null,"lease":null,"request_id":null,"expected_revision":null,"kind":"snapshot","body":{}})
}
fn grant(writer: &str) -> Json {
    grant_at(writer, 0)
}
fn grant_at(writer: &str, revision: u64) -> Json {
    let mut e = read();
    e["kind"] = json!("grant");
    e["writer"] = json!(writer);
    e["request_id"] = json!("1");
    e["expected_revision"] = json!(revision.to_string());
    e["body"] = json!({"scope":SCOPE});
    e
}
fn command(id: u64, rev: u64, action: Json) -> Json {
    let mut e = read();
    e["kind"] = json!("command");
    e["writer"] = json!("writer-a");
    e["lease"] = json!("1");
    e["request_id"] = json!((id + 1).to_string());
    e["expected_revision"] = json!(rev.to_string());
    e["body"] = json!({"scope":SCOPE,"command":action});
    e
}
fn run(s: &mut Service, e: &Json) -> Json {
    s.handle(&serde_json::to_vec(e).unwrap())[0].clone()
}
fn service() -> Service {
    Service::new(engine()).unwrap()
}
#[test]
fn reads_never_grant_and_actual_commands_produce_corpus() {
    let mut s = service();
    let mut cases = Vec::new();
    let commands = vec![
        read(),
        grant("writer-a"),
        command(
            1,
            0,
            json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":0}]}),
        ),
        command(2, 1, json!({"action":"clear_to_hold"})),
        command(3, 2, json!({"action":"master","level":500})),
        command(4, 3, json!({"action":"blackout","enabled":true})),
        command(5, 0, json!({"action":"master","level":1000})),
        command(4, 3, json!({"action":"blackout","enabled":true})),
        read(),
    ];
    for input in commands {
        let before = s.engine().revision();
        let replies = s.handle(&serde_json::to_vec(&input).unwrap());
        cases.push(json!({"input":input,"prior_revision":before.to_string(),"replies":replies}));
    }
    let corpus = json!({"contract":"C-LIGHT:1","schema":"lx03-wire-v1","cases":cases});
    if std::env::var_os("LUX_GENERATE_CORPUS").is_some() {
        std::fs::create_dir_all("tests/fixtures/lx03/v1").unwrap();
        std::fs::write(
            "tests/fixtures/lx03/v1/commands.json",
            serde_json::to_string_pretty(&corpus).unwrap() + "\n",
        )
        .unwrap();
    } else {
        let path = "tests/fixtures/lx03/v1/commands.json";
        assert_eq!(
            corpus,
            serde_json::from_slice::<Json>(
                &std::fs::read(path).expect("generate real provider corpus first")
            )
            .unwrap()
        );
    }
    let mut ungranted = service();
    run(&mut ungranted, &read());
    assert_eq!(
        run(
            &mut ungranted,
            &command(1, 0, json!({"action":"master","level":1}))
        )["reason"],
        "lease"
    );
}
#[test]
fn strict_failures_preserve_authority() {
    let mut s = service();
    run(&mut s, &grant("writer-a"));
    let before = s.engine().clone();
    for raw in [
        b"{\"a\":1,\"a\":2}".as_slice(),
        b"{} trailing",
        b"{\"a\":0.1}",
        b"\xff",
        b"[[[[[[[[[[[[[0]]]]]]]]]]]]]",
        &vec![b' '; 65537],
    ] {
        assert_eq!(s.handle(raw)[0]["kind"], "rejected");
        assert_eq!(s.engine(), &before);
    }
    for (field, value, reason) in [
        (
            "show_id",
            json!("22222222-2222-4222-8222-222222222222"),
            "wrong_show",
        ),
        ("epoch", json!("8"), "epoch"),
        ("version", json!(2), "version"),
        ("epoch", json!("09"), "range"),
        ("epoch", json!(9), "malformed"),
        ("writer", json!("BAD"), "malformed"),
        ("extra", json!(1), "malformed"),
    ] {
        let mut e = command(1, 0, json!({"action":"master","level":1}));
        e[field] = value;
        assert_eq!(run(&mut s, &e)["reason"], reason);
        assert_eq!(s.engine(), &before);
    }
    let mut wrong = command(1, 0, json!({"action":"master","level":1}));
    wrong["body"]["scope"] = json!("other");
    assert_eq!(run(&mut s, &wrong)["reason"], "scope");
    let mut unknown = command(2, 0, json!({"action":"master","level":1,"unknown":true}));
    assert_eq!(run(&mut s, &unknown)["reason"], "malformed");
    unknown["request_id"] = json!("4");
    unknown["body"]["command"] = json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":1001}]});
    assert_eq!(run(&mut s, &unknown)["reason"], "range");
    assert_eq!(s.engine(), &before);
}
#[test]
fn replay_eviction_highwater_and_first_one() {
    let mut s = service();
    let mut wrong = grant("writer-a");
    wrong["request_id"] = json!("2");
    assert_eq!(run(&mut s, &wrong)["reason"], "reused_id");
    run(&mut s, &grant("writer-a"));
    let first = command(1, 0, json!({"action":"master","level":500}));
    let original = run(&mut s, &first);
    assert_eq!(run(&mut s, &first), original);
    let mut changed = first.clone();
    changed["body"]["command"]["level"] = json!(600);
    assert_eq!(run(&mut s, &changed)["reason"], "reused_id");
    for i in 2..=65 {
        assert_eq!(
            run(
                &mut s,
                &command(i, i - 1, json!({"action":"master","level":500}))
            )["kind"],
            "applied"
        );
    }
    assert_eq!(run(&mut s, &first)["reason"], "expired_id");
    assert_eq!(s.engine().revision(), 65);
    assert_eq!(
        run(
            &mut s,
            &command(67, 65, json!({"action":"master","level":700}))
        )["kind"],
        "applied"
    );
    assert_eq!(
        run(
            &mut s,
            &command(66, 66, json!({"action":"master","level":800}))
        )["reason"],
        "expired_id"
    );
    let retry = command(65, 64, json!({"action":"master","level":500}));
    assert_eq!(run(&mut s, &retry)["revision"], "65");
    assert_eq!(s.engine().revision(), 66);
}
#[test]
fn expiry_history_capacity_preserves_look() {
    let mut s = service();
    run(&mut s, &grant("writer-a"));
    run(
        &mut s,
        &command(
            1,
            0,
            json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":333}]}),
        ),
    );
    let look = s.engine().clone();
    s.advance(200).unwrap();
    assert_eq!(
        run(&mut s, &command(2, 1, json!({"action":"master","level":0})))["reason"],
        "lease"
    );
    assert_eq!(s.engine(), &look);
    assert_eq!(run(&mut s, &grant("writer-a"))["reason"], "lease");
    for i in 1..WRITER_HISTORY {
        assert_eq!(
            run(&mut s, &grant_at(&format!("writer-{i}"), 1))["kind"],
            "applied"
        );
        s.advance((i as u64 + 1) * 200).unwrap();
    }
    assert_eq!(run(&mut s, &grant("writer-last"))["reason"], "capacity");
    assert_eq!(s.engine(), &look);
}
#[test]
fn pages_coherent_deadline_and_maximum_state_atomic_budget() {
    let fixtures = (0..32)
        .map(|i| {
            FixtureSpec::synthetic(
                Id::new(&format!("f{i:02}{}", "x".repeat(60))).unwrap(),
                "Synthetic",
                SyntheticMode::RgbPosition,
                i * 7 + 1,
            )
        })
        .collect::<Vec<_>>();
    let mut e = Authority::new(
        Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures,
        })
        .unwrap(),
        SHOW,
        9,
    )
    .unwrap();
    // Fill actual accumulated masks through legal <=64-target transactions.
    let all = e
        .patch()
        .fixtures()
        .iter()
        .flat_map(|f| {
            f.capabilities.iter().map(|a| Value {
                fixture: f.id.clone(),
                attribute: a.attribute,
                value: a.default,
            })
        })
        .collect::<Vec<_>>();
    for chunk in all.chunks(56) {
        e.execute(Command::Touch(chunk.to_vec())).unwrap();
    }
    let mut s = Service::new(e).unwrap();
    let revision = s.engine().revision();
    run(&mut s, &grant_at("writer-a", revision));
    let mut accepted = 0;
    for i in 0..64 {
        let kind = if i < 32 { "cue" } else { "palette" };
        let input = command(
            i + 1,
            s.engine().revision(),
            json!({"action":"record","kind":kind,"id":format!("s{i:02}{}","x".repeat(60))}),
        );
        let old = s.engine().clone();
        let reply = run(&mut s, &input);
        if reply["kind"] == "applied" {
            accepted += 1;
        } else {
            assert_eq!(reply["reason"], "capacity");
            assert_eq!(s.engine(), &old);
            break;
        }
    }
    assert!(accepted > 0);
    let pages = s.handle(&serde_json::to_vec(&read()).unwrap());
    assert!(pages.len() > 1 && pages.len() <= 16);
    let text = pages
        .iter()
        .map(|p| p["body"]["chunk"].as_str().unwrap())
        .collect::<String>();
    assert_eq!(
        serde_json::from_str::<Json>(&text).unwrap(),
        s.inventory().unwrap()
    );
}

#[test]
fn encoded_refusal_and_reconnect_corpus() {
    let mut service = service();
    let mut cases = Vec::new();
    let mut send = |input: Json, tick: u64| {
        service.advance(tick).unwrap();
        let prior = service.inventory().unwrap();
        let replies = service.handle(&serde_json::to_vec(&input).unwrap());
        cases.push(json!({"input":input,"tick":tick.to_string(),"prior":prior,"replies":replies,"after":service.inventory().unwrap()}));
    };
    send(read(), 0);
    send(grant("writer-a"), 0);
    for (field, value) in [
        ("show_id", json!("22222222-2222-4222-8222-222222222222")),
        ("epoch", json!("8")),
        ("version", json!(2)),
        ("epoch", json!("09")),
        ("writer", json!("BAD")),
        ("unknown", json!(true)),
    ] {
        let mut input = command(1, 0, json!({"action":"master","level":500}));
        input[field] = value;
        send(input, 0);
    }
    let mut wrongscope = command(1, 0, json!({"action":"master","level":500}));
    wrongscope["body"]["scope"] = json!("other");
    send(wrongscope, 0);
    send(
        command(
            2,
            0,
            json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":333}]}),
        ),
        0,
    );
    send(command(3, 0, json!({"action":"clear_to_hold"})), 0);
    send(
        command(4, 0, json!({"action":"blackout","enabled":true})),
        0,
    );
    send(
        command(
            2,
            0,
            json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":333}]}),
        ),
        0,
    );
    send(
        command(
            2,
            0,
            json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":444}]}),
        ),
        0,
    );
    send(command(6, 2, json!({"action":"master","level":700})), 0);
    send(command(5, 3, json!({"action":"master","level":100})), 0);
    send(command(7, 3, json!({"action":"master","level":100})), 200);
    send(grant("writer-a"), 200);
    send(read(), 200);
    send(grant_at("writer-new", 1), 200);
    let mut new = command(1, 1, json!({"action":"master","level":800}));
    new["writer"] = json!("writer-new");
    new["lease"] = json!("2");
    send(new, 200);
    send(read(), 200);
    let corpus = json!({"contract":"C-LIGHT:1","schema":"lx03-wire-v1","cases":cases});
    let path = "tests/fixtures/lx03/v1/refusals.json";
    if std::env::var_os("LUX_GENERATE_CORPUS").is_some() {
        std::fs::write(path, serde_json::to_string_pretty(&corpus).unwrap() + "\n").unwrap();
    } else {
        assert_eq!(
            corpus,
            serde_json::from_slice::<Json>(&std::fs::read(path).unwrap()).unwrap()
        );
    }
}

#[test]
fn lost_grant_ack_retry_stale_and_full_correlation() {
    let mut s = service();
    let request = grant("writer-a");
    let original = run(&mut s, &request);
    assert_eq!(original["body"]["lease"], "1");
    s.advance(50).unwrap();
    assert_eq!(run(&mut s, &request), original);
    let applied = run(
        &mut s,
        &command(1, 0, json!({"action":"master","level":500})),
    );
    for field in ["writer", "lease", "request_id", "expected_revision"] {
        assert_eq!(
            applied[field],
            command(1, 0, json!({"action":"master","level":500}))[field]
        );
    }
    assert_eq!(run(&mut s, &request), original); // before stale-revision test
    let mut changed = request.clone();
    changed["expected_revision"] = json!("1");
    assert_eq!(run(&mut s, &changed)["reason"], "reused_id");
    let refusal = run(
        &mut s,
        &command(2, 0, json!({"action":"master","level":500})),
    );
    assert_eq!(refusal["reason"], "stale_revision");
    assert_eq!(refusal["writer"], "writer-a");
    assert_eq!(refusal["request_id"], "3");
    s.advance(200).unwrap();
    assert_eq!(run(&mut s, &request)["reason"], "lease");
    let stale = grant("writer-stale");
    let denied = run(&mut s, &stale);
    assert_eq!(denied["reason"], "stale_revision");
    assert_eq!(run(&mut s, &stale), denied);
    assert_eq!(
        run(&mut s, &grant_at("writer-stale", 1))["reason"],
        "reused_id"
    );
    assert_eq!(run(&mut s, &grant_at("writer-new", 1))["kind"], "applied");
    assert_eq!(s.engine().revision(), 1);
}
