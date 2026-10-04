//! Strict C-LIGHT:1 owner codec and scoped null-output service. No hardware I/O.
use crate::authority::{Authority, Command, ReleasePreview};
use crate::fixture::{Error, Id, PatchSpec, Value};
use crate::lighting_contract::{MAX_DEPTH, MAX_MESSAGE_BYTES, StoredKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const SCOPE: &str = "lighting-control";
pub const WRITER_HISTORY: usize = 1024;
pub const LEASE_TICKS: u64 = 200;
pub const CACHE: usize = 64;
/// Canonical u64 decimal strings. Numeric JSON, leading zeros and overflow refuse.
pub mod counter {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let s = String::deserialize(d)?;
        parse(&s).map_err(serde::de::Error::custom)
    }
    pub fn parse(s: &str) -> Result<u64, &'static str> {
        if s.is_empty()
            || (s.len() > 1 && s.starts_with('0'))
            || !s.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("range");
        }
        s.parse().map_err(|_| "range")
    }
}

/// A visitor rejects duplicate keys before serde_json can collapse them.
struct Strict(Json);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Strict;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("bounded JSON")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(Json::Bool(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Strict, E> {
                Err(E::custom("integer required"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Json::Null))
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Strict, E> {
                self.visit_unit()
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut values = Vec::new();
                while let Some(Strict(v)) = a.next_element()? {
                    if values.len() >= 1024 {
                        return Err(serde::de::Error::custom("capacity"));
                    }
                    values.push(v);
                }
                Ok(Strict(Json::Array(values)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if values.len() >= 128 || values.contains_key(&k) {
                        return Err(serde::de::Error::custom("duplicate/capacity"));
                    }
                    let Strict(v) = a.next_value()?;
                    values.insert(k, v);
                }
                Ok(Strict(Json::Object(values)))
            }
        }
        d.deserialize_any(Visitor)
    }
}
pub fn strict_json(bytes: &[u8]) -> Result<Json, &'static str> {
    strict_json_bounded(bytes, MAX_MESSAGE_BYTES)
}
pub(crate) fn strict_json_bounded(bytes: &[u8], limit: usize) -> Result<Json, &'static str> {
    if bytes.len() > limit {
        return Err("capacity");
    }
    // Preflight nesting without allocating or recursing; ignore escaped quoted characters.
    let (mut depth, mut quoted, mut escape) = (0usize, false, false);
    for &b in bytes {
        if quoted {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > MAX_DEPTH {
                        return Err("depth");
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    serde_json::from_slice::<Strict>(bytes)
        .map(|v| v.0)
        .map_err(|_| "malformed")
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub contract: String,
    pub version: u16,
    pub show_id: String,
    pub module: String,
    pub epoch: String,
    pub writer: Option<Id>,
    pub lease: Option<String>,
    pub request_id: Option<String>,
    pub expected_revision: Option<String>,
    pub kind: String,
    pub body: Json,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Touch {
        values: Vec<Value>,
    },
    ClearToHold,
    Record {
        kind: Store,
        id: Id,
    },
    Update {
        kind: Store,
        id: Id,
    },
    ApplyPalette {
        id: Id,
    },
    Go {
        cue: Id,
        playback: Id,
    },
    Level {
        playback: Id,
        level: i32,
    },
    Off {
        playback: Id,
    },
    Master {
        level: i32,
    },
    FixtureMaster {
        fixture: Id,
        level: i32,
    },
    Blackout {
        enabled: bool,
    },
    Mode {
        mode: String,
    },
    ReplacePatch {
        patch: PatchSpec,
    },
    ReleasePreview {
        values: Vec<Value>,
    },
    ReleaseCommit {
        token: Json,
    },
    ReleaseCancel,
    Checkpoint,
    AnalysisCalibrate {
        phase: String,
    },
    AnalysisGrant {
        fixtures: Vec<Id>,
        cap: i32,
        ttl_ms: u64,
    },
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Store {
    Cue,
    Palette,
}
impl From<Store> for StoredKind {
    fn from(v: Store) -> Self {
        match v {
            Store::Cue => Self::Cue,
            Store::Palette => Self::Palette,
        }
    }
}
impl Action {
    fn command(self) -> Result<Command, &'static str> {
        Ok(match self {
            Self::Touch { values } => Command::Touch(values),
            Self::ClearToHold => Command::ClearToHold,
            Self::Record { kind, id } => Command::Record {
                kind: kind.into(),
                id,
            },
            Self::Update { kind, id } => Command::Update {
                kind: kind.into(),
                id,
            },
            Self::ApplyPalette { id } => Command::ApplyPalette(id),
            Self::Go { cue, playback } => Command::Go { cue, playback },
            Self::Level { playback, level } => Command::Level { playback, level },
            Self::Off { playback } => Command::Off(playback),
            Self::Master { level } => Command::Master(level),
            Self::FixtureMaster { fixture, level } => Command::FixtureMaster { fixture, level },
            Self::Blackout { enabled } => Command::Blackout(enabled),
            Self::Mode { mode } => Command::Mode(match mode.as_str() {
                "manual" => crate::authority::Mode::Manual,
                "assist" => crate::authority::Mode::Assist,
                "auto" => crate::authority::Mode::Auto,
                _ => return Err("unavailable"),
            }),
            Self::ReplacePatch { patch } => Command::ReplacePatch(patch),
            Self::ReleasePreview { .. }
            | Self::ReleaseCommit { .. }
            | Self::ReleaseCancel
            | Self::AnalysisCalibrate { .. }
            | Self::AnalysisGrant { .. }
            | Self::Checkpoint => return Err("unavailable"),
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scoped {
    scope: String,
}
#[derive(Clone)]
struct Session {
    lease: u64,
    expiry: u64,
    highwater: u64,
    cache: VecDeque<(u64, Json, Json)>,
}
#[derive(Clone)]
struct AnalysisGrant {
    writer: Id,
    lease: u64,
    issue_revision: u64,
    patch: u64,
    generation: u64,
    expiry: u64,
    fixtures: Vec<Id>,
    cap: i32,
}
/// One scope, one active writer, bounded never-forgotten identities per epoch.
#[derive(Clone)]
pub struct Service {
    engine: Authority,
    analysis: Option<crate::analysis_subscription::AnalysisState>,
    analysis_grant: Option<AnalysisGrant>,
    last_analysis_step: Option<u64>,
    analysis_now: u64,
    auto_held_reason: Option<String>,
    analysis_emergency: bool,
    tick: u64,
    sequence: u64,
    next_lease: u64,
    sessions: BTreeMap<Id, Session>,
    retired: BTreeSet<Id>,
    denied_grants: BTreeMap<Id, (Json, Json)>,
    timed: bool,
    preview: Option<PreviewEntry>,
    store: Option<std::sync::Arc<crate::recovery::CheckpointStore>>,
    epoch_owner: Option<std::sync::Arc<crate::recovery::CheckpointStore>>,
    durability: crate::lighting_contract::Durability,
}
impl Service {
    pub fn new(engine: Authority) -> Result<Self, &'static str> {
        let service = Self {
            engine,
            analysis: None,
            analysis_grant: None,
            last_analysis_step: None,
            analysis_now: 0,
            auto_held_reason: None,
            analysis_emergency: false,
            tick: 0,
            sequence: 0,
            next_lease: 0,
            sessions: BTreeMap::new(),
            retired: BTreeSet::new(),
            denied_grants: BTreeMap::new(),
            timed: false,
            preview: None,
            store: None,
            epoch_owner: None,
            durability: crate::lighting_contract::Durability::Volatile,
        };
        service.pages()?;
        Ok(service)
    }
    pub fn timed(engine: Authority) -> Result<Self, &'static str> {
        let mut service = Self::new(engine)?;
        service.timed = true;
        service.pages()?;
        Ok(service)
    }
    /// Epoch reservation is already durable and the owner lock remains retained.
    /// Volatile scenes still retain a durably reserved never-reused listener epoch.
    pub fn reserved(
        store: std::sync::Arc<crate::recovery::CheckpointStore>,
        patch: crate::fixture::Patch,
        timed: bool,
    ) -> Result<Self, &'static str> {
        let engine = Authority::new(patch, store.show_id(), store.epoch()).map_err(error_reason)?;
        let mut service = if timed {
            Self::timed(engine)?
        } else {
            Self::new(engine)?
        };
        service.epoch_owner = Some(store);
        Ok(service)
    }
    pub fn recover(
        store: std::sync::Arc<crate::recovery::CheckpointStore>,
        fallback_patch: crate::fixture::Patch,
    ) -> Result<Self, &'static str> {
        let saved = store.load().map_err(|_| "recovery")?;
        let engine = match &saved {
            Some(state) => Authority::restore_durable(state, store.show_id(), store.epoch())
                .map_err(error_reason)?,
            None => Authority::new(fallback_patch, store.show_id(), store.epoch())
                .map_err(error_reason)?,
        };
        let mut service = Self::timed(engine)?;
        service.durability = if saved.is_some() {
            crate::lighting_contract::Durability::Checkpointed
        } else {
            crate::lighting_contract::Durability::Volatile
        };
        service.store = Some(store);
        service.pages()?;
        Ok(service)
    }
    pub fn enable_analysis(&mut self) -> Result<(), &'static str> {
        let mut next = self.clone();
        next.engine.enable_analysis();
        next.analysis = Some(crate::analysis_subscription::AnalysisState::default());
        next.pages()?;
        *self = next;
        Ok(())
    }
    fn revoke_analysis(&mut self, reason: &str) -> Result<(), &'static str> {
        if self.analysis_grant.take().is_some() {
            self.engine.freeze_auto(reason).map_err(error_reason)?;
            self.auto_held_reason = Some(reason.into());
        }
        Ok(())
    }
    /// Called by the independent owner clock even with no control clients connected.
    pub fn poll_analysis(
        &mut self,
        client: Option<&crate::analysis_subscription::Client>,
        now_ms: u64,
        tick: u64,
    ) -> Result<(), &'static str> {
        self.poll_analysis_clock(
            client,
            now_ms,
            tick,
            crate::analysis_subscription::monotonic_ms,
        )
    }
    fn poll_analysis_clock(
        &mut self,
        client: Option<&crate::analysis_subscription::Client>,
        now_ms: u64,
        tick: u64,
        mut clock: impl FnMut() -> std::io::Result<u64>,
    ) -> Result<(), &'static str> {
        if self.analysis.is_none() {
            return self.advance(tick);
        }
        let mut baseline = self.clone();
        baseline.analysis_emergency = true;
        baseline.advance(tick)?;
        let mut candidate = baseline.clone();
        candidate.analysis_emergency = false;
        candidate.poll_analysis_inner(client, now_ms, tick, &mut clock)?;
        if candidate.pages().is_err() {
            baseline.analysis_emergency = true;
            if let Some(a) = &mut baseline.analysis {
                a.invalidate("snapshot_capacity");
            }
            baseline.engine.set_analysis_proposal(None);
            baseline.revoke_analysis("snapshot_capacity")?;
            baseline.last_analysis_step = Some(tick);
            baseline.analysis_now = clock().map_err(|_| "clock")?;
            baseline.pages()?;
            *self = baseline;
            return Ok(());
        }
        // Encoding a near-budget snapshot also takes time: never commit a newly stale step.
        let final_now = clock().map_err(|_| "clock")?;
        if final_now < candidate.analysis_now {
            candidate.engine = baseline.engine.clone();
            candidate.analysis_emergency = true;
            if let Some(a) = &mut candidate.analysis {
                a.invalidate("clock_regression");
            }
            candidate.engine.set_analysis_proposal(None);
            candidate.revoke_analysis("clock_regression")?;
            candidate.pages()?;
            *self = candidate;
            return Ok(());
        }
        candidate.analysis_now = final_now;
        if let Some(a) = &mut candidate.analysis {
            a.expire(final_now);
        }
        candidate
            .engine
            .set_analysis_proposal(candidate.analysis.as_ref().and_then(|a| a.proposal));
        if candidate.analysis_grant.is_some()
            && candidate.analysis.as_ref().is_none_or(|a| !a.ready())
        {
            candidate.engine = baseline.engine;
            candidate.analysis_emergency = true;
            candidate.engine.set_analysis_proposal(None);
            candidate.revoke_analysis("analysis_age_after_processing")?;
            candidate.pages()?;
        }
        *self = candidate;
        Ok(())
    }
    fn poll_analysis_inner(
        &mut self,
        client: Option<&crate::analysis_subscription::Client>,
        now_ms: u64,
        tick: u64,
        clock: &mut impl FnMut() -> std::io::Result<u64>,
    ) -> Result<(), &'static str> {
        self.tick = tick;
        self.analysis_now = now_ms;
        let Some(analysis) = &mut self.analysis else {
            return Ok(());
        };
        if let Some(client) = client {
            client.drain(analysis, now_ms);
        } else {
            analysis.expire(now_ms);
        }
        // DSP may consume the last milliseconds of freshness. Re-sample acquisition age now.
        let after = clock().map_err(|_| "clock")?;
        self.analysis_now = after;
        if after < now_ms {
            analysis.invalidate("clock_regression");
        }
        analysis.expire(self.analysis_now);
        self.engine.set_analysis_proposal(analysis.proposal);
        let invalid = self.analysis_grant.as_ref().and_then(|g| {
            if !analysis.ready() || g.generation != analysis.generation {
                Some("analysis_loss_or_change")
            } else if self.engine.mode() != crate::authority::Mode::Auto {
                Some("mode")
            } else if g.patch != self.engine.patch().revision() {
                Some("patch")
            } else if g.expiry <= tick {
                Some("expiry")
            } else if !self
                .sessions
                .get(&g.writer)
                .is_some_and(|s| s.lease == g.lease && s.expiry > tick)
            {
                Some("lighting_lease")
            } else {
                None
            }
        });
        if let Some(reason) = invalid {
            self.revoke_analysis(reason)?;
        }
        if self.last_analysis_step != Some(tick) {
            self.last_analysis_step = Some(tick);
            if let Some(g) = &self.analysis_grant {
                let proposal = self
                    .analysis
                    .as_ref()
                    .and_then(|a| a.proposal)
                    .ok_or("analysis_unavailable")?;
                let revision = self.engine.revision();
                self.engine
                    .step_auto(&g.fixtures, g.cap, proposal)
                    .map_err(error_reason)?;
                let a = self.analysis.as_ref().ok_or("analysis_unavailable")?;
                let status = a.status(self.analysis_now);
                self.engine.set_auto_provenance(&g.fixtures,&json!({"analysis_version":"lux.analysis.v1","source_identity":status["source_identity"],"source_window_range":status["source_window_range"],"calibration_generation":status["calibration_generation"],"confidence":status["confidence"]}));
                if self.engine.revision() != revision {
                    self.preview = None;
                    self.durability = crate::lighting_contract::Durability::Volatile;
                }
            }
        }
        Ok(())
    }
    pub fn analysis_state_mut(
        &mut self,
    ) -> Option<&mut crate::analysis_subscription::AnalysisState> {
        self.analysis.as_mut()
    }
    pub fn analysis_status(&self) -> Json {
        let mut status = self
            .analysis
            .as_ref()
            .map_or(Json::Null, |a| a.status(self.analysis_now));
        if !status.is_null() {
            status["grant"]=self.analysis_grant.as_ref().map_or(Json::Null,|g|json!({"writer":g.writer,"lease":g.lease.to_string(),"issue_revision":g.issue_revision.to_string(),"patch_revision":g.patch.to_string(),"analysis_generation":g.generation.to_string(),"expiry_tick":g.expiry.to_string(),"fixtures":g.fixtures,"cap":g.cap}));
            status["automatic_layer"] = self.engine.auto_inventory();
            status["held_reason"] = json!(self.auto_held_reason);
        }
        status
    }
    pub fn engine(&self) -> &Authority {
        &self.engine
    }
    pub fn tick(&self) -> u64 {
        self.tick
    }
    pub fn advance(&mut self, tick: u64) -> Result<(), &'static str> {
        if tick < self.tick {
            return Err("clock");
        }
        let mut next = self.clone();
        next.tick = tick;
        if next.timed {
            next.engine.advance(tick).map_err(error_reason)?;
            if next.engine.revision() != self.engine.revision() {
                next.durability = crate::lighting_contract::Durability::Volatile;
            }
        }
        let expired = next
            .sessions
            .iter()
            .filter(|(_, s)| s.expiry <= tick)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in expired {
            next.sessions.remove(&id);
            next.retired.insert(id);
        }
        if next.preview.as_ref().is_some_and(|p| {
            !next.engine.preview_is_current(&p.preview)
                || !next
                    .sessions
                    .get(&p.writer)
                    .is_some_and(|s| s.lease == p.lease)
        }) {
            next.preview = None;
        }
        next.pages()?;
        *self = next;
        Ok(())
    }
    fn reply(&self, kind: &str, reason: Option<&str>, request: Option<&str>, body: Json) -> Json {
        let s = self.engine.snapshot();
        json!({"contract":"C-LIGHT","version":1,"show_id":s.show_id,"module":"lighting","epoch":s.epoch.to_string(),"sequence":self.sequence.to_string(),"revision":s.revision.to_string(),"effective_tick":self.tick.to_string(),"writer":null,"lease":null,"expected_revision":null,"request_id":request,"kind":kind,"reason":reason,"body":body})
    }
    fn correlated(&self, mut reply: Json, request: &Envelope) -> Json {
        reply["writer"] = serde_json::to_value(&request.writer).unwrap_or(Json::Null);
        reply["lease"] = request
            .lease
            .as_ref()
            .filter(|s| counter::parse(s).is_ok())
            .map_or(Json::Null, |s| json!(s));
        reply["request_id"] = request
            .request_id
            .as_ref()
            .filter(|s| counter::parse(s).is_ok())
            .map_or(Json::Null, |s| json!(s));
        reply["expected_revision"] = request
            .expected_revision
            .as_ref()
            .filter(|s| counter::parse(s).is_ok())
            .map_or(Json::Null, |s| json!(s));
        reply
    }
    pub fn handle(&mut self, bytes: &[u8]) -> Vec<Json> {
        let context = strict_json(bytes)
            .ok()
            .and_then(|v| serde_json::from_value::<Envelope>(v).ok());
        match self.handle_inner(bytes) {
            Ok(v) => v,
            Err(reason) => {
                let response = self.reply(
                    if reason == "stale_revision" {
                        "conflict"
                    } else if reason == "busy" {
                        "busy"
                    } else {
                        "rejected"
                    },
                    Some(reason),
                    None,
                    Json::Null,
                );
                vec![
                    context
                        .as_ref()
                        .map_or(response.clone(), |e| self.correlated(response, e)),
                ]
            }
        }
    }
    fn handle_inner(&mut self, bytes: &[u8]) -> Result<Vec<Json>, &'static str> {
        let raw = strict_json(bytes)?;
        if raw.as_object().is_none_or(|o| o.len() != 11) {
            return Err("malformed");
        }
        let e: Envelope = serde_json::from_value(raw.clone()).map_err(|_| "malformed")?;
        let s = self.engine.snapshot();
        if e.contract != "C-LIGHT" || e.version != 1 {
            return Err("version");
        }
        if e.show_id != s.show_id {
            return Err("wrong_show");
        }
        if e.module != "lighting" {
            return Err("target");
        }
        if counter::parse(&e.epoch)? != s.epoch {
            return Err("epoch");
        }
        self.sequence = self.sequence.checked_add(1).ok_or("capacity")?;
        if matches!(
            e.kind.as_str(),
            "snapshot" | "capabilities" | "analysis_status"
        ) {
            if e.writer.is_some()
                || e.lease.is_some()
                || e.request_id.is_some()
                || e.expected_revision.is_some()
                || e.body != json!({})
            {
                return Err("malformed");
            }
            if e.kind == "analysis_status" {
                if self.analysis.is_none() {
                    return Err("unavailable");
                }
                return Ok(vec![self.reply(
                    "analysis_status",
                    None,
                    None,
                    json!({"wire_schema":"lx05-v1","analysis":self.analysis_status()}),
                )]);
            }
            return self.pages();
        }
        let writer = e.writer.clone().ok_or("lease")?;
        if e.kind == "grant" {
            return self.grant_request(&e, writer, raw);
        }
        let lease = counter::parse(e.lease.as_deref().ok_or("lease")?)?;
        let session = self.sessions.get(&writer).ok_or("lease")?;
        if session.lease != lease || session.expiry <= self.tick {
            return Err("lease");
        }
        let id = counter::parse(e.request_id.as_deref().ok_or("malformed")?)?;
        if let Some((_, old, response)) = session.cache.iter().find(|(i, _, _)| *i == id) {
            return if old == &raw {
                Ok(vec![response.clone()])
            } else {
                Err("reused_id")
            };
        }
        if id <= session.highwater {
            return Err("expired_id");
        }
        if session.highwater == 0 && id != 1 {
            return Err("reused_id");
        }
        let expected = counter::parse(e.expected_revision.as_deref().ok_or("malformed")?)?;
        let outcome = if expected != self.engine.revision() {
            Err("stale_revision")
        } else {
            self.apply_request(&e, &writer)
        };
        let response = match outcome {
            Ok(body) => self.reply("applied", None, e.request_id.as_deref(), body),
            Err(reason) => self.reply(
                if reason == "stale_revision" {
                    "conflict"
                } else {
                    "rejected"
                },
                Some(reason),
                e.request_id.as_deref(),
                Json::Null,
            ),
        };
        let response = self.correlated(response, &e);
        // Retire removes the session; its identity remains permanently unavailable this epoch.
        if let Some(session) = self.sessions.get_mut(&writer) {
            session.highwater = id;
            session.cache.push_back((id, raw, response.clone()));
            if session.cache.len() > CACHE {
                session.cache.pop_front();
            }
        }
        Ok(vec![response])
    }
    fn grant_request(
        &mut self,
        e: &Envelope,
        writer: Id,
        raw: Json,
    ) -> Result<Vec<Json>, &'static str> {
        let body: Scoped = serde_json::from_value(e.body.clone()).map_err(|_| "malformed")?;
        if body.scope != SCOPE {
            return Err("scope");
        }
        if e.lease.is_some() {
            return Err("lease");
        }
        let id = counter::parse(e.request_id.as_deref().ok_or("malformed")?)?;
        let expected = counter::parse(e.expected_revision.as_deref().ok_or("malformed")?)?;
        if id != 1 {
            return Err("reused_id");
        }
        if let Some(session) = self.sessions.get(&writer) {
            if session.expiry <= self.tick {
                return Err("lease");
            }
            if let Some((_, old, response)) = session.cache.iter().find(|(i, _, _)| *i == 1) {
                return if old == &raw {
                    Ok(vec![response.clone()])
                } else {
                    Err("reused_id")
                };
            }
            return Err("expired_id");
        }
        if let Some((old, response)) = self.denied_grants.get(&writer) {
            return if old == &raw {
                Ok(vec![response.clone()])
            } else {
                Err("reused_id")
            };
        }
        if self.retired.contains(&writer) {
            return Err("lease");
        }
        if self.retired.len() + self.sessions.len() >= WRITER_HISTORY {
            return Err("capacity");
        }
        let outcome = (|| {
            if expected != self.engine.revision() {
                return Err("stale_revision");
            }
            if !self.sessions.is_empty() {
                return Err("busy");
            }
            let expiry = self.tick.checked_add(LEASE_TICKS).ok_or("capacity")?;
            let lease = self.next_lease.checked_add(1).ok_or("capacity")?;
            let mut candidate = self.clone();
            candidate.next_lease = lease;
            candidate.sessions.insert(
                writer.clone(),
                Session {
                    lease,
                    expiry,
                    highwater: 1,
                    cache: VecDeque::new(),
                },
            );
            candidate.pages()?;
            self.next_lease = candidate.next_lease;
            self.sessions = candidate.sessions;
            Ok(json!({"scope":SCOPE,"lease":lease.to_string(),"remaining_ms":2000}))
        })();
        let response = match outcome {
            Ok(body) => self.reply("applied", None, e.request_id.as_deref(), body),
            Err(reason) => self.reply(
                if reason == "stale_revision" {
                    "conflict"
                } else if reason == "busy" {
                    "busy"
                } else {
                    "rejected"
                },
                Some(reason),
                e.request_id.as_deref(),
                Json::Null,
            ),
        };
        let response = self.correlated(response, e);
        if let Some(session) = self.sessions.get_mut(&writer) {
            session.cache.push_back((1, raw, response.clone()));
        } else {
            self.retired.insert(writer.clone());
            self.denied_grants.insert(writer, (raw, response.clone()));
        }
        Ok(vec![response])
    }
    fn apply_request(&mut self, e: &Envelope, writer: &Id) -> Result<Json, &'static str> {
        if e.kind == "renew" || e.kind == "retire" {
            let body: Scoped = serde_json::from_value(e.body.clone()).map_err(|_| "malformed")?;
            if body.scope != SCOPE {
                return Err("scope");
            }
            if e.kind == "renew" {
                let expiry = self.tick.checked_add(LEASE_TICKS).ok_or("capacity")?;
                self.sessions.get_mut(writer).ok_or("lease")?.expiry = expiry;
                return Ok(json!({"remaining_ms":2000}));
            }
            if self.preview.as_ref().is_some_and(|p| &p.writer == writer) {
                self.preview = None;
            }
            self.sessions.remove(writer);
            self.retired.insert(writer.clone());
            return Ok(json!({"retired":true}));
        }
        if e.kind != "command" {
            return Err("unavailable");
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Body {
            scope: String,
            command: Json,
        }
        let body: Body = serde_json::from_value(e.body.clone()).map_err(|_| "malformed")?;
        if body.scope != SCOPE {
            return Err("scope");
        }
        let action: Action = serde_json::from_value(body.command).map_err(|_| "malformed")?;
        let mut candidate = self.clone();
        candidate.analysis_emergency = false;
        let mut result =
            json!({"application":"logical_static","output":"null_disarmed","physical":"unknown"});
        match action {
            Action::AnalysisCalibrate { phase } => {
                let now = crate::analysis_subscription::monotonic_ms().map_err(|_| "clock")?;
                candidate.analysis_now = now;
                candidate
                    .analysis
                    .as_mut()
                    .ok_or("unavailable")?
                    .calibrate(&phase, now)?;
                candidate.revoke_analysis("calibration")?;
                candidate.engine.set_analysis_proposal(None);
                result = json!({"analysis":candidate.analysis_status()});
            }
            Action::AnalysisGrant {
                fixtures,
                cap,
                ttl_ms,
            } => {
                let now = crate::analysis_subscription::monotonic_ms().map_err(|_| "clock")?;
                candidate.analysis_now = now;
                let a = candidate.analysis.as_mut().ok_or("unavailable")?;
                a.expire(now);
                if !a.ready() {
                    return Err("analysis_unavailable");
                }
                if ttl_ms == 0 || ttl_ms > 2000 || !ttl_ms.is_multiple_of(10) {
                    return Err("range");
                }
                candidate
                    .engine
                    .validate_auto_targets(&fixtures, cap)
                    .map_err(error_reason)?;
                let session = candidate.sessions.get(writer).ok_or("lease")?;
                let generation = a.generation;
                let lease = session.lease;
                candidate.revoke_analysis("replaced_grant")?;
                candidate.analysis_grant = Some(AnalysisGrant {
                    writer: writer.clone(),
                    lease,
                    issue_revision: candidate.engine.revision(),
                    patch: candidate.engine.patch().revision(),
                    generation,
                    expiry: candidate.tick.checked_add(ttl_ms / 10).ok_or("capacity")?,
                    fixtures,
                    cap,
                });
                candidate.auto_held_reason = None;
                result = json!({"analysis":candidate.analysis_status()});
            }
            Action::ReleasePreview { values } => {
                if !candidate.timed {
                    return Err("unavailable");
                }
                let preview = candidate
                    .engine
                    .preview_release(&values)
                    .map_err(error_reason)?;
                let lease = candidate.sessions.get(writer).ok_or("lease")?.lease;
                let nonce = preview_nonce()?;
                let payload = json!({"schema":"lx04-release-v1","nonce":nonce,"writer":writer,"lease":lease.to_string(),"scope":SCOPE,"preview":preview});
                result = json!({"token":payload,"application":"preview_only","physical":"unknown"});
                candidate.preview = Some(PreviewEntry {
                    writer: writer.clone(),
                    lease,
                    preview,
                    payload,
                });
            }
            Action::ReleaseCommit { token } => {
                if !candidate.timed {
                    return Err("unavailable");
                }
                let stored = candidate.preview.as_ref().ok_or("target")?;
                let lease = candidate.sessions.get(writer).ok_or("lease")?.lease;
                if &stored.writer != writer || stored.lease != lease || stored.payload != token {
                    return Err("lease");
                }
                candidate
                    .engine
                    .release_commit(&stored.preview)
                    .map_err(error_reason)?;
                candidate.preview = None;
                result = json!({"application":"logical_timed","transition":candidate.engine.transition_status(),"physical":"unknown"});
            }
            Action::ReleaseCancel => {
                if !candidate.timed {
                    return Err("unavailable");
                }
                candidate.preview = None;
                result = json!({"application":"preview_cancelled","physical":"unknown"});
            }
            Action::Checkpoint => {
                candidate.pages()?;
                let store = candidate.store.as_ref().ok_or("unavailable")?;
                let state = candidate.engine.durable_state();
                if store.checkpoint(&state).is_err() {
                    self.durability = crate::lighting_contract::Durability::Error;
                    return Err("durability");
                }
                candidate.durability = crate::lighting_contract::Durability::Checkpointed;
                result = json!({"application":"checkpointed","durability":"checkpointed","physical":"unknown"});
            }
            ordinary => {
                candidate
                    .engine
                    .execute(ordinary.command()?)
                    .map_err(error_reason)?;
                candidate.preview = None;
                if candidate.analysis_grant.as_ref().is_some_and(|g| {
                    g.patch != candidate.engine.patch().revision()
                        || candidate.engine.mode() != crate::authority::Mode::Auto
                }) {
                    candidate.revoke_analysis("mode_or_patch")?;
                }
            }
        }
        candidate.pages()?; // Coherent readable state is a transaction invariant.
        if candidate.engine.revision() != self.engine.revision() {
            candidate.durability = crate::lighting_contract::Durability::Volatile;
        }
        self.analysis_emergency = candidate.analysis_emergency;
        self.analysis = candidate.analysis;
        self.analysis_grant = candidate.analysis_grant;
        self.analysis_now = candidate.analysis_now;
        self.auto_held_reason = candidate.auto_held_reason;
        self.engine = candidate.engine;
        self.preview = candidate.preview;
        self.durability = candidate.durability;
        Ok(result)
    }
    pub fn inventory(&self) -> Result<Json, &'static str> {
        let snapshot = self.engine.snapshot();
        snapshot
            .validate_for_schema(
                self.engine.patch(),
                if self.analysis.is_some() {
                    "lx05-v1"
                } else {
                    "lx03-v1"
                },
            )
            .map_err(error_reason)?;
        let mut inventory = json!({"snapshot":snapshot,"patch":{"version":1,"patch_revision":self.engine.patch().revision().to_string(),"fixtures":self.engine.patch().fixtures()},"groups":[{"id":"all","fixtures":self.engine.patch().fixtures().iter().map(|f|f.id.clone()).collect::<Vec<_>>()}],"mode":match self.engine.mode(){crate::authority::Mode::Manual=>"manual",crate::authority::Mode::Assist=>"assist",crate::authority::Mode::Auto=>"auto"},"master":self.engine.master(),"blackout":self.engine.blackout(),"output":"null_disarmed","physical":"unknown","limits":{"fixtures":32,"playbacks":8,"cues":32,"palettes":32,"targets":64,"writers":1,"writer_history":WRITER_HISTORY,"cached_responses":64,"pages":16,"message_bytes":65536,"depth":12,"lease_ms":2000,"renew_ms":500},"authority_inventory":self.engine.control_inventory(),"grants":self.sessions.iter().map(|(writer,session)|json!({"writer":writer,"scope":SCOPE,"remaining_ms":session.expiry.saturating_sub(self.tick)*10})).collect::<Vec<_>>(),"capability_metadata":self.engine.patch().fixtures().iter().map(|f|json!({"fixture":f.id,"attributes":f.capabilities.iter().map(|c|json!({"attribute":c.attribute,"unit":c.attribute.unit(),"min":c.min,"max":c.max,"step":1,"default":c.default})).collect::<Vec<_>>(),"coherent_groups":match f.mode {crate::fixture::SyntheticMode::Dimmer=>json!([]),crate::fixture::SyntheticMode::Rgb=>json!([["red","green","blue"]]),crate::fixture::SyntheticMode::RgbPosition=>json!([["red","green","blue"],["pan","tilt"]])}})).collect::<Vec<_>>(),"snapshot_budget":"16 encoded pages of at most 65536 bytes; overbudget edits refused atomically","scope":SCOPE,"applied_auto":"unavailable"});
        if self.timed {
            inventory["wire_schema"] = json!("lx04-v1");
            inventory["release"] = json!({"transition_ms":500,"preview_validity_ms":2000,"transition":self.engine.transition_status(),"preview_available":self.preview.is_some()});
        }
        if self.store.is_some() {
            inventory["wire_schema"] = json!("lx04-durable-v1");
            inventory["snapshot"]["durability"] =
                serde_json::to_value(self.durability).map_err(|_| "unavailable")?;
            inventory["checkpoint"] = json!({"available":true,"recovery":"current_intended_look_frozen_into_hold","active_transients_resumed":false});
        }
        if self.analysis.is_some() {
            inventory["wire_schema"] = json!("lx05-v1");
            inventory["analysis"] = self.analysis_status();
            inventory["applied_auto"] = json!("explicit_bounded_intensity_grant");
        }
        Ok(inventory)
    }
    fn pages(&self) -> Result<Vec<Json>, &'static str> {
        let text = serde_json::to_string(&self.inventory()?).map_err(|_| "unavailable")?;
        // Chunks are UTF-8 text, never independent JSON. Assemble then decode once.
        let mut chunks = Vec::new();
        let mut offset = 0;
        while offset < text.len() {
            let mut end = (offset + 48_000).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            // Escape-heavy labels must still fit the encoded envelope.
            while serde_json::to_vec(&text[offset..end])
                .map_err(|_| "unavailable")?
                .len()
                > 60_000
            {
                end = offset + (end - offset) / 2;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
            }
            chunks.push(text[offset..end].to_owned());
            offset = end;
            if chunks.len()
                > if self.analysis.is_some() && !self.analysis_emergency {
                    15
                } else {
                    16
                }
            {
                return Err("capacity");
            }
        }
        let count = chunks.len();
        chunks.into_iter().enumerate().map(|(index,chunk)|{
            let page=self.reply("snapshot",None,None,json!({"page":index,"page_count":count,"encoding":"json_utf8_chunks","chunk":chunk}));
            if serde_json::to_vec(&page).map_err(|_|"unavailable")?.len()>MAX_MESSAGE_BYTES {Err("capacity")} else {Ok(page)}
        }).collect()
    }
}
fn error_reason(e: Error) -> &'static str {
    match e {
        Error::Range | Error::Default => "range",
        Error::Capacity => "capacity",
        Error::Id => "id",
        Error::Version => "version",
        Error::Unavailable => "unavailable",
        Error::Duplicate => "duplicate",
        Error::Group => "group",
        _ => "target",
    }
}

/// Test-only interoperability checker, not a public trusted inventory decoder.
#[cfg(test)]
#[derive(Default)]
struct PageAssembler {
    key: Option<(String, String, String, String, usize, u64)>,
    pages: BTreeMap<usize, String>,
}
#[cfg(test)]
impl PageAssembler {
    pub fn push(&mut self, page: &Json, receipt_tick: u64) -> Result<Option<Json>, &'static str> {
        let result = self.push_inner(page, receipt_tick);
        if result.is_err() {
            self.key = None;
            self.pages.clear();
        }
        result
    }
    fn push_inner(&mut self, page: &Json, tick: u64) -> Result<Option<Json>, &'static str> {
        let string = |key: &str| page[key].as_str().map(str::to_owned).ok_or("malformed");
        if page["kind"] != "snapshot" || page["contract"] != "C-LIGHT" || page["version"] != 1 {
            return Err("version");
        }
        let show = string("show_id")?;
        crate::lighting_contract::validate_show_id(&show).map_err(|_| "wrong_show")?;
        let epoch = string("epoch")?;
        counter::parse(&epoch)?;
        let revision = string("revision")?;
        counter::parse(&revision)?;
        let sequence = string("sequence")?;
        counter::parse(&sequence)?;
        let body = &page["body"];
        let count = body["page_count"].as_u64().ok_or("malformed")? as usize;
        let index = body["page"].as_u64().ok_or("malformed")? as usize;
        if count == 0 || count > 16 || index >= count || body["encoding"] != "json_utf8_chunks" {
            return Err("capacity");
        }
        let chunk = body["chunk"].as_str().ok_or("malformed")?;
        if chunk.len() > 60_000
            || serde_json::to_vec(page).map_err(|_| "malformed")?.len() > MAX_MESSAGE_BYTES
        {
            return Err("capacity");
        }
        if let Some((s, e, r, q, n, start)) = &self.key {
            if s != &show
                || e != &epoch
                || r != &revision
                || q != &sequence
                || *n != count
                || tick < *start
                || tick - *start > 200
            {
                return Err("stale_page");
            }
        } else {
            self.key = Some((show, epoch, revision, sequence, count, tick));
        }
        if self.pages.insert(index, chunk.into()).is_some() {
            return Err("duplicate");
        }
        if self.pages.len() != count {
            return Ok(None);
        }
        let text = self.pages.values().cloned().collect::<String>();
        // Aggregate is bounded by <=16 encoded pages, rather than single-message limit.
        let value = strict_json_bounded(text.as_bytes(), 16 * MAX_MESSAGE_BYTES)?;
        let (show, epoch, revision, _, _, _) = self.key.as_ref().ok_or("malformed")?;
        if value["snapshot"]["show_id"] != *show
            || value["snapshot"]["epoch"] != *epoch
            || value["snapshot"]["revision"] != *revision
        {
            return Err("identity");
        }
        let spec: PatchSpec =
            serde_json::from_value(value["patch"].clone()).map_err(|_| "malformed")?;
        let patch = crate::fixture::Patch::validate(spec).map_err(error_reason)?;
        let snapshot: crate::lighting_contract::Snapshot =
            serde_json::from_value(value["snapshot"].clone()).map_err(|_| "malformed")?;
        snapshot
            .validate_for_schema(&patch, value["wire_schema"].as_str().unwrap_or("lx03-v1"))
            .map_err(error_reason)?;
        self.key = None;
        self.pages.clear();
        Ok(Some(value))
    }
}

#[cfg(test)]
mod page_tests {
    use super::*;
    fn service() -> Service {
        let patch = crate::fixture::Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![],
        })
        .unwrap();
        Service::new(Authority::new(patch, "11111111-1111-4111-8111-111111111111", 9).unwrap())
            .unwrap()
    }
    #[test]
    fn assembled_identity_depth_patch_and_deadline_refuse() {
        let s = service();
        let pages = s.pages().unwrap();
        let page = &pages[0];
        assert_eq!(
            PageAssembler::default().push(page, 0).unwrap(),
            Some(s.inventory().unwrap())
        );
        for field in ["show_id", "epoch", "revision"] {
            let mut inventory = s.inventory().unwrap();
            inventory["snapshot"][field] = if field == "show_id" {
                json!("22222222-2222-4222-8222-222222222222")
            } else {
                json!("8")
            };
            let mut modified = page.clone();
            modified["body"]["chunk"] = json!(serde_json::to_string(&inventory).unwrap());
            assert_eq!(PageAssembler::default().push(&modified, 0), Err("identity"));
        }
        let mut nested = page.clone();
        nested["body"]["chunk"] = json!("[[[[[[[[[[[[[0]]]]]]]]]]]]]");
        assert_eq!(PageAssembler::default().push(&nested, 0), Err("depth"));
        let mut badpatch = s.inventory().unwrap();
        badpatch["patch"]["patch_revision"] = json!("2");
        let mut modified = page.clone();
        modified["body"]["chunk"] = json!(serde_json::to_string(&badpatch).unwrap());
        assert!(PageAssembler::default().push(&modified, 0).is_err());
        let mut p0 = page.clone();
        p0["body"]["page_count"] = json!(2);
        let mut p1 = p0.clone();
        p1["body"]["page"] = json!(1);
        let mut a = PageAssembler::default();
        assert!(a.push(&p0, 0).unwrap().is_none());
        assert_eq!(a.push(&p1, 201), Err("stale_page"));
        let mut a = PageAssembler::default();
        a.push(&p0, 0).unwrap();
        p1["revision"] = json!("2");
        assert_eq!(a.push(&p1, 1), Err("stale_page"));
    }
}

#[derive(Clone)]
struct PreviewEntry {
    writer: Id,
    lease: u64,
    preview: ReleasePreview,
    payload: Json,
}
fn preview_nonce() -> Result<String, &'static str> {
    let mut bytes = [0u8; 16];
    // SAFETY: valid16byte output buffer; nonblocking OS randomness, no device opened.
    let n = unsafe { libc::getrandom(bytes.as_mut_ptr().cast(), bytes.len(), libc::GRND_NONBLOCK) };
    if n != bytes.len() as isize {
        return Err("unavailable");
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod durability_page_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn real_checkpointed_and_error_pages_roundtrip_with_schema_aware_validation() {
        let path = std::env::temp_dir().join(format!("lux-page-durable-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let show = "11111111-1111-4111-8111-111111111111";
        let store =
            std::sync::Arc::new(crate::recovery::CheckpointStore::open(&path, show).unwrap());
        let patch = crate::fixture::Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![],
        })
        .unwrap();
        let mut s = Service::recover(store.clone(), patch).unwrap();
        let envelope = |kind: &str, id: u64, lease: Option<&str>, body: Json| json!({"contract":"C-LIGHT","version":1,"show_id":show,"module":"lighting","epoch":"1","writer":"writer","lease":lease,"request_id":id.to_string(),"expected_revision":"0","kind":kind,"body":body});
        s.handle(&serde_json::to_vec(&envelope("grant", 1, None, json!({"scope":SCOPE}))).unwrap());
        let checkpoint = |id| {
            envelope(
                "command",
                id,
                Some("1"),
                json!({"scope":SCOPE,"command":{"action":"checkpoint"}}),
            )
        };
        assert_eq!(
            s.handle(&serde_json::to_vec(&checkpoint(2)).unwrap())[0]["kind"],
            "applied"
        );
        for expected in ["checkpointed", "error"] {
            if expected == "error" {
                std::fs::write(path.join("lighting-checkpoint.json"), b"{\"version\":99}").unwrap();
                assert_eq!(
                    s.handle(&serde_json::to_vec(&checkpoint(3)).unwrap())[0]["reason"],
                    "durability"
                );
            }
            let inventory = s.inventory().unwrap();
            assert_eq!(inventory["snapshot"]["durability"], expected);
            let pages = s.pages().unwrap();
            let mut assembler = PageAssembler::default();
            let mut result = None;
            for page in pages {
                result = assembler.push(&page, 0).unwrap().or(result);
            }
            assert_eq!(result, Some(inventory));
        }
        drop(s);
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[cfg(test)]
mod analysis_clock_tests {
    use super::*;
    use crate::{
        analysis_subscription::Descriptor,
        authority::Mode,
        fixture::{FixtureSpec, Patch, PatchSpec, SyntheticMode},
    };
    fn ready_service() -> Service {
        let id = Id::new("fixture-11").unwrap();
        let patch = Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![FixtureSpec::synthetic(
                id.clone(),
                "Synthetic",
                SyntheticMode::Dimmer,
                1,
            )],
        })
        .unwrap();
        let mut s = Service::timed(
            Authority::new(patch, "11111111-1111-4111-8111-111111111111", 9).unwrap(),
        )
        .unwrap();
        s.enable_analysis().unwrap();
        s.engine.execute(Command::Mode(Mode::Auto)).unwrap();
        let c: Json =
            serde_json::from_str(include_str!("../tests/fixtures/lx05/v1/e09.json")).unwrap();
        let d = Descriptor::decode(&serde_json::to_vec(&c["descriptor"]).unwrap()).unwrap();
        let a = s.analysis.as_mut().unwrap();
        a.attach(d);
        for n in 0..111u64 {
            let mut b = b"GAW1".to_vec();
            for v in [0u64, 1, 2, 1000] {
                b.extend(v.to_be_bytes());
            }
            b.extend([0; 4]);
            for i in 0..10 {
                let h = c["packets_hex"][i].as_str().unwrap();
                let mut p: Vec<u8> = (0..h.len())
                    .step_by(2)
                    .map(|j| u8::from_str_radix(&h[j..j + 2], 16).unwrap())
                    .collect();
                p[24..32].copy_from_slice(&(48000 + n * 480 + i as u64 * 48).to_be_bytes());
                p[32..36].copy_from_slice(&(n as u32 * 10 + i as u32).to_be_bytes());
                b.extend(p);
            }
            a.ingest(&b, 1000).unwrap();
            if n == 0 {
                a.calibrate("start", 1000).unwrap();
            }
            if n == 50 {
                a.calibrate("finish", 1000).unwrap();
            }
        }
        assert!(a.ready());
        let generation = a.generation;
        let writer = Id::new("clock-test").unwrap();
        s.sessions.insert(
            writer.clone(),
            Session {
                lease: 1,
                expiry: 200,
                highwater: 1,
                cache: VecDeque::new(),
            },
        );
        s.analysis_grant = Some(AnalysisGrant {
            writer,
            lease: 1,
            issue_revision: s.engine.revision(),
            patch: 1,
            generation,
            expiry: 100,
            fixtures: vec![id],
            cap: 500,
        });
        s
    }
    #[test]
    fn processing_and_encoding_delay_or_clock_regression_never_commit_stale_step() {
        for clocks in [[1101, 1102], [1099, 1101], [1099, 1098]] {
            let mut s = ready_service();
            let baseline = s.engine.snapshot();
            let mut times = clocks.into_iter();
            s.poll_analysis_clock(None, 1099, 1, || Ok(times.next().unwrap()))
                .unwrap();
            assert!(s.analysis_grant.is_none());
            assert_eq!(
                s.engine.snapshot().fixtures[0].attributes[0].resolved,
                baseline.fixtures[0].attributes[0].resolved
            );
            assert_eq!(
                s.analysis.as_ref().unwrap().status(s.analysis_now)["confidence"],
                0
            );
            s.pages().unwrap();
        }
    }
    #[test]
    fn actual_near_fifteen_page_step_capacity_freezes_exact_auto_layer() {
        use crate::authority::StoredLook;
        use crate::fixture::{Attribute, Value};
        let template = ready_service();
        fn build(template: &Service, full: usize, tail: usize, padding: usize) -> Service {
            let mut fixtures = (0..32)
                .map(|i| {
                    FixtureSpec::synthetic(
                        Id::new(&format!("f{i:02}{}", "x".repeat(60))).unwrap(),
                        "",
                        SyntheticMode::RgbPosition,
                        i * 7 + 1,
                    )
                })
                .collect::<Vec<_>>();
            for f in fixtures.iter_mut().take(2) {
                f.label = "\"".repeat(padding);
            }
            let patch = Patch::validate(PatchSpec {
                version: 1,
                patch_revision: 1,
                fixtures,
            })
            .unwrap();
            let all = patch
                .fixtures()
                .iter()
                .flat_map(|f| {
                    f.capabilities.iter().map(|c| Value {
                        fixture: f.id.clone(),
                        attribute: c.attribute,
                        value: c.default,
                    })
                })
                .collect::<Vec<_>>();
            let count = full + usize::from(tail > 0);
            let stores = (0..count)
                .map(|i| StoredLook {
                    kind: if i < 32 {
                        StoredKind::Cue
                    } else {
                        StoredKind::Palette
                    },
                    id: Id::new(&format!("s{i:02}{}", "x".repeat(60))).unwrap(),
                    values: if i == full {
                        all[..tail * 7].to_vec()
                    } else {
                        all.clone()
                    },
                })
                .collect();
            let mut e = Authority::new(patch, "11111111-1111-4111-8111-111111111111", 9)
                .unwrap()
                .with_stores(stores)
                .unwrap();
            for values in all.chunks(56) {
                e.execute(Command::Touch(values.to_vec())).unwrap();
            }
            e.execute(Command::ClearToHold).unwrap();
            e.enable_analysis();
            e.execute(Command::Mode(Mode::Auto)).unwrap();
            let targets = e
                .patch()
                .fixtures()
                .iter()
                .map(|f| f.id.clone())
                .collect::<Vec<_>>();
            for _ in 0..5 {
                e.step_auto(&targets, 500, 99).unwrap();
            }
            let mut s = template.clone();
            s.engine = e;
            s.analysis_now = 1000;
            let g = s.analysis_grant.as_mut().unwrap();
            g.fixtures = targets;
            g.issue_revision = s.engine.revision();
            s
        }
        // Find a real production boundary with bounded binary searches, then pack
        // the last partial store and legal quoted labels to within a few bytes.
        fn maximum(mut low: usize, mut high: usize, mut fits: impl FnMut(usize) -> bool) -> usize {
            while low < high {
                let mid = (low + high).div_ceil(2);
                if fits(mid) { low = mid } else { high = mid - 1 }
            }
            low
        }
        let full = maximum(0, 63, |n| build(&template, n, 0, 0).pages().is_ok());
        let tail = maximum(0, 31, |n| build(&template, full, n, 0).pages().is_ok());
        let padding = maximum(0, 128, |n| build(&template, full, tail, n).pages().is_ok());
        let mut s = build(&template, full, tail, padding);
        assert_eq!(s.pages().unwrap().len(), 15);
        let before = s.engine.snapshot();
        let mut candidate = s.clone();
        candidate
            .poll_analysis_inner(None, 1000, 1, &mut || Ok(1000))
            .unwrap();
        assert_eq!(
            candidate.pages().unwrap_err(),
            "capacity",
            "fixture must actually cross production active15page budget"
        );
        s.poll_analysis_clock(None, 1000, 1, || Ok(1000)).unwrap();
        assert!(s.analysis_grant.is_none());
        assert_eq!(
            s.analysis.as_ref().unwrap().reason,
            Some("snapshot_capacity")
        );
        assert!(s.pages().unwrap().len() <= 16);
        for (old, new) in before.fixtures.iter().zip(s.engine.snapshot().fixtures) {
            for (a, b) in old.attributes.iter().zip(new.attributes) {
                assert_eq!(a.resolved, b.resolved);
                assert_eq!(a.auto, b.auto);
                assert_eq!(a.hold, b.hold);
                if b.attribute == Attribute::Intensity {
                    assert_eq!(b.auto, Some(99));
                    assert!(b.contributors.iter().any(|c| c.source
                        == crate::lighting_contract::Source::Auto(
                            Id::new("analysis-held").unwrap()
                        )));
                }
            }
        }
    }
    #[test]
    fn qualified_pcm_active_loss_and_manual_owner_output_corpus() {
        let mut s = ready_service();
        s.poll_analysis_clock(None, 1000, 1, || Ok(1000)).unwrap();
        let active = s.inventory().unwrap();
        assert_eq!(
            active["snapshot"]["fixtures"][0]["attributes"][0]["resolved"],
            20
        );
        s.poll_analysis_clock(None, 1101, 2, || Ok(1101)).unwrap();
        let lost = s.inventory().unwrap();
        assert!(s.analysis_grant.is_none());
        let request = json!({"contract":"C-LIGHT","version":1,"show_id":"11111111-1111-4111-8111-111111111111","module":"lighting","epoch":"9","writer":"clock-test","lease":"1","request_id":"2","expected_revision":s.engine.revision().to_string(),"kind":"command","body":{"scope":SCOPE,"command":{"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":900}]}}});
        let replies = s.handle(&serde_json::to_vec(&request).unwrap());
        assert_eq!(replies[0]["kind"], "applied");
        let manual = s.inventory().unwrap();
        let corpus = json!({"schema":"lx05-v1","provenance":"Actual independent Lux PCM decoder/Calibrator/Analyzer/Director and Authority/Service. Exact accepted E09 PCM; injected same-host acquisition clock1000/1101ms, synthetic fixture; no physical output.","active":active,"aged_loss":lost,"manual":{"request":request,"replies":replies,"inventory":manual}});
        if std::env::var_os("LUX_GENERATE_LX05_CORPUS").is_some() {
            std::fs::write(
                "tests/fixtures/lx05/v1/features.json",
                serde_json::to_string_pretty(&corpus).unwrap() + "\n",
            )
            .unwrap();
        } else {
            assert_eq!(
                corpus,
                serde_json::from_str::<Json>(include_str!(
                    "../tests/fixtures/lx05/v1/features.json"
                ))
                .unwrap()
            );
        }
    }
    #[test]
    fn assist_without_grant_encoding_age_expiry_clears_snapshot_proposal() {
        let mut s = ready_service();
        s.engine.execute(Command::Mode(Mode::Assist)).unwrap();
        s.analysis_grant = None;
        let mut times = [1099, 1101].into_iter();
        s.poll_analysis_clock(None, 1099, 1, || Ok(times.next().unwrap()))
            .unwrap();
        assert!(s.analysis.as_ref().unwrap().proposal.is_none());
        assert_eq!(s.analysis.as_ref().unwrap().state, "stale");
        assert!(
            s.engine
                .snapshot()
                .fixtures
                .iter()
                .flat_map(|f| &f.attributes)
                .all(|a| a.proposal.is_none())
        );
        assert_eq!(s.analysis_status()["confidence"], 0);
    }
}
