use std::fmt;

use crate::TargetJobId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(uuid::Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Binary reprensentation
    pub fn to_bytes(self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }

    /// From binary reprensentation
    pub fn from_bytes(b: &[u8]) -> Result<Self, InvalidJobIdBytes> {
        Ok(Self(
            uuid::Uuid::from_slice(b).map_err(|_| InvalidJobIdBytes)?,
        ))
    }

    /// From text reprensentation
    pub fn parse(b: &str) -> Result<Self, InvalidJobIdString> {
        Ok(Self(
            uuid::Uuid::parse_str(b).map_err(|_| InvalidJobIdString)?,
        ))
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

impl std::convert::TryFrom<TargetJobId> for JobId {
    type Error = InvalidJobIdBytes;
    fn try_from(target: TargetJobId) -> Result<Self, Self::Error> {
        Self::from_bytes(&target.jobid)
    }
}

impl From<JobId> for TargetJobId {
    fn from(id: JobId) -> TargetJobId {
        TargetJobId {
            jobid: id.to_bytes(),
        }
    }
}

/// Binary version was invalid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvalidJobIdBytes;

impl fmt::Display for InvalidJobIdBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid job id (UUID required)")
    }
}

impl std::error::Error for InvalidJobIdBytes {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&InvalidJobIdBytes)
    }
}

/// String representation was invalid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvalidJobIdString;

impl fmt::Display for InvalidJobIdString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid job id (UUID required)")
    }
}

impl std::error::Error for InvalidJobIdString {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&InvalidJobIdString)
    }
}
