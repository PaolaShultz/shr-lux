use shr_lux::{
    authority::{Authority, Command, Mode},
    fixture::*,
    scenes::*,
};
fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn patch() -> Patch {
    Patch::validate(PatchSpec {
        version: 1,
        patch_revision: 1,
        fixtures: vec![
            FixtureSpec::synthetic(id("wash"), "Wash", SyntheticMode::Rgb, 1),
            FixtureSpec::synthetic(id("front"), "Front", SyntheticMode::Dimmer, 10),
        ],
    })
    .unwrap()
}
fn evidence(energy: i32, section: Option<u32>) -> Evidence {
    Evidence {
        available: true,
        energy,
        density: 200,
        confidence: 1000,
        section_id: section,
        harmonic_relation: None,
    }
}
fn feed(d: &mut Director, p: &Patch, start: u64, end: u64, energy: i32, section: Option<u32>) {
    for ms in (start..end).step_by(100) {
        d.step(evidence(energy, section), ms, p, &[]).unwrap();
    }
}
#[test]
fn strict_model_fixture_bindings_ranges_and_capability_resolution() {
    let p = patch();
    let map = ShowMap::defaults(&p);
    map.validate(&p).unwrap();
    let values = map.values(&id("intimate"), &p).unwrap();
    assert_eq!(values.len(), 5);
    assert!(
        !values
            .iter()
            .any(|v| v.fixture == id("front") && v.attribute == Attribute::Red)
    );
    for change in 0..6 {
        let mut bad = map.clone();
        match change {
            0 => bad.scenes[0].fade_ms = 0,
            1 => bad.roles[0].fixtures.push(id("absent")),
            2 => bad.scenes[0].compatible_from.push(id("missing")),
            3 => bad.scenes[0].roles[0].rgb[0] = 1001,
            4 => bad.scenes[0].conditions.energy.min = 1200,
            _ => bad.scenes.push(bad.scenes[0].clone()),
        };
        assert!(bad.validate(&p).is_err());
    }
    let mut raw = serde_json::to_value(map).unwrap();
    raw["scenes"][0]["extra"] = true.into();
    assert!(serde_json::from_value::<ShowMap>(raw).is_err());
}
#[test]
fn stable_harmony_cannot_cycle_and_recurring_section_retains_identity() {
    let p = patch();
    let mut d = Director::new(ShowMap::defaults(&p));
    feed(&mut d, &p, 0, 7000, 100, Some(1));
    assert_eq!(d.status.current, Some(id("intimate")));
    for ms in (7000..11000).step_by(100) {
        let mut e = evidence(100, Some(1));
        e.harmonic_relation = Some(format!("relation-{ms}"));
        d.step(e, ms, &p, &[]).unwrap();
        assert_eq!(d.status.current, Some(id("intimate")));
    }
    feed(&mut d, &p, 11000, 18000, 700, Some(2));
    assert_eq!(d.status.current, Some(id("driving")));
    feed(&mut d, &p, 18000, 26000, 100, Some(1));
    assert_eq!(d.status.current, Some(id("intimate")));
    assert_eq!(d.status.variation, 0);
}
#[test]
fn confirmation_dwell_fade_uncertainty_and_source_reset() {
    let p = patch();
    let mut d = Director::new(ShowMap::defaults(&p));
    feed(&mut d, &p, 0, 500, 100, None);
    assert!(d.status.current.is_none());
    let mut unknown = evidence(100, None);
    unknown.confidence = 0;
    d.step(unknown, 500, &p, &[]).unwrap();
    feed(&mut d, &p, 600, 1200, 100, None);
    assert!(d.status.current.is_none());
    feed(&mut d, &p, 1200, 1300, 100, None);
    assert_eq!(d.status.current, Some(id("intimate")));
    let first = d.status.transition.clone().unwrap();
    feed(&mut d, &p, 1300, 1800, 900, None);
    assert_eq!(d.status.transition.as_ref().unwrap().to, first.to);
    assert_eq!(d.status.reason, "finishing_fade");
    d.reset_source();
    assert!(d.status.current.is_none());
    assert!(d.status.transition.is_none());
    assert!(d.status.intended_values.is_empty());
}
#[test]
fn bounded_variation_changes_rgb_and_repetition_never_drives_switch() {
    let p = patch();
    let mut map = ShowMap::defaults(&p);
    for s in &mut map.scenes {
        s.conditions.energy = Range { min: 0, max: 1000 };
        s.conditions.trend = Range {
            min: -1000,
            max: 1000,
        };
        s.repetition_penalty = 1000;
    }
    let mut d = Director::new(map);
    feed(&mut d, &p, 0, 10000, 500, None);
    let chosen = d.status.current.clone();
    feed(&mut d, &p, 10000, 30000, 500, None);
    assert_eq!(d.status.current, chosen);
}
#[test]
fn actual_rgb_uses_existing_authority_manual_hold_caps_and_loss_freeze() {
    let p = patch();
    let mut a = Authority::new(p.clone(), "11111111-1111-4111-8111-111111111111", 1).unwrap();
    a.enable_analysis();
    a.enable_scenes();
    a.execute(Command::Mode(Mode::Auto)).unwrap();
    let values = ShowMap::defaults(&p).values(&id("floating"), &p).unwrap();
    a.step_scene(
        &values,
        &[id("wash")],
        500,
        600,
        &serde_json::json!({"map_revision":"1"}),
    )
    .unwrap();
    let snap = a.snapshot();
    snap.validate_for_schema(&p, "lx06-scenes-v1").unwrap();
    assert!(snap.validate_for_schema(&p, "lx05-v1").is_err());
    let red = snap.fixtures[0]
        .attributes
        .iter()
        .find(|a| a.attribute == Attribute::Red)
        .unwrap();
    assert_eq!(red.auto, Some(230));
    assert!(snap.fixtures[1].attributes[0].auto.is_some());
    a.execute(Command::Touch(vec![
        Value {
            fixture: id("wash"),
            attribute: Attribute::Red,
            value: 900,
        },
        Value {
            fixture: id("wash"),
            attribute: Attribute::Green,
            value: 850,
        },
        Value {
            fixture: id("wash"),
            attribute: Attribute::Blue,
            value: 800,
        },
    ]))
    .unwrap();
    a.execute(Command::ClearToHold).unwrap();
    a.step_scene(&values, &[id("wash")], 500, 600, &serde_json::json!({}))
        .unwrap();
    let current = a.snapshot();
    assert_eq!(
        current.fixtures[0]
            .attributes
            .iter()
            .find(|a| a.attribute == Attribute::Red)
            .unwrap()
            .resolved,
        900
    );
    a.freeze_auto("source_loss").unwrap();
    assert!(
        a.scene_inventory()["values"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["source"] == "scene-held")
    );
    a.execute(Command::Mode(Mode::Manual)).unwrap();
    assert!(a.scene_inventory()["values"].as_array().unwrap().is_empty());
}

#[test]
fn varied_look_is_real_deterministic_and_unknown_midfade_cancels() {
    let p = patch();
    let mut a = Director::new(ShowMap::defaults(&p));
    let mut b = a.clone();
    for (start, end, energy, section) in [
        (0, 7000, 100, Some(1)),
        (7000, 15000, 700, Some(2)),
        (15000, 23000, 100, Some(1)),
    ] {
        for now in (start..end).step_by(100) {
            let e = evidence(energy, section);
            a.step(e.clone(), now, &p, &[]).unwrap();
            b.step(e, now, &p, &[]).unwrap();
            assert_eq!(
                serde_json::to_value(&a.status).unwrap(),
                serde_json::to_value(&b.status).unwrap()
            );
        }
        if energy == 700 {
            assert!(a.status.variation > 0);
            let current = a
                .status
                .map
                .scene(a.status.current.as_ref().unwrap())
                .unwrap();
            let red = a
                .status
                .intended_values
                .iter()
                .find(|v| v.fixture == id("wash") && v.attribute == Attribute::Red)
                .unwrap();
            assert_eq!(
                red.value,
                (current.roles[0].rgb[0] + a.status.variation).min(1000)
            );
        }
    }
    let mut unknown = evidence(900, Some(3));
    unknown.confidence = 0;
    feed(&mut a, &p, 23000, 24200, 900, Some(3));
    let actual = vec![Value {
        fixture: id("wash"),
        attribute: Attribute::Intensity,
        value: 123,
    }];
    a.step(unknown, 24200, &p, &actual).unwrap();
    assert!(a.status.transition.is_none());
    assert_eq!(a.status.intended_values, actual);
    a.reset_source();
    assert_eq!(a.status.interpretation.change, "unknown");
    assert_eq!(a.status.variation, 0);
}
#[test]
fn role_omission_retains_held_values_and_bad_scene_step_is_atomic() {
    let p = patch();
    let mut a = Authority::new(p.clone(), "11111111-1111-4111-8111-111111111111", 1).unwrap();
    a.enable_analysis();
    a.enable_scenes();
    a.execute(Command::Mode(Mode::Auto)).unwrap();
    let before_front = a.snapshot().fixtures[1].clone();
    let values = ShowMap::defaults(&p).values(&id("floating"), &p).unwrap();
    a.step_scene(&values, &[id("wash")], 500, 1000, &serde_json::json!({}))
        .unwrap();
    assert_eq!(a.snapshot().fixtures[1], before_front);
    let before = a.clone();
    let mut bad = values.clone();
    bad.push(Value {
        fixture: id("wash"),
        attribute: Attribute::Blue,
        value: 1001,
    });
    assert!(
        a.step_scene(&bad, &[id("wash")], 500, 1000, &serde_json::json!({}))
            .is_err()
    );
    assert_eq!(a, before);
    a.step_scene(&[], &[id("wash")], 500, 1000, &serde_json::json!({}))
        .unwrap();
    assert!(
        a.scene_inventory()["values"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["source"] == "scene-held" && v["held_reason"] == "scene_unmapped")
    );
    a.replace_scene_intensity(&[id("wash")]).unwrap();
    a.step_auto(&[id("wash")], 500, 200).unwrap();
    assert!(
        !a.scene_inventory()["values"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["attribute"] == "intensity")
    );
    assert!(
        a.scene_inventory()["values"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["source"] == "scene-held")
    );
}
#[test]
fn distant_harmonic_relation_only_modifies_supported_transition_duration() {
    let p = patch();
    for relation in ["stationary", "fifthsforward", "other"] {
        let mut d = Director::new(ShowMap::defaults(&p));
        for now in (0..700).step_by(100) {
            let mut e = evidence(100, None);
            e.harmonic_relation = Some(relation.into());
            d.step(e, now, &p, &[]).unwrap();
        }
        assert_eq!(d.status.current, Some(id("intimate")));
        assert_eq!(
            d.status.transition.as_ref().unwrap().duration_ms,
            if relation == "other" { 1500 } else { 1200 }
        );
    }
}

#[test]
fn six_default_families_require_their_supported_conditions() {
    let p = patch();
    for (family, first, then) in [
        (Family::Intimate, 100, 100),
        (Family::Floating, 300, 300),
        (Family::Driving, 900, 900),
        (Family::Building, 100, 500),
        (Family::OpenLift, 300, 850),
        (Family::Release, 900, 600),
    ] {
        let mut d = Director::new(ShowMap::defaults(&p));
        d.step(evidence(first, None), 0, &p, &[]).unwrap();
        feed(&mut d, &p, 100, 2000, then, None);
        let selected = d
            .status
            .map
            .scene(d.status.current.as_ref().expect("confirmed family"))
            .unwrap();
        assert_eq!(
            selected.family, family,
            "{first}->{then}: {:?}",
            d.status.reason
        );
    }
}

#[test]
fn gradual_sustained_build_release_and_high_plateau_are_distinct() {
    let p = patch();
    let mut d = Director::new(ShowMap::defaults(&p));
    feed(&mut d, &p, 0, 5000, 100, None);
    let mut families = vec![];
    for now in (5000..17000).step_by(100) {
        let e = 100 + ((now - 5000) * 600 / 12000) as i32;
        d.step(evidence(e, None), now, &p, &[]).unwrap();
        if let Some(s) = d
            .status
            .current
            .as_ref()
            .and_then(|id| d.status.map.scene(id))
        {
            families.push(s.family);
        }
    }
    assert!(families.contains(&Family::Building));
    feed(&mut d, &p, 17000, 26000, 700, None);
    assert_eq!(
        d.status
            .map
            .scene(d.status.current.as_ref().unwrap())
            .unwrap()
            .family,
        Family::Driving
    );
    for now in (26000..38000).step_by(100) {
        let e = 700 - ((now - 26000) * 600 / 12000) as i32;
        d.step(evidence(e, None), now, &p, &[]).unwrap();
        if let Some(s) = d
            .status
            .current
            .as_ref()
            .and_then(|id| d.status.map.scene(id))
        {
            families.push(s.family);
        }
    }
    assert!(families.contains(&Family::Release));
    feed(&mut d, &p, 38000, 45000, 100, None);
    assert_eq!(
        d.status
            .map
            .scene(d.status.current.as_ref().unwrap())
            .unwrap()
            .family,
        Family::Intimate
    );
}

#[test]
fn density_and_scene_confidence_conditions_can_end_an_energy_stable_hold() {
    let p = patch();
    for confidence_case in [false, true] {
        let mut map = ShowMap::defaults(&p);
        for s in &mut map.scenes {
            s.conditions.energy = Range {
                min: 999,
                max: 1000,
            };
        }
        map.scenes[0].conditions.energy = Range { min: 0, max: 500 };
        map.scenes[1].conditions.energy = Range { min: 80, max: 500 };
        if confidence_case {
            map.scenes[0].conditions.min_confidence = 900;
            map.scenes[1].conditions.min_confidence = 600;
        } else {
            map.scenes[0].conditions.density = Range { min: 0, max: 300 };
            map.scenes[1].conditions.density = Range {
                min: 400,
                max: 1000,
            };
        }
        let mut d = Director::new(map);
        feed(&mut d, &p, 0, 8000, 100, None);
        assert_eq!(d.status.current, Some(id("intimate")));
        for now in (8000..13000).step_by(100) {
            let mut e = evidence(100, None);
            if confidence_case {
                e.confidence = 700;
            } else {
                e.density = 700;
            }
            d.step(e, now, &p, &[]).unwrap();
            if now < 8600 {
                assert_eq!(d.status.current, Some(id("intimate")));
            }
        }
        assert_eq!(d.status.current, Some(id("floating")));
    }
}
