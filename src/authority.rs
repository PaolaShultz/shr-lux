//! C-LIGHT null-output authority, reviewed logical release and opt-in bounded intensity automation.
//! Audio transport/DSP and physical I/O remain outside this owner.
use crate::fixture::{Attribute, Error, Id, Patch, PatchSpec, VERSION, Value};
use crate::lighting_contract::{
    AttributeSnapshot, Contributor, Durability, FixtureSnapshot, Inhibit, MAX_CUES, MAX_PALETTES,
    MAX_PLAYBACKS, NamedValue, Snapshot, Source, StoredKind, StoredValue, validate_show_id,
};
use std::collections::BTreeMap;

type Key = (Id, Attribute);
type Look = BTreeMap<Key, i32>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Manual,
    Assist,
    Auto,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Playback {
    look: Look,
    level: i32,
    order: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoValue {
    value: i32,
    source: String,
    held_reason: Option<String>,
    provenance: Option<serde_json::Value>,
}
/// Each command is one atomic transaction. Caller selection never implies ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Touch(Vec<Value>),
    ClearToHold,
    Record { kind: StoredKind, id: Id },
    Update { kind: StoredKind, id: Id },
    ApplyPalette(Id),
    Go { cue: Id, playback: Id },
    Level { playback: Id, level: i32 },
    Off(Id),
    Master(i32),
    FixtureMaster { fixture: Id, level: i32 },
    Blackout(bool),
    Mode(Mode),
    ReplacePatch(PatchSpec),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authority {
    patch: Patch,
    show_id: String,
    epoch: u64,
    revision: u64,
    programmer: Look,
    hold: Look,
    cues: BTreeMap<Id, Look>,
    palettes: BTreeMap<Id, Look>,
    playing: BTreeMap<Id, Playback>,
    order: u64,
    master: i32,
    fixture_masters: BTreeMap<Id, i32>,
    blackout: bool,
    mode: Mode,
    tick: u64,
    transition: Option<ReleaseTransition>,
    analysis_enabled: bool,
    automatic: BTreeMap<Id, AutoValue>,
    proposal: Option<i32>,
}
impl Authority {
    pub fn new(patch: Patch, show_id: &str, epoch: u64) -> Result<Self, Error> {
        validate_show_id(show_id)?;
        Ok(Self {
            patch,
            show_id: show_id.into(),
            epoch,
            revision: 0,
            programmer: Look::new(),
            hold: Look::new(),
            cues: BTreeMap::new(),
            palettes: BTreeMap::new(),
            playing: BTreeMap::new(),
            order: 0,
            master: 1000,
            fixture_masters: BTreeMap::new(),
            blackout: false,
            mode: Mode::Manual,
            tick: 0,
            transition: None,
            analysis_enabled: false,
            automatic: BTreeMap::new(),
            proposal: None,
        })
    }
    /// Validated volatile initial cue/palette copies. This is not durable restart loading.
    pub fn with_stores(mut self, stores: Vec<StoredLook>) -> Result<Self, Error> {
        if self.revision != 0 {
            return Err(Error::Unavailable);
        }
        for stored in stores {
            let mut look = Look::new();
            for v in stored.values {
                if look.insert((v.fixture, v.attribute), v.value).is_some() {
                    return Err(Error::Duplicate);
                }
            }
            validate_look(&self.patch, &look)?;
            let limit = match stored.kind {
                StoredKind::Cue => MAX_CUES,
                StoredKind::Palette => MAX_PALETTES,
            };
            let store = self.store(stored.kind);
            if store.contains_key(&stored.id) {
                return Err(Error::Duplicate);
            }
            if store.len() >= limit {
                return Err(Error::Capacity);
            }
            store.insert(stored.id, look);
        }
        Ok(self)
    }
    pub fn enable_analysis(&mut self) {
        self.analysis_enabled = true;
    }
    pub fn set_analysis_proposal(&mut self, proposal: Option<i32>) {
        self.proposal = proposal;
    }
    pub fn auto_inventory(&self) -> serde_json::Value {
        serde_json::json!({"values":self.automatic.iter().map(|(fixture,v)|serde_json::json!({"fixture":fixture,"intensity":v.value,"source":v.source,"held_reason":v.held_reason,"provenance":v.provenance})).collect::<Vec<_>>()})
    }
    pub fn set_auto_provenance(&mut self, fixtures: &[Id], provenance: &serde_json::Value) {
        for id in fixtures {
            if self.transition.as_ref().is_some_and(|t| {
                t.values
                    .iter()
                    .any(|v| &v.fixture == id && v.attribute == Attribute::Intensity)
            }) {
                continue;
            }
            if let Some(value) = self.automatic.get_mut(id)
                && value.source == "analysis-active"
            {
                value.provenance = Some(provenance.clone());
            }
        }
    }
    pub fn freeze_auto(&mut self, reason: &str) -> Result<(), Error> {
        let mut changed = false;
        for v in self.automatic.values_mut() {
            if v.source == "analysis-active" {
                v.source = "analysis-held".into();
                v.held_reason = Some(reason.into());
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.checked_add(1).ok_or(Error::Capacity)?;
        }
        Ok(())
    }
    pub fn validate_auto_targets(&self, fixtures: &[Id], cap: i32) -> Result<(), Error> {
        if !self.analysis_enabled || self.mode != Mode::Auto {
            return Err(Error::Unavailable);
        }
        bounded(cap)?;
        if fixtures.is_empty() || fixtures.len() > 32 {
            return Err(Error::Capacity);
        }
        for (i, id) in fixtures.iter().enumerate() {
            if fixtures[..i].contains(id) {
                return Err(Error::Duplicate);
            }
            self.patch
                .fixture(id)
                .and_then(|f| f.capability(Attribute::Intensity))
                .ok_or(Error::Target)?;
            if self.automatic.get(id).map_or(0, |v| v.value) > cap {
                return Err(Error::Range);
            }
        }
        Ok(())
    }
    /// Caller validates fresh source/grant first; this is one bounded owner tick, no catchup.
    pub fn step_auto(&mut self, fixtures: &[Id], cap: i32, proposal: i32) -> Result<(), Error> {
        self.validate_auto_targets(fixtures, cap)?;
        bounded(proposal)?;
        let mut changed = false;
        for id in fixtures {
            // Reviewed release destination must stay fixed until transition removes itself.
            if self.transition.as_ref().is_some_and(|t| {
                t.values
                    .iter()
                    .any(|v| &v.fixture == id && v.attribute == Attribute::Intensity)
            }) {
                continue;
            }
            let v = self.automatic.entry(id.clone()).or_insert(AutoValue {
                value: 0,
                source: "mode-entry-continuity".into(),
                held_reason: None,
                provenance: None,
            });
            let value = &mut v.value;
            let target = proposal.min(cap);
            let step = (target - *value).clamp(-20, 20);
            changed |= step != 0;
            *value += step;
            if v.source != "analysis-active" {
                v.source = "analysis-active".into();
                v.held_reason = None;
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.checked_add(1).ok_or(Error::Capacity)?;
        }
        Ok(())
    }
    pub fn patch(&self) -> &Patch {
        &self.patch
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn master(&self) -> i32 {
        self.master
    }
    pub fn blackout(&self) -> bool {
        self.blackout
    }
    /// Invalid commands retain every field, including revision and activation order.
    pub fn execute(&mut self, command: Command) -> Result<Snapshot, Error> {
        let mut next = self.clone();
        // Cancel from the actual interpolated current position, preserving human continuity.
        if let Some(transition) = next.transition.clone() {
            let touched = match &command {
                Command::Touch(values) => Some(values),
                _ => None,
            };
            for t in &transition.values {
                if touched.is_none_or(|values| {
                    values
                        .iter()
                        .any(|v| v.fixture == t.fixture && v.attribute == t.attribute)
                }) {
                    next.hold
                        .insert((t.fixture.clone(), t.attribute), next.release_value(t));
                }
            }
            if let Some(values) = touched {
                if let Some(active) = &mut next.transition {
                    active.values.retain(|t| {
                        !values
                            .iter()
                            .any(|v| v.fixture == t.fixture && v.attribute == t.attribute)
                    });
                }
                if next
                    .transition
                    .as_ref()
                    .is_some_and(|t| t.values.is_empty())
                {
                    next.transition = None;
                }
            } else {
                next.transition = None;
            }
        }
        next.apply(command)?;
        next.revision = next.revision.checked_add(1).ok_or(Error::Capacity)?;
        let snapshot = next.snapshot();
        if next.analysis_enabled {
            snapshot.validate_for_schema(&next.patch, "lx05-v1")?;
        } else {
            snapshot.validate(&next.patch)?;
        }
        *self = next;
        Ok(snapshot)
    }
    fn store(&mut self, kind: StoredKind) -> &mut BTreeMap<Id, Look> {
        match kind {
            StoredKind::Cue => &mut self.cues,
            StoredKind::Palette => &mut self.palettes,
        }
    }
    fn apply(&mut self, command: Command) -> Result<(), Error> {
        match command {
            Command::Touch(values) => {
                self.patch.validate_values(&values)?;
                for v in values {
                    self.programmer.insert((v.fixture, v.attribute), v.value);
                }
            }
            Command::ClearToHold => self.hold.append(&mut self.programmer),
            Command::Record { kind, id } => {
                let limit = match kind {
                    StoredKind::Cue => MAX_CUES,
                    StoredKind::Palette => MAX_PALETTES,
                };
                if self.store(kind).contains_key(&id) {
                    return Err(Error::Duplicate);
                }
                if self.store(kind).len() >= limit {
                    return Err(Error::Capacity);
                }
                let look = self.programmer.clone();
                self.store(kind).insert(id, look);
            }
            Command::Update { kind, id } => {
                let look = self.programmer.clone();
                self.store(kind)
                    .get_mut(&id)
                    .ok_or(Error::Target)?
                    .extend(look);
            }
            Command::ApplyPalette(id) => {
                let look = self.palettes.get(&id).ok_or(Error::Target)?;
                self.programmer.extend(look.clone());
            }
            Command::Go { cue, playback } => {
                let look = self.cues.get(&cue).ok_or(Error::Target)?.clone();
                if !self.playing.contains_key(&playback) && self.playing.len() >= MAX_PLAYBACKS {
                    return Err(Error::Capacity);
                }
                self.order = self.order.checked_add(1).ok_or(Error::Capacity)?;
                self.playing.insert(
                    playback,
                    Playback {
                        look,
                        level: 1000,
                        order: self.order,
                    },
                );
            }
            Command::Level { playback, level } => {
                bounded(level)?;
                self.playing.get_mut(&playback).ok_or(Error::Target)?.level = level;
            }
            Command::Off(id) => {
                self.playing.remove(&id).ok_or(Error::Target)?;
            }
            Command::Master(level) => {
                bounded(level)?;
                self.master = level;
            }
            Command::FixtureMaster { fixture, level } => {
                bounded(level)?;
                self.patch.fixture(&fixture).ok_or(Error::Target)?;
                self.fixture_masters.insert(fixture, level);
            }
            Command::Blackout(value) => self.blackout = value,
            Command::Mode(mode) => {
                if mode == Mode::Auto && !self.analysis_enabled {
                    return Err(Error::Unavailable);
                }
                if self.mode != mode {
                    let snapshot = self.snapshot();
                    if mode == Mode::Auto {
                        self.automatic.clear();
                        for f in snapshot.fixtures {
                            for a in f.attributes {
                                if a.attribute == Attribute::Intensity
                                    && a.programmer.is_none()
                                    && a.hold.is_none()
                                    && a.release.is_none()
                                {
                                    self.automatic.insert(
                                        f.fixture.clone(),
                                        AutoValue {
                                            value: a.resolved,
                                            source: "mode-entry-continuity".into(),
                                            held_reason: Some("mode_entry".into()),
                                            provenance: None,
                                        },
                                    );
                                }
                            }
                        }
                    } else {
                        for f in snapshot.fixtures {
                            for a in f.attributes {
                                self.hold
                                    .insert((f.fixture.clone(), a.attribute), a.resolved);
                            }
                        }
                        self.automatic.clear();
                    }
                    self.mode = mode;
                }
            }
            Command::ReplacePatch(spec) => {
                if spec.patch_revision <= self.patch.revision() {
                    return Err(Error::Target);
                }
                let patch = Patch::validate(spec)?;
                for look in std::iter::once(&self.programmer)
                    .chain(std::iter::once(&self.hold))
                    .chain(self.cues.values())
                    .chain(self.palettes.values())
                    .chain(self.playing.values().map(|p| &p.look))
                {
                    validate_look(&patch, look)?;
                }
                if self
                    .fixture_masters
                    .keys()
                    .any(|id| patch.fixture(id).is_none())
                {
                    return Err(Error::Target);
                }
                self.automatic.retain(|id, v| {
                    patch
                        .fixture(id)
                        .and_then(|f| f.capability(Attribute::Intensity))
                        .is_some_and(|c| c.contains(v.value))
                });
                self.patch = patch;
            }
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: VERSION,
            show_id: self.show_id.clone(),
            epoch: self.epoch,
            revision: self.revision,
            patch_revision: self.patch.revision(),
            durability: Durability::Volatile,
            fixtures: self
                .patch
                .fixtures()
                .iter()
                .map(|f| FixtureSnapshot {
                    fixture: f.id.clone(),
                    attributes: f
                        .capabilities
                        .iter()
                        .map(|cap| {
                            let key = (f.id.clone(), cap.attribute);
                            let programmer = self.programmer.get(&key).copied();
                            let hold = self.hold.get(&key).copied();
                            let release = self
                                .transition
                                .as_ref()
                                .and_then(|t| {
                                    t.values
                                        .iter()
                                        .find(|v| v.fixture == f.id && v.attribute == cap.attribute)
                                })
                                .map(|v| self.release_value(v));
                            let stored = [
                                (StoredKind::Cue, &self.cues),
                                (StoredKind::Palette, &self.palettes),
                            ]
                            .into_iter()
                            .flat_map(|(kind, store)| {
                                let key = &key;
                                store.iter().filter_map(move |(id, look)| {
                                    look.get(key).map(|&value| StoredValue {
                                        kind,
                                        id: id.clone(),
                                        value,
                                    })
                                })
                            })
                            .collect();
                            let active: Vec<_> = self
                                .playing
                                .iter()
                                .filter_map(|(id, p)| {
                                    p.look.get(&key).map(|&v| {
                                        (
                                            id,
                                            p,
                                            if cap.attribute == Attribute::Intensity {
                                                scale(v, p.level)
                                            } else {
                                                v
                                            },
                                        )
                                    })
                                })
                                .collect();
                            let playing = active
                                .iter()
                                .map(|(id, _, value)| NamedValue {
                                    id: (*id).clone(),
                                    value: *value,
                                })
                                .collect();
                            let winning = active.iter().max_by_key(|(_, p, v)| {
                                (
                                    if cap.attribute == Attribute::Intensity {
                                        *v
                                    } else {
                                        0
                                    },
                                    p.order,
                                )
                            });
                            let auto = (cap.attribute == Attribute::Intensity)
                                .then(|| self.automatic.get(&f.id))
                                .flatten();
                            let auto_id = Id::new(
                                auto.map_or("mode-entry-continuity", |v| v.source.as_str()),
                            )
                            .expect("bounded owner identifier");
                            let auto = auto.map(|v| v.value);
                            let (source, resolved) = if let Some(v) = programmer {
                                (Source::Programmer, v)
                            } else if let Some(v) = release {
                                (Source::Release, v)
                            } else if let Some(v) = hold {
                                (Source::Hold, v)
                            } else if let Some(v) = auto {
                                (Source::Auto(auto_id.clone()), v)
                            } else if let Some((id, _, v)) = winning {
                                (Source::Playback((*id).clone()), *v)
                            } else {
                                (Source::FixtureDefault, cap.default)
                            };
                            let mut contributors: Vec<_> = active
                                .iter()
                                .map(|(id, _, v)| Contributor {
                                    source: Source::Playback((*id).clone()),
                                    value: *v,
                                    winner: programmer.is_none()
                                        && hold.is_none()
                                        && release.is_none()
                                        && auto.is_none()
                                        && if cap.attribute == Attribute::Intensity {
                                            *v == resolved
                                        } else {
                                            source == Source::Playback((*id).clone())
                                        },
                                })
                                .collect();
                            if let Some(v) = auto {
                                contributors.push(Contributor {
                                    source: Source::Auto(auto_id),
                                    value: v,
                                    winner: programmer.is_none()
                                        && hold.is_none()
                                        && release.is_none(),
                                });
                            }
                            if let Some(v) = hold {
                                contributors.push(Contributor {
                                    source: Source::Hold,
                                    value: v,
                                    winner: programmer.is_none() && release.is_none(),
                                });
                            }
                            if let Some(v) = release {
                                contributors.push(Contributor {
                                    source: Source::Release,
                                    value: v,
                                    winner: programmer.is_none(),
                                });
                            }
                            if let Some(v) = programmer {
                                contributors.push(Contributor {
                                    source: Source::Programmer,
                                    value: v,
                                    winner: true,
                                });
                            }
                            if contributors.is_empty() {
                                contributors.push(Contributor {
                                    source: Source::FixtureDefault,
                                    value: cap.default,
                                    winner: true,
                                });
                            }
                            let intensity = cap.attribute == Attribute::Intensity;
                            let final_intent = if intensity && self.blackout {
                                0
                            } else if intensity {
                                // One rational product, rounded once, preserves bounded master semantics.
                                ((i64::from(resolved)
                                    * i64::from(self.master)
                                    * i64::from(*self.fixture_masters.get(&f.id).unwrap_or(&1000))
                                    + 500_000)
                                    / 1_000_000) as i32
                            } else {
                                resolved
                            };
                            AttributeSnapshot {
                                attribute: cap.attribute,
                                programmer,
                                hold,
                                release,
                                stored,
                                playing,
                                proposal: if cap.attribute == Attribute::Intensity
                                    && self.mode == Mode::Assist
                                {
                                    self.proposal
                                } else {
                                    None
                                },
                                auto,
                                resolved,
                                final_intent,
                                source,
                                contributors,
                                inhibit: if intensity && self.blackout {
                                    Inhibit::Blackout
                                } else {
                                    Inhibit::None
                                },
                                clamped: false,
                                submitted: None,
                                observed: None,
                            }
                        })
                        .collect(),
                })
                .collect(),
        }
    }
    /// Complete static identities, including empty objects and playback levels.
    pub fn control_inventory(&self) -> serde_json::Value {
        let values = |look: &Look| {
            look.iter()
                .map(|((fixture, attribute), value)| Value {
                    fixture: fixture.clone(),
                    attribute: *attribute,
                    value: *value,
                })
                .collect::<Vec<_>>()
        };
        serde_json::json!({
            "cues": self.cues.iter().map(|(id,look)| serde_json::json!({"id":id,"values":values(look)})).collect::<Vec<_>>(),
            "palettes": self.palettes.iter().map(|(id,look)| serde_json::json!({"id":id,"values":values(look)})).collect::<Vec<_>>(),
            "playbacks":self.playing.iter().map(|(id,p)| serde_json::json!({"id":id,"level":p.level,"activation_order":p.order.to_string(),"values":values(&p.look)})).collect::<Vec<_>>(),
            "fixture_masters":self.fixture_masters.iter().map(|(id,level)| serde_json::json!({"fixture":id,"level":level})).collect::<Vec<_>>()
        })
    }
    /// Explicit durable owner data; transient programmer/playback/transition are not resumed.
    pub fn durable_state(&self) -> crate::recovery::DurableState {
        let values = |look: &Look| {
            look.iter()
                .map(|((fixture, attribute), value)| Value {
                    fixture: fixture.clone(),
                    attribute: *attribute,
                    value: *value,
                })
                .collect::<Vec<_>>()
        };
        crate::recovery::DurableState {
            format: "shr-lux-checkpoint".into(),
            version: 1,
            show_id: self.show_id.clone(),
            epoch: self.epoch,
            revision: self.revision,
            patch: PatchSpec {
                version: VERSION,
                patch_revision: self.patch.revision(),
                fixtures: self.patch.fixtures().to_vec(),
            },
            stores: [
                (StoredKind::Cue, &self.cues),
                (StoredKind::Palette, &self.palettes),
            ]
            .into_iter()
            .flat_map(|(kind, store)| {
                store
                    .iter()
                    .map(move |(id, look)| crate::recovery::SavedLook {
                        kind,
                        id: id.clone(),
                        values: values(look),
                    })
            })
            .collect(),
            manual_hold: values(&self.hold),
            intended_look: self
                .snapshot()
                .fixtures
                .into_iter()
                .flat_map(|f| {
                    f.attributes.into_iter().map(move |a| Value {
                        fixture: f.fixture.clone(),
                        attribute: a.attribute,
                        value: a.resolved,
                    })
                })
                .collect(),
            master: self.master,
            fixture_masters: self
                .fixture_masters
                .iter()
                .map(|(fixture, level)| crate::recovery::SavedMaster {
                    fixture: fixture.clone(),
                    level: *level,
                })
                .collect(),
            blackout: self.blackout,
        }
    }
    /// Validate every source/capability before constructing a fresh disarmed epoch.
    pub fn restore_durable(
        state: &crate::recovery::DurableState,
        show: &str,
        epoch: u64,
    ) -> Result<Self, Error> {
        if epoch <= state.epoch {
            return Err(Error::Target);
        }
        let patch = state.validate(show).map_err(|_| Error::Target)?;
        let mut next = Self::new(patch, show, epoch)?.with_stores(
            state
                .stores
                .iter()
                .map(|s| StoredLook {
                    kind: s.kind,
                    id: s.id.clone(),
                    values: s.values.clone(),
                })
                .collect(),
        )?;
        // Freeze actual checkpointed pre-master current look into human Hold.
        // Old programmer, active playbacks and release are deliberately absent.
        next.hold = state
            .intended_look
            .iter()
            .map(|v| ((v.fixture.clone(), v.attribute), v.value))
            .collect();
        validate_look(&next.patch, &next.hold)?;
        next.master = state.master;
        next.blackout = state.blackout;
        next.fixture_masters = state
            .fixture_masters
            .iter()
            .map(|m| (m.fixture.clone(), m.level))
            .collect();
        next.snapshot().validate_for_schema(
            &next.patch,
            if next.analysis_enabled {
                "lx05-v1"
            } else {
                "lx03-v1"
            },
        )?;
        Ok(next)
    }
    /// Preview-only E05 token. Caller cannot supply a destination or forge token fields.
    /// Engine-created typed token; timing is injected, never wall-clock derived here.
    pub fn preview_release(&self, targets: &[Value]) -> Result<ReleasePreview, Error> {
        self.patch.validate_values(targets)?;
        if targets.is_empty() {
            return Err(Error::Target);
        }
        if targets.iter().any(|v| v.attribute != Attribute::Intensity) {
            return Err(Error::Unavailable);
        }
        let mut destination = self.clone();
        for t in targets {
            let key = (t.fixture.clone(), t.attribute);
            if !self.hold.contains_key(&key) || self.programmer.contains_key(&key) {
                return Err(Error::Target);
            }
            destination.hold.remove(&key);
        }
        let before = self.snapshot();
        let after = destination.snapshot();
        let values = targets
            .iter()
            .map(|t| {
                let get = |s: &Snapshot| {
                    s.fixtures
                        .iter()
                        .find(|f| f.fixture == t.fixture)
                        .unwrap()
                        .attributes
                        .iter()
                        .find(|a| a.attribute == t.attribute)
                        .unwrap()
                        .resolved
                };
                ReleaseDestination {
                    fixture: t.fixture.clone(),
                    attribute: t.attribute,
                    current: get(&before),
                    destination: get(&after),
                }
            })
            .collect();
        if self.transition.is_some() {
            return Err(Error::Unavailable);
        }
        let expiry_tick = self.tick.checked_add(200).ok_or(Error::Capacity)?;
        Ok(ReleasePreview {
            issued_tick: self.tick,
            expiry_tick,
            show_id: self.show_id.clone(),
            epoch: self.epoch,
            revision: self.revision,
            patch_revision: self.patch.revision(),
            values,
            transition_ms: 500,
            validity_ms: 2000,
        })
    }
    pub fn preview_is_current(&self, token: &ReleasePreview) -> bool {
        token.show_id == self.show_id
            && token.epoch == self.epoch
            && token.revision == self.revision
            && token.patch_revision == self.patch.revision()
            && self.tick < token.expiry_tick
            && self.tick >= token.issued_tick
            && self.transition.is_none()
    }
    pub fn tick(&self) -> u64 {
        self.tick
    }
    fn release_value(&self, target: &ReleaseDestination) -> i32 {
        let elapsed = self
            .transition
            .as_ref()
            .map_or(0, |t| self.tick.saturating_sub(t.start_tick).min(50));
        let numerator = i64::from(target.current) * (50 - elapsed) as i64
            + i64::from(target.destination) * elapsed as i64;
        if numerator >= 0 {
            ((numerator + 25) / 50) as i32
        } else {
            -(((-numerator + 25) / 50) as i32)
        }
    }
    /// Inject monotonic10ms logical ticks. Late updates compute position once.
    pub fn advance(&mut self, tick: u64) -> Result<(), Error> {
        if tick < self.tick {
            return Err(Error::Range);
        }
        let mut next = self.clone();
        next.tick = tick;
        if let Some(transition) = next.transition.clone() {
            if tick >= transition.end_tick {
                next.transition = None;
            }
            if next.snapshot().fixtures != self.snapshot().fixtures
                || next.transition != self.transition
            {
                next.revision = next.revision.checked_add(1).ok_or(Error::Capacity)?;
                next.snapshot().validate_for_schema(
                    &next.patch,
                    if next.analysis_enabled {
                        "lx05-v1"
                    } else {
                        "lx03-v1"
                    },
                )?;
            }
        }
        *self = next;
        Ok(())
    }
    pub fn release_commit(&mut self, token: &ReleasePreview) -> Result<(), Error> {
        if !self.preview_is_current(token) {
            return Err(Error::Target);
        }
        // Recompute destination through real source validation, never caller DTOs.
        let targets = token
            .values
            .iter()
            .map(|v| Value {
                fixture: v.fixture.clone(),
                attribute: v.attribute,
                value: v.current,
            })
            .collect::<Vec<_>>();
        let checked = self.preview_release(&targets)?;
        if checked.values != token.values {
            return Err(Error::Target);
        }
        let end = self.tick.checked_add(50).ok_or(Error::Capacity)?;
        let mut next = self.clone();
        for target in &token.values {
            next.hold
                .remove(&(target.fixture.clone(), target.attribute));
        }
        next.transition = Some(ReleaseTransition {
            start_tick: self.tick,
            end_tick: end,
            values: token.values.clone(),
        });
        next.revision = next.revision.checked_add(1).ok_or(Error::Capacity)?;
        next.snapshot().validate_for_schema(
            &next.patch,
            if next.analysis_enabled {
                "lx05-v1"
            } else {
                "lx03-v1"
            },
        )?;
        *self = next;
        Ok(())
    }
    /// Current, target and progress are logical intent; physical status is unknown.
    pub fn transition_status(&self) -> serde_json::Value {
        match &self.transition {
            None => serde_json::Value::Null,
            Some(t) => {
                serde_json::json!({"start_tick":t.start_tick.to_string(),"end_tick":t.end_tick.to_string(),"current_tick":self.tick.to_string(),"progress_ticks":self.tick.saturating_sub(t.start_tick).min(50),"duration_ticks":50,"targets":t.values.iter().map(|v|serde_json::json!({"fixture":v.fixture,"attribute":v.attribute,"current":self.release_value(v),"start":v.current,"target":v.destination})).collect::<Vec<_>>(),"physical":"unknown"})
            }
        }
    }
}
fn bounded(level: i32) -> Result<(), Error> {
    if (0..=1000).contains(&level) {
        Ok(())
    } else {
        Err(Error::Range)
    }
}
fn scale(value: i32, level: i32) -> i32 {
    (value * level + 500) / 1000
}
fn validate_look(patch: &Patch, look: &Look) -> Result<(), Error> {
    // Transaction capacity applies to incoming edits; accumulated masks can be larger.
    for f in patch.fixtures() {
        let values: Vec<_> = look
            .iter()
            .filter(|((id, _), _)| *id == f.id)
            .map(|((id, attribute), value)| Value {
                fixture: id.clone(),
                attribute: *attribute,
                value: *value,
            })
            .collect();
        patch.validate_values(&values)?;
    }
    if look.keys().any(|(id, _)| patch.fixture(id).is_none()) {
        return Err(Error::Target);
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReleaseDestination {
    pub fixture: Id,
    pub attribute: Attribute,
    pub current: i32,
    pub destination: i32,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReleasePreview {
    #[serde(with = "crate::wire::counter")]
    issued_tick: u64,
    #[serde(with = "crate::wire::counter")]
    expiry_tick: u64,
    show_id: String,
    #[serde(with = "crate::wire::counter")]
    epoch: u64,
    #[serde(with = "crate::wire::counter")]
    revision: u64,
    #[serde(with = "crate::wire::counter")]
    patch_revision: u64,
    values: Vec<ReleaseDestination>,
    transition_ms: u16,
    validity_ms: u16,
}
impl ReleasePreview {
    pub fn values(&self) -> &[ReleaseDestination] {
        &self.values
    }
    pub fn transition_ms(&self) -> u16 {
        self.transition_ms
    }
    /// Authority-clock validity; expiry preserves human Hold.
    pub fn validity_ms(&self) -> u16 {
        self.validity_ms
    }
}
/// Local intent observation only: never submission acknowledgement or physical light.
#[derive(Debug, Default)]
pub struct NullSink {
    last_intent: Option<Snapshot>,
}
impl NullSink {
    pub fn observe(&mut self, engine: &Authority) {
        self.last_intent = Some(engine.snapshot());
    }
    pub fn last_intent(&self) -> Option<&Snapshot> {
        self.last_intent.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLook {
    pub kind: StoredKind,
    pub id: Id,
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseTransition {
    start_tick: u64,
    end_tick: u64,
    values: Vec<ReleaseDestination>,
}
