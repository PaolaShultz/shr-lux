//! Bounded authored scene map and deterministic elapsed-time director. No I/O or arbitration.
use crate::fixture::{Attribute, Id, Patch, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Intimate,
    Floating,
    Driving,
    Building,
    OpenLift,
    Release,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub min: i32,
    pub max: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conditions {
    pub energy: Range,
    pub density: Range,
    pub trend: Range,
    pub min_confidence: i32,
}
impl Conditions {
    fn accepts(&self, e: &Evidence, trend: i32) -> bool {
        e.confidence >= self.min_confidence
            && (self.energy.min..=self.energy.max).contains(&e.energy)
            && (self.density.min..=self.density.max).contains(&e.density)
            && (self.trend.min..=self.trend.max).contains(&trend)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub id: Id,
    pub fixtures: Vec<Id>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleLook {
    pub role: Id,
    pub rgb: [i32; 3],
    pub intensity: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub id: Id,
    pub label: String,
    pub family: Family,
    pub mood: String,
    pub palette: String,
    pub lift: i32,
    pub roles: Vec<RoleLook>,
    pub conditions: Conditions,
    pub compatible_from: Vec<Id>,
    pub fade_ms: u64,
    pub min_dwell_ms: u64,
    pub confirm_ms: u64,
    pub variation: i32,
    pub repetition_penalty: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShowMap {
    pub version: u16,
    #[serde(with = "crate::wire::counter")]
    pub revision: u64,
    pub roles: Vec<Role>,
    pub scenes: Vec<Scene>,
}
impl ShowMap {
    pub fn validate(&self, patch: &Patch) -> Result<(), &'static str> {
        if self.version != 1 || self.revision == 0 {
            return Err("scene map version/revision");
        }
        if self.roles.is_empty()
            || self.roles.len() > 16
            || self.scenes.is_empty()
            || self.scenes.len() > 32
        {
            return Err("scene map capacity");
        }
        let mut roles = BTreeSet::new();
        let mut fixtures = BTreeSet::new();
        for role in &self.roles {
            if !roles.insert(&role.id) || role.fixtures.is_empty() || role.fixtures.len() > 32 {
                return Err("scene role duplicate/capacity");
            }
            for id in &role.fixtures {
                if !fixtures.insert(id) || patch.fixture(id).is_none() {
                    return Err("scene role fixture");
                }
            }
        }
        let mut ids = BTreeSet::new();
        for s in &self.scenes {
            if !ids.insert(&s.id) {
                return Err("scene duplicate");
            }
            for label in [&s.label, &s.mood, &s.palette] {
                if label.is_empty() || label.len() > 64 || label.chars().any(char::is_control) {
                    return Err("scene label");
                }
            }
            if !(0..=1000).contains(&s.lift)
                || !(0..=100).contains(&s.variation)
                || !(0..=1000).contains(&s.repetition_penalty)
                || !(100..=10000).contains(&s.fade_ms)
                || !(500..=60000).contains(&s.min_dwell_ms)
                || !(100..=10000).contains(&s.confirm_ms)
            {
                return Err("scene timing/range");
            }
            for (r, min, max) in [
                (&s.conditions.energy, 0, 1000),
                (&s.conditions.density, 0, 1000),
                (&s.conditions.trend, -1000, 1000),
            ] {
                if r.min < min || r.max > max || r.min > r.max {
                    return Err("scene condition");
                }
            }
            if !(0..=1000).contains(&s.conditions.min_confidence)
                || s.roles.is_empty()
                || s.roles.len() > 16
                || s.compatible_from.len() > 32
            {
                return Err("scene conditions/capacity");
            }
            let mut used = BTreeSet::new();
            for look in &s.roles {
                if !used.insert(&look.role)
                    || !roles.contains(&look.role)
                    || !(0..=1000).contains(&look.intensity)
                    || look.rgb.iter().any(|v| !(0..=1000).contains(v))
                {
                    return Err("scene look");
                }
            }
            self.values(&s.id, patch)?;
        }
        for s in &self.scenes {
            let mut used = BTreeSet::new();
            for id in &s.compatible_from {
                if !ids.contains(id) || !used.insert(id) {
                    return Err("scene compatibility");
                }
            }
        }
        Ok(())
    }
    pub fn scene(&self, id: &Id) -> Option<&Scene> {
        self.scenes.iter().find(|s| &s.id == id)
    }
    pub fn values(&self, id: &Id, patch: &Patch) -> Result<Vec<Value>, &'static str> {
        let scene = self.scene(id).ok_or("scene target")?;
        let mut values = Vec::new();
        for look in &scene.roles {
            let role = self
                .roles
                .iter()
                .find(|r| r.id == look.role)
                .ok_or("scene role")?;
            for fixture in &role.fixtures {
                let f = patch.fixture(fixture).ok_or("scene fixture")?;
                if f.capability(Attribute::Intensity).is_none() {
                    return Err("scene intensity unavailable");
                }
                values.push(Value {
                    fixture: fixture.clone(),
                    attribute: Attribute::Intensity,
                    value: look.intensity * scene.lift / 1000,
                });
                let rgb = [Attribute::Red, Attribute::Green, Attribute::Blue];
                let count = rgb.iter().filter(|a| f.capability(**a).is_some()).count();
                if count != 0 && count != 3 {
                    return Err("scene RGB unavailable");
                }
                for (attribute, value) in rgb.into_iter().zip(look.rgb) {
                    if f.capability(attribute).is_some() {
                        values.push(Value {
                            fixture: fixture.clone(),
                            attribute,
                            value,
                        })
                    }
                }
            }
        }
        Ok(values)
    }
    pub fn defaults(patch: &Patch) -> Self {
        let role = Id::new("stage").unwrap();
        let families = [
            Family::Intimate,
            Family::Floating,
            Family::Driving,
            Family::Building,
            Family::OpenLift,
            Family::Release,
        ];
        let names = [
            "intimate",
            "floating",
            "driving",
            "building",
            "open_lift",
            "release",
        ];
        let colors = [
            [1000, 380, 170],
            [230, 410, 1000],
            [850, 220, 90],
            [630, 230, 780],
            [1000, 720, 380],
            [200, 460, 760],
        ];
        let ranges = [
            (0, 210, -1000, 150),
            (160, 430, -120, 120),
            (400, 1000, -120, 120),
            (220, 800, 90, 1000),
            (750, 1000, 150, 1000),
            (180, 900, -1000, -90),
        ];
        Self {
            version: 1,
            revision: 1,
            roles: vec![Role {
                id: role.clone(),
                fixtures: patch.fixtures().iter().map(|f| f.id.clone()).collect(),
            }],
            scenes: names
                .into_iter()
                .enumerate()
                .map(|(i, name)| {
                    let (a, b, c, d) = ranges[i];
                    Scene {
                        id: Id::new(name).unwrap(),
                        label: name.replace('_', " "),
                        family: families[i],
                        mood: "authored".into(),
                        palette: format!("palette-{name}"),
                        lift: [300, 450, 650, 600, 900, 400][i],
                        roles: vec![RoleLook {
                            role: role.clone(),
                            rgb: colors[i],
                            intensity: 1000,
                        }],
                        conditions: Conditions {
                            energy: Range { min: a, max: b },
                            density: Range { min: 0, max: 1000 },
                            trend: Range { min: c, max: d },
                            min_confidence: 600,
                        },
                        compatible_from: vec![],
                        fade_ms: 1200,
                        min_dwell_ms: 2500,
                        confirm_ms: 600,
                        variation: 20,
                        repetition_penalty: 80,
                    }
                })
                .collect(),
        }
    }
}
/// Local arrangement identity support is independent of tentative semantic labels.
pub fn section_identity(snapshot: crate::musical::MusicalSnapshot) -> Option<u32> {
    snapshot
        .section
        .filter(|s| snapshot.ready && (s.novelty <= 0.18 || s.recurrence >= 0.4))
        .map(|s| s.id)
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Evidence {
    pub available: bool,
    pub energy: i32,
    pub density: i32,
    pub confidence: i32,
    pub section_id: Option<u32>,
    pub harmonic_relation: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Interpretation {
    pub trend: i32,
    pub change: String,
    pub uncertain: bool,
}
impl Default for Interpretation {
    fn default() -> Self {
        Self {
            trend: 0,
            change: "unknown".into(),
            uncertain: true,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Transition {
    pub from: Option<Id>,
    pub to: Id,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub progress: i32,
}
fn serialize_map<S: serde::Serializer>(
    map: &std::sync::Arc<ShowMap>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    map.as_ref().serialize(serializer)
}
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    #[serde(serialize_with = "serialize_map")]
    pub map: std::sync::Arc<ShowMap>,
    pub evidence: Evidence,
    pub interpretation: Interpretation,
    pub current: Option<Id>,
    pub proposed: Option<Id>,
    pub reason: String,
    pub transition: Option<Transition>,
    pub intended_values: Vec<Value>,
    pub variation: i32,
}
#[derive(Clone)]
pub struct Director {
    pub status: Status,
    history: VecDeque<(u64, i32)>,
    sections: BTreeMap<u32, Id>,
    section_order: VecDeque<u32>,
    recent: VecDeque<Id>,
    pending: Option<(Id, u64)>,
    current_since: u64,
    current_energy: i32,
    current_section: Option<u32>,
    from_values: Vec<Value>,
}
impl Director {
    pub fn new(map: ShowMap) -> Self {
        Self {
            status: Status {
                map: std::sync::Arc::new(map),
                evidence: Evidence::default(),
                interpretation: Interpretation::default(),
                current: None,
                proposed: None,
                reason: "waiting_for_evidence".into(),
                transition: None,
                intended_values: vec![],
                variation: 0,
            },
            history: VecDeque::new(),
            sections: BTreeMap::new(),
            section_order: VecDeque::new(),
            recent: VecDeque::new(),
            pending: None,
            current_since: 0,
            current_energy: 0,
            current_section: None,
            from_values: vec![],
        }
    }
    pub fn reset_source(&mut self) {
        self.status.evidence = Evidence::default();
        self.status.interpretation = Interpretation::default();
        self.status.variation = 0;
        self.status.current = None;
        self.status.intended_values.clear();
        self.recent.clear();
        self.current_section = None;
        self.history.clear();
        self.sections.clear();
        self.section_order.clear();
        self.pending = None;
        self.status.proposed = None;
        self.status.transition = None;
        self.status.reason = "source_unavailable".into();
    }
    pub fn step(
        &mut self,
        e: Evidence,
        now: u64,
        patch: &Patch,
        actual: &[Value],
    ) -> Result<(), &'static str> {
        self.status.evidence = e.clone();
        if !e.available || e.confidence < 600 {
            self.pending = None;
            self.status.transition = None;
            self.status.intended_values = actual
                .iter()
                .filter(|v| {
                    matches!(
                        v.attribute,
                        Attribute::Intensity | Attribute::Red | Attribute::Green | Attribute::Blue
                    )
                })
                .cloned()
                .collect();
            self.status.proposed = None;
            self.status.interpretation = Interpretation {
                uncertain: true,
                change: "unknown".into(),
                trend: 0,
            };
            self.status.reason = "uncertain_hold".into();
            return Ok(());
        }
        while self
            .history
            .front()
            .is_some_and(|(t, _)| now.saturating_sub(*t) > 4500)
        {
            self.history.pop_front();
        }
        let trend = self
            .history
            .front()
            .map_or(0, |(_, v)| (e.energy - *v).clamp(-1000, 1000));
        if self
            .history
            .back()
            .is_none_or(|(t, _)| now.saturating_sub(*t) >= 150)
        {
            if self.history.len() == 32 {
                self.history.pop_front();
            }
            self.history.push_back((now, e.energy));
        }
        self.status.interpretation = Interpretation {
            trend,
            change: if trend > 90 {
                "building"
            } else if trend < -90 {
                "release"
            } else {
                "stable"
            }
            .into(),
            uncertain: false,
        };
        if let Some(t) = &mut self.status.transition {
            t.progress = (now.saturating_sub(t.start_ms) * 1000 / t.duration_ms).min(1000) as i32;
            let mut target = self.status.map.values(&t.to, patch)?;
            for v in &mut target {
                if v.attribute == Attribute::Red {
                    v.value = (v.value + self.status.variation).min(1000)
                } else if v.attribute == Attribute::Blue {
                    v.value = (v.value - self.status.variation).max(0)
                }
            }
            self.status.intended_values = target
                .into_iter()
                .map(|mut v| {
                    let start = self
                        .from_values
                        .iter()
                        .find(|a| a.fixture == v.fixture && a.attribute == v.attribute)
                        .map_or(0, |a| a.value);
                    v.value = start + (v.value - start) * t.progress / 1000;
                    v
                })
                .collect();
            if t.progress < 1000 {
                self.status.reason = "finishing_fade".into();
                return Ok(());
            }
            self.status.transition = None;
        }
        let recurring = e.section_id.and_then(|id| self.sections.get(&id));
        let mut best: Option<(&Scene, i32)> = None;
        for s in &self.status.map.scenes {
            let c = &s.conditions;
            if !c.accepts(&e, trend) {
                continue;
            }
            if !s.compatible_from.is_empty()
                && self
                    .status
                    .current
                    .as_ref()
                    .is_some_and(|id| !s.compatible_from.contains(id))
            {
                continue;
            }
            let score = 1000 - (e.energy - (c.energy.min + c.energy.max) / 2).abs()
                + if self.status.current.as_ref() == Some(&s.id) {
                    80
                } else {
                    0
                }
                + if recurring == Some(&s.id) { 500 } else { 0 }
                - self.recent.iter().filter(|id| *id == &s.id).count() as i32
                    * s.repetition_penalty;
            if best.is_none_or(|(old, v)| score > v || (score == v && s.id < old.id)) {
                best = Some((s, score))
            }
        }
        let Some((s, _)) = best else {
            self.pending = None;
            self.status.proposed = None;
            self.status.reason = "no_eligible_scene".into();
            return Ok(());
        };
        let s = s.clone();
        self.status.proposed = Some(s.id.clone());
        if self.status.current.as_ref() == Some(&s.id) {
            self.pending = None;
            self.status.reason = "current_scene_fits".into();
            if let Some(id) = e.section_id
                && !matches!(
                    s.family,
                    Family::Building | Family::Release | Family::OpenLift
                )
            {
                self.remember(id, s.id);
            }
            return Ok(());
        }
        if self
            .status
            .current
            .as_ref()
            .and_then(|id| self.status.map.scene(id))
            .is_some_and(|s| now.saturating_sub(self.current_since) < s.min_dwell_ms)
        {
            self.pending = None;
            self.status.reason = "minimum_dwell".into();
            return Ok(());
        }
        if self
            .status
            .current
            .as_ref()
            .and_then(|id| self.status.map.scene(id))
            .is_some_and(|s| s.conditions.accepts(&e, trend))
            && (e.energy - self.current_energy).abs() < 80
            && e.section_id == self.current_section
            && trend.abs() < 90
        {
            self.pending = None;
            self.status.reason = "stable_evidence_hold".into();
            return Ok(());
        }
        if self.pending.as_ref().is_none_or(|(id, _)| id != &s.id) {
            self.pending = Some((s.id.clone(), now));
        }
        if now.saturating_sub(self.pending.as_ref().unwrap().1) < s.confirm_ms {
            self.status.reason = "confirming_change".into();
            return Ok(());
        }
        let from = self.status.current.replace(s.id.clone());
        self.current_since = now;
        self.current_energy = e.energy;
        self.current_section = e.section_id;
        self.pending = None;
        self.from_values = actual.to_vec();
        self.status.transition = Some(Transition {
            from,
            to: s.id.clone(),
            start_ms: now,
            duration_ms: match e.harmonic_relation.as_deref() {
                Some("fifthsforward" | "fifthsbackward" | "stationary") => s.fade_ms,
                Some("other") => (s.fade_ms + s.fade_ms / 4).min(10000),
                _ => s.fade_ms,
            },
            progress: 0,
        });
        self.status.intended_values = actual
            .iter()
            .filter(|v| {
                matches!(
                    v.attribute,
                    Attribute::Intensity | Attribute::Red | Attribute::Green | Attribute::Blue
                )
            })
            .cloned()
            .collect();
        self.status.variation = if recurring == Some(&s.id) {
            0
        } else {
            (self.recent.len() as i32 * 17) % (s.variation + 1)
        };
        if self.recent.len() == 8 {
            self.recent.pop_front();
        }
        self.recent.push_back(s.id.clone());

        self.status.reason = "confirmed_change".into();
        Ok(())
    }
    fn remember(&mut self, id: u32, scene: Id) {
        if !self.sections.contains_key(&id) {
            if self.sections.len() == 16
                && let Some(oldest) = self.section_order.pop_front()
            {
                self.sections.remove(&oldest);
            }
            self.section_order.push_back(id);
            self.sections.insert(id, scene);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{FixtureSpec, PatchSpec, SyntheticMode};
    #[test]
    fn bounded_section_memory_continues_learning_after_sixteen_sections() {
        let patch = Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![FixtureSpec::synthetic(
                Id::new("wash").unwrap(),
                "Wash",
                SyntheticMode::Rgb,
                1,
            )],
        })
        .unwrap();
        let mut d = Director::new(ShowMap::defaults(&patch));
        for now in (0..4000).step_by(100) {
            d.step(
                Evidence {
                    available: true,
                    energy: 100,
                    confidence: 1000,
                    ..Evidence::default()
                },
                now,
                &patch,
                &[],
            )
            .unwrap();
        }
        for section in 1..=20 {
            d.step(
                Evidence {
                    available: true,
                    energy: 100,
                    confidence: 1000,
                    section_id: Some(section),
                    ..Evidence::default()
                },
                4000 + u64::from(section) * 100,
                &patch,
                &[],
            )
            .unwrap();
        }
        assert_eq!(d.sections.len(), 16);
        assert!(!d.sections.contains_key(&1));
        assert!(d.sections.contains_key(&20));
        d.reset_source();
        assert!(d.sections.is_empty());
        assert!(d.section_order.is_empty());
    }
}
