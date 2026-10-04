use shr_lux::authority::*;
use shr_lux::fixture::*;
use shr_lux::lighting_contract::*;
fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn v(a: Attribute, value: i32) -> Value {
    Value {
        fixture: id("fixture-11"),
        attribute: a,
        value,
    }
}
fn spec() -> PatchSpec {
    PatchSpec {
        version: VERSION,
        patch_revision: 1,
        fixtures: vec![FixtureSpec::synthetic(
            id("fixture-11"),
            "Synthetic",
            SyntheticMode::RgbPosition,
            1,
        )],
    }
}
fn engine() -> Authority {
    Authority::new(
        Patch::validate(spec()).unwrap(),
        "11111111-1111-4111-8111-111111111111",
        9,
    )
    .unwrap()
}
fn attr(s: &Snapshot, a: Attribute) -> &AttributeSnapshot {
    s.fixtures[0]
        .attributes
        .iter()
        .find(|v| v.attribute == a)
        .unwrap()
}
fn stored(name: &str, values: Vec<Value>) -> StoredLook {
    StoredLook {
        kind: StoredKind::Cue,
        id: id(name),
        values,
    }
}
fn rgb(r: i32, g: i32, b: i32) -> Vec<Value> {
    vec![
        v(Attribute::Red, r),
        v(Attribute::Green, g),
        v(Attribute::Blue, b),
    ]
}
fn seeded() -> Authority {
    let mut first = vec![
        v(Attribute::Intensity, 700),
        v(Attribute::Pan, 100),
        v(Attribute::Tilt, -100),
        v(Attribute::Zoom, 50),
    ];
    first.extend(rgb(200, 300, 400));
    engine()
        .with_stores(vec![
            stored("cue-1", first),
            stored("cue-2", vec![v(Attribute::Intensity, 500)]),
        ])
        .unwrap()
}
fn go(e: &mut Authority, cue: &str, playback: &str) {
    e.execute(Command::Go {
        cue: id(cue),
        playback: id(playback),
    })
    .unwrap();
}
fn htp() -> Authority {
    let mut e = seeded();
    go(&mut e, "cue-1", "playback-1");
    go(&mut e, "cue-2", "playback-2");
    e
}
#[test]
fn e04_executes_owner_commands_from_named_prior_states() {
    let mut states = std::collections::BTreeMap::new();
    states.insert("fixture_defaults".to_owned(), seeded());
    let corpus = include_str!("fixtures/c-light-v1/e04.tsv");
    let header: Vec<_> = corpus.lines().next().unwrap().split('\t').collect();
    for row in corpus.lines().skip(1) {
        let fields: Vec<_> = row.split('\t').collect();
        let get = |name: &str| fields[header.iter().position(|h| *h == name).unwrap()];
        let int = |name: &str| get(name).parse::<i32>().unwrap();
        let mut e = states[get("prior_state")].clone();
        match get("command") {
            "go_playback_1_then_2" => {
                go(&mut e, "cue-1", "playback-1");
                go(&mut e, "cue-2", "playback-2");
            }
            "touch_programmer_intensity_0" => {
                e.execute(Command::Touch(vec![v(Attribute::Intensity, 0)]))
                    .unwrap();
            }
            "clear_to_hold" => {
                e.execute(Command::ClearToHold).unwrap();
            }
            "set_master_500" => {
                e.execute(Command::Master(500)).unwrap();
            }
            "set_blackout_true" => {
                e.execute(Command::Blackout(true)).unwrap();
            }
            x => panic!("unknown corpus command {x}"),
        }
        let s = e.snapshot();
        s.validate(e.patch()).unwrap();
        let a = attr(&s, Attribute::Intensity);
        for (actual, name) in [(a.resolved, "resolved"), (a.final_intent, "final_intent")] {
            assert_eq!(actual, int(name), "{} {name}", get("case"));
        }
        for (actual, name) in [(a.programmer, "programmer"), (a.hold, "hold")] {
            assert_eq!(
                actual,
                if get(name) == "null" {
                    None
                } else {
                    Some(int(name))
                }
            );
        }
        assert_eq!(
            a.source,
            match get("source") {
                "programmer" => Source::Programmer,
                "hold" => Source::Hold,
                "playback-1" => Source::Playback(id("playback-1")),
                x => panic!("{x}"),
            }
        );
        for (n, name, winner) in [
            ("playback-1", "playback_1", "winner_1"),
            ("playback-2", "playback_2", "winner_2"),
        ] {
            let c = a
                .contributors
                .iter()
                .find(|c| c.source == Source::Playback(id(n)))
                .unwrap();
            assert_eq!(c.value, int(name));
            assert_eq!(c.winner, get(winner) == "true");
        }
        for (a, name) in [
            (Attribute::Red, "color_red"),
            (Attribute::Green, "color_green"),
            (Attribute::Blue, "color_blue"),
            (Attribute::Pan, "pan"),
            (Attribute::Tilt, "tilt"),
            (Attribute::Zoom, "zoom"),
        ] {
            let a = attr(&s, a);
            assert_eq!(a.resolved, int(name));
            assert_eq!(a.final_intent, int(name));
        }
        assert_eq!(e.master(), int("master"));
        assert_eq!(e.blackout(), get("blackout") == "true");
        assert_eq!(
            a.inhibit,
            if e.blackout() {
                Inhibit::Blackout
            } else {
                Inhibit::None
            }
        );
        assert!(
            s.fixtures
                .iter()
                .flat_map(|f| &f.attributes)
                .all(|a| a.submitted.is_none() && a.observed.is_none())
        );
        states.insert(get("case").to_owned(), e);
    }
    assert_eq!(states.len(), 6);
}
#[test]
fn invalid_transactions_and_patch_changes_are_atomic() {
    let mut e = htp();
    let before = e.clone();
    for values in [
        vec![v(Attribute::Intensity, 0), v(Attribute::Red, 100)],
        vec![v(Attribute::Intensity, 0), v(Attribute::Zoom, 451)],
    ] {
        assert!(e.execute(Command::Touch(values)).is_err());
        assert_eq!(e, before);
    }
    for cmd in [
        Command::Master(-1),
        Command::FixtureMaster {
            fixture: id("missing"),
            level: 500,
        },
        Command::Level {
            playback: id("playback-1"),
            level: 1001,
        },
        Command::Off(id("missing")),
    ] {
        assert!(e.execute(cmd).is_err());
        assert_eq!(e, before);
    }
    let mut bad = spec();
    bad.patch_revision = 2;
    bad.fixtures.clear();
    assert_eq!(e.execute(Command::ReplacePatch(bad)), Err(Error::Target));
    assert_eq!(e, before);
    let mut bad = spec();
    bad.patch_revision = 2;
    bad.fixtures[0].address = 510;
    assert_eq!(e.execute(Command::ReplacePatch(bad)), Err(Error::Address));
    assert_eq!(e, before);
    let mut good = spec();
    good.patch_revision = 2;
    good.fixtures[0].address = 50;
    e.execute(Command::ReplacePatch(good)).unwrap();
    assert_eq!(e.patch().revision(), 2);
    assert_eq!(attr(&e.snapshot(), Attribute::Intensity).resolved, 700);
}
#[test]
fn htp_ties_removal_and_activation_order_are_owned_by_engine() {
    let mut e = engine()
        .with_stores(vec![
            stored("a", {
                let mut x = rgb(100, 200, 300);
                x.push(v(Attribute::Intensity, 700));
                x
            }),
            stored("b", {
                let mut x = rgb(400, 500, 600);
                x.push(v(Attribute::Intensity, 700));
                x
            }),
        ])
        .unwrap();
    go(&mut e, "a", "z-first");
    go(&mut e, "b", "a-second");
    let s = e.snapshot();
    let a = attr(&s, Attribute::Intensity);
    assert_eq!(a.resolved, 700);
    assert_eq!(a.contributors.iter().filter(|c| c.winner).count(), 2);
    for a in [Attribute::Red, Attribute::Green, Attribute::Blue] {
        assert_eq!(attr(&s, a).source, Source::Playback(id("a-second")));
    }
    e.execute(Command::Level {
        playback: id("a-second"),
        level: 0,
    })
    .unwrap();
    let s = e.snapshot();
    assert_eq!(attr(&s, Attribute::Red).resolved, 400);
    assert_eq!(attr(&s, Attribute::Intensity).resolved, 700);
    e.execute(Command::Off(id("z-first"))).unwrap();
    assert_eq!(attr(&e.snapshot(), Attribute::Intensity).resolved, 0);
    e.execute(Command::Off(id("a-second"))).unwrap();
    assert_eq!(
        attr(&e.snapshot(), Attribute::Red).source,
        Source::FixtureDefault
    );
    let mut e = htp();
    e.execute(Command::Off(id("playback-1"))).unwrap();
    assert_eq!(attr(&e.snapshot(), Attribute::Intensity).resolved, 500);
}
#[test]
fn record_update_and_palette_are_copies_with_explicit_masks() {
    let mut e = engine();
    e.execute(Command::Touch(vec![v(Attribute::Intensity, 123)]))
        .unwrap();
    e.execute(Command::ClearToHold).unwrap();
    e.execute(Command::Touch(rgb(10, 20, 30))).unwrap();
    e.execute(Command::Record {
        kind: StoredKind::Cue,
        id: id("cue"),
    })
    .unwrap();
    e.execute(Command::Record {
        kind: StoredKind::Palette,
        id: id("palette"),
    })
    .unwrap();
    go(&mut e, "cue", "p");
    let s = e.snapshot();
    assert!(attr(&s, Attribute::Intensity).stored.is_empty());
    assert!(attr(&s, Attribute::Intensity).playing.is_empty());
    assert_eq!(attr(&s, Attribute::Intensity).hold, Some(123));
    e.execute(Command::Touch(rgb(40, 50, 60))).unwrap();
    e.execute(Command::Update {
        kind: StoredKind::Cue,
        id: id("cue"),
    })
    .unwrap();
    assert_eq!(attr(&e.snapshot(), Attribute::Red).playing[0].value, 10);
    go(&mut e, "cue", "p");
    assert_eq!(attr(&e.snapshot(), Attribute::Red).playing[0].value, 40);
    e.execute(Command::ApplyPalette(id("palette"))).unwrap();
    assert_eq!(attr(&e.snapshot(), Attribute::Red).programmer, Some(10));
    e.execute(Command::Update {
        kind: StoredKind::Palette,
        id: id("palette"),
    })
    .unwrap();
    e.execute(Command::Touch(rgb(70, 80, 90))).unwrap();
    assert_eq!(
        attr(&e.snapshot(), Attribute::Red)
            .stored
            .iter()
            .find(|v| v.kind == StoredKind::Palette)
            .unwrap()
            .value,
        10
    );
}
#[test]
fn manual_zero_clear_mode_masters_blackout_and_null_status() {
    let mut e = htp();
    e.execute(Command::Touch(vec![v(Attribute::Intensity, 0)]))
        .unwrap();
    let before = e.snapshot();
    e.execute(Command::ClearToHold).unwrap();
    let after = e.snapshot();
    assert_eq!(
        attr(&before, Attribute::Intensity).resolved,
        attr(&after, Attribute::Intensity).resolved
    );
    assert_eq!(attr(&after, Attribute::Intensity).programmer, None);
    assert_eq!(attr(&after, Attribute::Intensity).hold, Some(0));
    let token = e.preview_release(&[v(Attribute::Intensity, 0)]).unwrap();
    assert_eq!(token.values()[0].destination, 700);
    assert_eq!(token.transition_ms(), 500);
    assert_eq!(token.validity_ms(), 2000);
    assert!(e.preview_is_current(&token));
    // Timed release is covered by LX04 production tests; this static test keeps its prior Hold.
    e.execute(Command::Mode(Mode::Assist)).unwrap();
    assert!(!e.preview_is_current(&token));
    go(&mut e, "cue-1", "playback-1");
    assert_eq!(attr(&e.snapshot(), Attribute::Intensity).resolved, 0);
    let mut e = htp();
    e.execute(Command::Master(500)).unwrap();
    e.execute(Command::FixtureMaster {
        fixture: id("fixture-11"),
        level: 500,
    })
    .unwrap();
    assert_eq!(attr(&e.snapshot(), Attribute::Intensity).final_intent, 175);
    let before = e.snapshot();
    e.execute(Command::Mode(Mode::Assist)).unwrap();
    e.execute(Command::Off(id("playback-1"))).unwrap();
    e.execute(Command::Off(id("playback-2"))).unwrap();
    let after = e.snapshot();
    for (a, b) in before.fixtures[0]
        .attributes
        .iter()
        .zip(&after.fixtures[0].attributes)
    {
        assert_eq!(a.resolved, b.resolved);
        assert_eq!(a.final_intent, b.final_intent);
    }
    e.execute(Command::Blackout(true)).unwrap();
    let s = e.snapshot();
    assert_eq!(attr(&s, Attribute::Intensity).final_intent, 0);
    assert_eq!(attr(&s, Attribute::Red).final_intent, 200);
    assert_eq!(attr(&s, Attribute::Pan).final_intent, 100);
    e.execute(Command::Blackout(false)).unwrap();
    assert_eq!(attr(&e.snapshot(), Attribute::Intensity).final_intent, 175);
    let mut sink = NullSink::default();
    sink.observe(&e);
    let s = sink.last_intent().unwrap();
    assert_eq!(s.output_state(), "null_disarmed");
    assert_eq!(s.physical_state(), "unknown");
    assert_eq!(s.durability, Durability::Volatile);
    assert!(
        s.fixtures[0]
            .attributes
            .iter()
            .all(|a| a.submitted.is_none() && a.observed.is_none())
    );
}
#[test]
fn stored_masks_are_validated_and_store_playback_capacities_are_independent() {
    assert_eq!(
        engine().with_stores(vec![stored("bad", vec![v(Attribute::Red, 100)])]),
        Err(Error::Group)
    );
    let stores = (0..32)
        .flat_map(|i| {
            [StoredKind::Cue, StoredKind::Palette].map(|kind| StoredLook {
                kind,
                id: id(&format!("s{i}")),
                values: vec![v(Attribute::Intensity, 1)],
            })
        })
        .collect();
    let mut e = engine().with_stores(stores).unwrap();
    assert_eq!(
        e.execute(Command::Record {
            kind: StoredKind::Cue,
            id: id("extra")
        }),
        Err(Error::Capacity)
    );
    assert_eq!(
        e.execute(Command::Record {
            kind: StoredKind::Palette,
            id: id("extra")
        }),
        Err(Error::Capacity)
    );
    for i in 0..8 {
        go(&mut e, "s0", &format!("p{i}"));
    }
    let before = e.clone();
    assert_eq!(
        e.execute(Command::Go {
            cue: id("s0"),
            playback: id("extra")
        }),
        Err(Error::Capacity)
    );
    assert_eq!(e, before);
}

#[test]
fn coherent_position_groups_retrigger_and_rounding() {
    let mut e = engine()
        .with_stores(vec![
            stored(
                "first",
                vec![
                    v(Attribute::Pan, 100),
                    v(Attribute::Tilt, -100),
                    v(Attribute::Intensity, 1),
                ],
            ),
            stored(
                "second",
                vec![v(Attribute::Pan, -200), v(Attribute::Tilt, 200)],
            ),
        ])
        .unwrap();
    go(&mut e, "first", "p1");
    go(&mut e, "second", "p2");
    for a in [Attribute::Pan, Attribute::Tilt] {
        assert_eq!(attr(&e.snapshot(), a).source, Source::Playback(id("p2")));
    }
    go(&mut e, "first", "p1");
    for a in [Attribute::Pan, Attribute::Tilt] {
        assert_eq!(attr(&e.snapshot(), a).source, Source::Playback(id("p1")));
    }
    e.execute(Command::Master(500)).unwrap();
    assert_eq!(attr(&e.snapshot(), Attribute::Intensity).final_intent, 1);
    let before = e.clone();
    assert_eq!(
        e.execute(Command::Touch(vec![v(Attribute::Pan, 0)])),
        Err(Error::Group)
    );
    assert_eq!(e, before);
    e.execute(Command::Touch(vec![
        v(Attribute::Pan, 0),
        v(Attribute::Tilt, 0),
    ]))
    .unwrap();
    e.execute(Command::ClearToHold).unwrap();
    e.execute(Command::Level {
        playback: id("p1"),
        level: 0,
    })
    .unwrap();
    assert_eq!(attr(&e.snapshot(), Attribute::Pan).resolved, 0);
    assert_eq!(attr(&e.snapshot(), Attribute::Pan).source, Source::Hold);
}

#[test]
fn initial_store_seeding_refuses_active_state_and_preserves_preview() {
    let mut e = htp();
    e.execute(Command::Touch(vec![v(Attribute::Intensity, 0)]))
        .unwrap();
    e.execute(Command::ClearToHold).unwrap();
    let preview = e.preview_release(&[v(Attribute::Intensity, 0)]).unwrap();
    let before = e.clone();
    assert_eq!(
        e.clone()
            .with_stores(vec![stored("late", vec![v(Attribute::Intensity, 100)])]),
        Err(Error::Unavailable)
    );
    assert_eq!(e, before);
    assert!(e.preview_is_current(&preview));
    assert_eq!(preview.values()[0].destination, 700);
    assert_eq!(
        engine()
            .with_stores(vec![stored("initial", vec![v(Attribute::Intensity, 100)])])
            .unwrap()
            .revision(),
        0
    );
}
