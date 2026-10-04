use shr_lux::fixture::*;
use shr_lux::lighting_contract::*;

fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn rig(mode: SyntheticMode, address: u16) -> PatchSpec {
    PatchSpec {
        version: VERSION,
        patch_revision: 1,
        fixtures: vec![FixtureSpec::synthetic(
            id("fixture-11"),
            "Invented fixture",
            mode,
            address,
        )],
    }
}
fn value(attribute: Attribute, value: i32) -> Value {
    Value {
        fixture: id("fixture-11"),
        attribute,
        value,
    }
}
#[test]
fn fixture_patch_boundaries_and_atomic_replacement() {
    let mut patch = Patch::validate(rig(SyntheticMode::RgbPosition, 506)).unwrap();
    let before = patch.clone();
    for address in [0, 507, 512, 513, u16::MAX] {
        assert_eq!(
            patch.replace(rig(SyntheticMode::RgbPosition, address)),
            Err(Error::Address)
        );
        assert_eq!(patch, before);
    }
    assert!(Patch::validate(rig(SyntheticMode::Dimmer, 512)).is_ok());
    let mut spec = rig(SyntheticMode::Rgb, 1);
    spec.fixtures.push(FixtureSpec::synthetic(
        id("fixture-12"),
        "Second",
        SyntheticMode::Dimmer,
        4,
    ));
    assert_eq!(Patch::validate(spec.clone()), Err(Error::Overlap));
    spec.fixtures[1].address = 5;
    assert!(Patch::validate(spec.clone()).is_ok());
    spec.fixtures[1].id = id("fixture-11");
    assert_eq!(Patch::validate(spec), Err(Error::Duplicate));
    let mut spec = rig(SyntheticMode::Dimmer, 1);
    spec.version = 2;
    assert_eq!(Patch::validate(spec), Err(Error::Version));
    let spec = PatchSpec {
        version: VERSION,
        patch_revision: 1,
        fixtures: (0..33)
            .map(|i| {
                FixtureSpec::synthetic(
                    id(&format!("fixture-{i}")),
                    "",
                    SyntheticMode::Dimmer,
                    i + 1,
                )
            })
            .collect(),
    };
    assert_eq!(Patch::validate(spec), Err(Error::Capacity));
}
#[test]
fn fixture_defaults_modes_units_and_ids() {
    for s in ["", "A", "a/b", "a b", "é", "_a"] {
        assert_eq!(Id::new(s), Err(Error::Id));
    }
    assert!(Id::new(&"a".repeat(64)).is_ok());
    assert_eq!(Id::new(&"a".repeat(65)), Err(Error::Id));
    let mut spec = rig(SyntheticMode::RgbPosition, 1);
    spec.fixtures[0].capabilities[0].default = 1001;
    assert_eq!(Patch::validate(spec.clone()), Err(Error::Default));
    spec.fixtures[0].capabilities[0].default = 0;
    spec.fixtures[0].capabilities[0].min = 1;
    assert_eq!(Patch::validate(spec.clone()), Err(Error::Range));
    spec.fixtures[0].capabilities[0].min = 0;
    spec.fixtures[0].capabilities.pop();
    assert_eq!(Patch::validate(spec), Err(Error::Mode));
    for label in ["bad\nlabel".to_owned(), "é".repeat(65)] {
        let mut spec = rig(SyntheticMode::Dimmer, 1);
        spec.fixtures[0].label = label;
        assert_eq!(Patch::validate(spec), Err(Error::Label));
    }
    assert_eq!(Attribute::Pan.unit(), "tenth_degree");
    assert_eq!(Attribute::Red.unit(), "tenth_percent");
}
#[test]
fn fixture_coherent_edits_reject_partial_or_invalid_multi_target_values() {
    let patch = Patch::validate(rig(SyntheticMode::RgbPosition, 1)).unwrap();
    assert_eq!(
        patch.validate_values(&[value(Attribute::Red, 100)]),
        Err(Error::Group)
    );
    assert_eq!(
        patch.validate_values(&[value(Attribute::Pan, 100)]),
        Err(Error::Group)
    );
    let mut rgb = vec![
        value(Attribute::Red, 1000),
        value(Attribute::Green, 0),
        value(Attribute::Blue, 500),
    ];
    assert!(patch.validate_values(&rgb).is_ok());
    rgb[2].value = -1;
    assert_eq!(patch.validate_values(&rgb), Err(Error::Range));
    assert!(
        patch
            .validate_values(&[value(Attribute::Pan, -2700), value(Attribute::Tilt, 1350)])
            .is_ok()
    );
    assert_eq!(
        patch.validate_values(&[value(Attribute::Pan, 2701), value(Attribute::Tilt, 0)]),
        Err(Error::Range)
    );
    assert_eq!(
        patch.validate_values(&[value(Attribute::Zoom, 49)]),
        Err(Error::Range)
    );
    assert_eq!(
        patch.validate_values(&[
            value(Attribute::Intensity, 0),
            value(Attribute::Intensity, 1)
        ]),
        Err(Error::Duplicate)
    );
    assert_eq!(
        patch.validate_values(&vec![value(Attribute::Intensity, 0); 65]),
        Err(Error::Capacity)
    );
    assert_eq!(
        Patch::validate(rig(SyntheticMode::Dimmer, 1))
            .unwrap()
            .validate_values(&[value(Attribute::Zoom, 100)]),
        Err(Error::Unavailable)
    );
    let mut unknown = value(Attribute::Intensity, 0);
    unknown.fixture = id("missing");
    assert_eq!(
        patch.validate_values(&[value(Attribute::Intensity, 0), unknown]),
        Err(Error::Target)
    );
}
fn default_snapshot(patch: &Patch) -> Snapshot {
    Snapshot {
        version: VERSION,
        show_id: "11111111-1111-4111-8111-111111111111".into(),
        epoch: 9,
        revision: 1,
        patch_revision: patch.revision(),
        durability: Durability::Volatile,
        fixtures: patch
            .fixtures()
            .iter()
            .map(|f| FixtureSnapshot {
                fixture: f.id.clone(),
                attributes: f
                    .capabilities
                    .iter()
                    .map(|c| AttributeSnapshot {
                        attribute: c.attribute,
                        programmer: None,
                        release: None,
                        auto: None,
                        hold: None,
                        proposal: None,
                        stored: vec![],
                        playing: vec![],
                        resolved: c.default,
                        final_intent: c.default,
                        source: Source::FixtureDefault,
                        contributors: vec![Contributor {
                            source: Source::FixtureDefault,
                            value: c.default,
                            winner: true,
                        }],
                        inhibit: Inhibit::None,
                        clamped: false,
                        submitted: None,
                        observed: None,
                    })
                    .collect(),
            })
            .collect(),
    }
}
#[test]
fn fixture_snapshot_masks_provenance_bounds_and_unknown_output() {
    let patch = Patch::validate(rig(SyntheticMode::RgbPosition, 1)).unwrap();
    let snapshot = default_snapshot(&patch);
    assert!(snapshot.validate(&patch).is_ok());
    assert_eq!(snapshot.output_state(), "null_disarmed");
    assert_eq!(snapshot.physical_state(), "unknown");
    let mut s = snapshot.clone();
    s.fixtures[0].attributes[0].observed = Some(0);
    assert_eq!(s.validate(&patch), Err(Error::Unavailable));
    let mut s = snapshot.clone();
    s.durability = Durability::Checkpointed;
    assert_eq!(s.validate(&patch), Err(Error::Unavailable));
    let mut s = snapshot.clone();
    s.fixtures[0].attributes[1].programmer = Some(0);
    assert_eq!(s.validate(&patch), Err(Error::Target));
    let mut s = snapshot.clone();
    s.fixtures[0].attributes[0].contributors.clear();
    assert_eq!(s.validate(&patch), Err(Error::Target));
    let mut s = snapshot.clone();
    s.fixtures[0].attributes[0].playing = (0..9)
        .map(|i| NamedValue {
            id: id(&format!("playback-{i}")),
            value: 0,
        })
        .collect();
    assert_eq!(s.validate(&patch), Err(Error::Capacity));
    let mut s = snapshot.clone();
    s.patch_revision += 1;
    assert_eq!(s.validate(&patch), Err(Error::Target));
    assert_eq!(decimal_counter("18446744073709551615"), Ok(u64::MAX));
    for v in ["18446744073709551616", "-1", "1.0", "1e3", "", " 1"] {
        assert_eq!(decimal_counter(v), Err(Error::Range));
    }
    assert!(validate_show_id("11111111-1111-4111-8111-111111111111").is_ok());
    assert_eq!(
        validate_show_id("11111111-1111-4111-8111-11111111111A"),
        Err(Error::Id)
    );
}
#[test]
fn fixture_e04_corpus_validates_expected_states_without_arbitration() {
    let patch = Patch::validate(rig(SyntheticMode::RgbPosition, 1)).unwrap();
    let corpus = include_str!("fixtures/c-light-v1/e04.tsv");
    let mut cases = 0;
    let header: Vec<_> = corpus.lines().next().unwrap().split('\t').collect();
    for line in corpus.lines().skip(1) {
        let c: Vec<_> = line.split('\t').collect();
        assert_eq!(c.len(), header.len());
        let field = |name: &str| c[header.iter().position(|h| *h == name).unwrap()];
        let integer = |name: &str| field(name).parse::<i32>().unwrap();
        let optional = |name: &str| {
            if field(name) == "null" {
                None
            } else {
                Some(integer(name))
            }
        };
        let mut s = default_snapshot(&patch);
        let source = match field("source") {
            "programmer" => Source::Programmer,
            "hold" => Source::Hold,
            "playback-1" => Source::Playback(id("playback-1")),
            other => panic!("unknown source {other}"),
        };
        let a = &mut s.fixtures[0].attributes[0];
        a.programmer = optional("programmer");
        a.hold = optional("hold");
        a.resolved = integer("resolved");
        a.final_intent = integer("final_intent");
        a.source = source.clone();
        a.playing = vec![
            NamedValue {
                id: id("playback-1"),
                value: integer("playback_1"),
            },
            NamedValue {
                id: id("playback-2"),
                value: integer("playback_2"),
            },
        ];
        a.contributors = a
            .playing
            .iter()
            .enumerate()
            .map(|(i, p)| Contributor {
                source: Source::Playback(p.id.clone()),
                value: p.value,
                winner: field(if i == 0 { "winner_1" } else { "winner_2" }) == "true",
            })
            .collect();
        if matches!(source, Source::Programmer | Source::Hold) {
            a.contributors.push(Contributor {
                source,
                value: a.resolved,
                winner: true,
            });
        }
        a.inhibit = if field("blackout") == "true" {
            Inhibit::Blackout
        } else {
            Inhibit::None
        };
        for (i, attribute) in s.fixtures[0].attributes.iter_mut().enumerate().skip(1) {
            attribute.resolved = integer(
                [
                    "color_red",
                    "color_green",
                    "color_blue",
                    "pan",
                    "tilt",
                    "zoom",
                ][i - 1],
            );
            attribute.final_intent = attribute.resolved;
            attribute.source = Source::Playback(id("playback-1"));
            attribute.playing = vec![NamedValue {
                id: id("playback-1"),
                value: attribute.resolved,
            }];
            attribute.contributors = vec![Contributor {
                source: attribute.source.clone(),
                value: attribute.resolved,
                winner: true,
            }];
        }
        assert_eq!(field("submitted"), "null");
        assert_eq!(field("observed"), "null");
        assert!(s.validate(&patch).is_ok(), "{}", c[0]);
        assert_eq!(a_value(&s), (integer("resolved"), integer("final_intent")));
        cases += 1;
    }
    assert_eq!(cases, 5);
}
fn a_value(s: &Snapshot) -> (i32, i32) {
    let a = &s.fixtures[0].attributes[0];
    (a.resolved, a.final_intent)
}

#[test]
fn fixture_patch_corpus_exercises_owner_validation() {
    for line in include_str!("fixtures/c-light-v1/patch.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = line.split('\t').collect();
        assert_eq!(c.len(), 6);
        let mode = match c[1] {
            "dimmer" => SyntheticMode::Dimmer,
            "rgb" => SyntheticMode::Rgb,
            "rgb_position" => SyntheticMode::RgbPosition,
            _ => panic!("unknown mode"),
        };
        let mut spec = rig(mode, c[2].parse().unwrap());
        spec.fixtures[0].capabilities[0].default = c[4].parse().unwrap();
        if c[3] != "null" {
            spec.fixtures.push(FixtureSpec::synthetic(
                id("fixture-12"),
                "Second",
                SyntheticMode::Dimmer,
                c[3].parse().unwrap(),
            ));
        }
        let result = Patch::validate(spec).map(|_| ());
        let expected = match c[5] {
            "valid" => Ok(()),
            "address" => Err(Error::Address),
            "overlap" => Err(Error::Overlap),
            "default" => Err(Error::Default),
            _ => panic!("unknown result"),
        };
        assert_eq!(result, expected, "{}", c[0]);
    }
}
#[test]
fn fixture_patch_reorder_keeps_identity_and_advertised_angular_limits() {
    let mut spec = rig(SyntheticMode::RgbPosition, 1);
    spec.fixtures.push(FixtureSpec::synthetic(
        id("fixture-12"),
        "Second",
        SyntheticMode::Rgb,
        8,
    ));
    spec.fixtures[0]
        .capabilities
        .iter_mut()
        .find(|c| c.attribute == Attribute::Pan)
        .unwrap()
        .max = 1200;
    let patch = Patch::validate(spec.clone()).unwrap();
    spec.fixtures.reverse();
    let reordered = Patch::validate(spec).unwrap();
    assert_eq!(
        patch.fixture(&id("fixture-11")),
        reordered.fixture(&id("fixture-11"))
    );
    assert_eq!(
        patch.validate_values(&[value(Attribute::Pan, 1201), value(Attribute::Tilt, 0)]),
        Err(Error::Range)
    );
    assert!(
        patch
            .validate_values(&[value(Attribute::Pan, 1200), value(Attribute::Tilt, 0)])
            .is_ok()
    );
}
#[test]
fn fixture_snapshot_incomplete_groups_and_unbacked_contributors_are_refused() {
    let patch = Patch::validate(rig(SyntheticMode::RgbPosition, 1)).unwrap();
    let snapshot = default_snapshot(&patch);
    let mut s = snapshot.clone();
    s.fixtures[0].attributes.pop();
    assert_eq!(s.validate(&patch), Err(Error::Group));
    let mut s = snapshot.clone();
    s.fixtures[0].attributes[1] = s.fixtures[0].attributes[0].clone();
    assert_eq!(s.validate(&patch), Err(Error::Duplicate));
    let mut s = snapshot.clone();
    s.fixtures[0].attributes[0].contributors.push(Contributor {
        source: Source::Playback(id("missing")),
        value: 0,
        winner: false,
    });
    assert_eq!(s.validate(&patch), Err(Error::Target));
    let mut s = snapshot.clone();
    s.fixtures[0].attributes[0].submitted = Some(0);
    assert_eq!(s.validate(&patch), Err(Error::Unavailable));
    let mut s = snapshot.clone();
    s.version = 2;
    assert_eq!(s.validate(&patch), Err(Error::Version));
}

#[test]
fn fixture_snapshot_global_playback_capacity_is_not_per_attribute() {
    let patch = Patch::validate(rig(SyntheticMode::RgbPosition, 1)).unwrap();
    let mut s = default_snapshot(&patch);
    s.fixtures[0].attributes[0].playing = (0..8)
        .map(|i| NamedValue {
            id: id(&format!("playback-{i}")),
            value: 0,
        })
        .collect();
    s.fixtures[0].attributes[0].contributors = s.fixtures[0].attributes[0]
        .playing
        .iter()
        .map(|p| Contributor {
            source: Source::Playback(p.id.clone()),
            value: p.value,
            winner: true,
        })
        .collect();
    s.fixtures[0].attributes[0].source = Source::Playback(id("playback-0"));
    assert!(s.validate(&patch).is_ok());
    s.fixtures[0].attributes[6].playing.push(NamedValue {
        id: id("playback-8"),
        value: 50,
    });
    assert_eq!(s.validate(&patch), Err(Error::Capacity));
}

#[test]
fn fixture_complete_provenance_priority_and_auto_refusal() {
    let patch = Patch::validate(rig(SyntheticMode::Dimmer, 1)).unwrap();
    let mut s = default_snapshot(&patch);
    let a = &mut s.fixtures[0].attributes[0];
    a.playing = vec![
        NamedValue {
            id: id("p1"),
            value: 700,
        },
        NamedValue {
            id: id("p2"),
            value: 500,
        },
    ];
    a.source = Source::Playback(id("p1"));
    a.resolved = 700;
    a.final_intent = 700;
    a.contributors = vec![Contributor {
        source: a.source.clone(),
        value: 700,
        winner: true,
    }];
    assert_eq!(s.validate(&patch), Err(Error::Target));
    s.fixtures[0].attributes[0].contributors.push(Contributor {
        source: Source::Playback(id("p2")),
        value: 500,
        winner: false,
    });
    assert_eq!(s.validate(&patch), Ok(()));
    let mut tie = s.clone();
    tie.fixtures[0].attributes[0].playing[1].value = 700;
    tie.fixtures[0].attributes[0].contributors[1].value = 700;
    assert_eq!(tie.validate(&patch), Err(Error::Target));
    tie.fixtures[0].attributes[0].contributors[1].winner = true;
    assert_eq!(tie.validate(&patch), Ok(()));
    let mut manual = s.clone();
    let a = &mut manual.fixtures[0].attributes[0];
    a.programmer = Some(0);
    a.hold = Some(100);
    a.resolved = 0;
    a.final_intent = 0;
    a.source = Source::Programmer;
    for c in &mut a.contributors {
        c.winner = false;
    }
    a.contributors.push(Contributor {
        source: Source::Programmer,
        value: 0,
        winner: true,
    });
    assert_eq!(manual.validate(&patch), Err(Error::Target));
    manual.fixtures[0].attributes[0]
        .contributors
        .push(Contributor {
            source: Source::Hold,
            value: 100,
            winner: false,
        });
    assert_eq!(manual.validate(&patch), Ok(()));
    let mut auto = default_snapshot(&patch);
    let a = &mut auto.fixtures[0].attributes[0];
    a.proposal = Some(700);
    a.source = Source::Auto(id("unbacked"));
    a.resolved = 700;
    a.final_intent = 700;
    a.contributors = vec![Contributor {
        source: a.source.clone(),
        value: 700,
        winner: true,
    }];
    assert_eq!(auto.validate(&patch), Err(Error::Unavailable));
    let mut proposal = default_snapshot(&patch);
    proposal.fixtures[0].attributes[0].proposal = Some(700);
    assert_eq!(proposal.validate(&patch), Ok(()));
}
#[test]
fn fixture_stored_namespaces_have_independent_global_limits() {
    let patch = Patch::validate(rig(SyntheticMode::RgbPosition, 1)).unwrap();
    let mut s = default_snapshot(&patch);
    s.fixtures[0].attributes[0].stored = [StoredKind::Cue, StoredKind::Palette]
        .into_iter()
        .flat_map(|kind| {
            (0..32).map(move |i| StoredValue {
                kind,
                id: id(&format!("stored-{i}")),
                value: 0,
            })
        })
        .collect();
    assert_eq!(s.validate(&patch), Ok(()));
    for kind in [StoredKind::Cue, StoredKind::Palette] {
        let mut over = s.clone();
        over.fixtures[0].attributes[6].stored.push(StoredValue {
            kind,
            id: id("extra"),
            value: 50,
        });
        assert_eq!(over.validate(&patch), Err(Error::Capacity));
    }
    s.fixtures[0].attributes[0].stored.pop();
    let duplicate = s.fixtures[0].attributes[0].stored[0].clone();
    s.fixtures[0].attributes[0].stored.push(duplicate);
    assert_eq!(s.validate(&patch), Err(Error::Duplicate));
}
