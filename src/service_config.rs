//! Explicit software-only startup identity. No device discovery or output backend.
use crate::{
    analysis_subscription::Binding,
    fixture::{Patch, PatchSpec},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
pub const CONFIG_LIMIT: usize = 65536;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    pub version: u16,
    pub show_id: String,
    pub patch: PatchSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Binding>,
}
impl ServiceConfig {
    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        let value = crate::wire::strict_json_bounded(bytes, CONFIG_LIMIT)?;
        let config: Self = serde_json::from_value(value).map_err(|_| "startup config schema")?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<Patch, &'static str> {
        if self.version != 1 {
            return Err("startup config version");
        }
        crate::lighting_contract::validate_show_id(&self.show_id).map_err(|_| "startup show id")?;
        if let Some(binding) = &self.analysis {
            binding.validate()?;
        }
        Patch::validate(self.patch.clone()).map_err(|_| "startup patch")
    }
    pub fn read(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(io::Error::other("absolute config path required"));
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("regular config file required"));
        }
        let mut bytes = Vec::new();
        file.take((CONFIG_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)?;
        Self::decode(&bytes).map_err(io::Error::other)
    }
}
