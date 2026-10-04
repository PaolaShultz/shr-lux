//! C-LIGHT:1 pure synthetic patch validation. No fixture encoding or hardware I/O.
use crate::dmx::{Address, CHANNELS};

pub const VERSION: u16 = 1;
pub const MAX_FIXTURES: usize = 32;
pub const MAX_TARGETS: usize = 64;

#[derive(serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Version,
    Id,
    Label,
    Capacity,
    Duplicate,
    Mode,
    Address,
    Overlap,
    Range,
    Default,
    Group,
    Target,
    Unavailable,
}

/// Common domain ID, independent of a fixture's position in a patch.
#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Id(String);
impl<'de> serde::Deserialize<'de> for Id {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(d)?;
        Self::new(&s).map_err(|_| serde::de::Error::custom("id"))
    }
}
impl Id {
    pub fn new(value: &str) -> Result<Self, Error> {
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 64
            || !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit()
            || !bytes.iter().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
        {
            return Err(Error::Id);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum Attribute {
    Intensity,
    Red,
    Green,
    Blue,
    Pan,
    Tilt,
    Zoom,
}
impl Attribute {
    pub fn name(self) -> &'static str {
        match self {
            Self::Intensity => "intensity",
            Self::Red => "red",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Pan => "pan",
            Self::Tilt => "tilt",
            Self::Zoom => "zoom",
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::Intensity | Self::Red | Self::Green | Self::Blue => "tenth_percent",
            Self::Pan | Self::Tilt | Self::Zoom => "tenth_degree",
        }
    }
}

/// Inclusive logical range; angular limits are profile-specific, never universal.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub attribute: Attribute,
    pub min: i32,
    pub max: i32,
    pub default: i32,
}
impl Capability {
    pub fn contains(self, value: i32) -> bool {
        (self.min..=self.max).contains(&value)
    }
    fn validate(self) -> Result<(), Error> {
        if self.min > self.max {
            return Err(Error::Range);
        }
        if matches!(
            self.attribute,
            Attribute::Intensity | Attribute::Red | Attribute::Green | Attribute::Blue
        ) && (self.min != 0 || self.max != 1000)
        {
            return Err(Error::Range);
        }
        if !self.contains(self.default) {
            return Err(Error::Default);
        }
        Ok(())
    }
}

/// The only supported personalities are explicitly invented. No real DMX map.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyntheticMode {
    Dimmer,
    Rgb,
    RgbPosition,
}
impl SyntheticMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Dimmer => "synthetic-dimmer-v1",
            Self::Rgb => "synthetic-rgb-v1",
            Self::RgbPosition => "synthetic-rgb-position-v1",
        }
    }
    pub fn attributes(self) -> &'static [Attribute] {
        use Attribute::*;
        match self {
            Self::Dimmer => &[Intensity],
            Self::Rgb => &[Intensity, Red, Green, Blue],
            Self::RgbPosition => &[Intensity, Red, Green, Blue, Pan, Tilt, Zoom],
        }
    }
    pub fn footprint(self) -> u16 {
        self.attributes().len() as u16
    }
    pub fn capabilities(self) -> Vec<Capability> {
        self.attributes()
            .iter()
            .map(|&attribute| {
                let (min, max, default) = match attribute {
                    Attribute::Pan => (-2700, 2700, 0),
                    Attribute::Tilt => (-1350, 1350, 0),
                    Attribute::Zoom => (50, 450, 50),
                    _ => (0, 1000, 0),
                };
                Capability {
                    attribute,
                    min,
                    max,
                    default,
                }
            })
            .collect()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FixtureSpec {
    pub id: Id,
    pub label: String,
    pub mode: SyntheticMode,
    pub address: u16,
    pub capabilities: Vec<Capability>,
}
impl FixtureSpec {
    pub fn synthetic(id: Id, label: &str, mode: SyntheticMode, address: u16) -> Self {
        Self {
            id,
            label: label.to_owned(),
            mode,
            address,
            capabilities: mode.capabilities(),
        }
    }
    pub fn capability(&self, attribute: Attribute) -> Option<Capability> {
        self.capabilities
            .iter()
            .find(|c| c.attribute == attribute)
            .copied()
    }
    fn validate(&self) -> Result<(), Error> {
        if self.label.len() > 128 || self.label.chars().any(char::is_control) {
            return Err(Error::Label);
        }
        if self.capabilities.len() != self.mode.attributes().len() {
            return Err(Error::Mode);
        }
        for &attribute in self.mode.attributes() {
            let mut found = self
                .capabilities
                .iter()
                .filter(|c| c.attribute == attribute);
            let capability = found.next().ok_or(Error::Mode)?;
            if found.next().is_some() {
                return Err(Error::Duplicate);
            }
            capability.validate()?;
        }
        let start = Address::new(self.address).ok_or(Error::Address)?;
        if usize::from(start.index()) + usize::from(self.mode.footprint()) > CHANNELS {
            return Err(Error::Address);
        }
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchSpec {
    pub version: u16,
    #[serde(with = "crate::wire::counter")]
    pub patch_revision: u64,
    pub fixtures: Vec<FixtureSpec>,
}

/// Only complete validated patches can be observed; failed replacement retains the old one.
#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Patch(PatchSpec);
impl Patch {
    pub fn validate(spec: PatchSpec) -> Result<Self, Error> {
        if spec.version != VERSION {
            return Err(Error::Version);
        }
        if spec.fixtures.len() > MAX_FIXTURES {
            return Err(Error::Capacity);
        }
        let mut occupied = [false; CHANNELS];
        for (index, fixture) in spec.fixtures.iter().enumerate() {
            fixture.validate()?;
            if spec.fixtures[..index].iter().any(|f| f.id == fixture.id) {
                return Err(Error::Duplicate);
            }
            let start = usize::from(Address::new(fixture.address).ok_or(Error::Address)?.index());
            for slot in &mut occupied[start..start + usize::from(fixture.mode.footprint())] {
                if *slot {
                    return Err(Error::Overlap);
                }
                *slot = true;
            }
        }
        Ok(Self(spec))
    }
    pub fn revision(&self) -> u64 {
        self.0.patch_revision
    }
    pub fn fixtures(&self) -> &[FixtureSpec] {
        &self.0.fixtures
    }
    pub fn fixture(&self, id: &Id) -> Option<&FixtureSpec> {
        self.0.fixtures.iter().find(|f| &f.id == id)
    }
    pub fn replace(&mut self, spec: PatchSpec) -> Result<(), Error> {
        let next = Self::validate(spec)?;
        *self = next;
        Ok(())
    }
    /// Validate only: does not apply an edit or claim authority. Coherent groups
    /// require every RGB or pan/tilt component of each touched fixture.
    pub fn validate_values(&self, values: &[Value]) -> Result<(), Error> {
        if values.len() > MAX_TARGETS {
            return Err(Error::Capacity);
        }
        for (index, value) in values.iter().enumerate() {
            let fixture = self.fixture(&value.fixture).ok_or(Error::Target)?;
            let cap = fixture
                .capability(value.attribute)
                .ok_or(Error::Unavailable)?;
            if !cap.contains(value.value) {
                return Err(Error::Range);
            }
            if values[..index]
                .iter()
                .any(|v| v.fixture == value.fixture && v.attribute == value.attribute)
            {
                return Err(Error::Duplicate);
            }
            let group: &[Attribute] = match value.attribute {
                Attribute::Red | Attribute::Green | Attribute::Blue => {
                    &[Attribute::Red, Attribute::Green, Attribute::Blue]
                }
                Attribute::Pan | Attribute::Tilt => &[Attribute::Pan, Attribute::Tilt],
                _ => &[],
            };
            if group.iter().any(|a| {
                !values
                    .iter()
                    .any(|v| v.fixture == value.fixture && v.attribute == *a)
            }) {
                return Err(Error::Group);
            }
        }
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Value {
    pub fixture: Id,
    pub attribute: Attribute,
    pub value: i32,
}
