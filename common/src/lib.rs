tonic::include_proto!("common");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(uuid::Uuid);

impl JobId {
    const ERROR: &'static str = "Invalid UUID";

    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Binary reprensentation
    pub fn to_bytes(self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }

    /// From binary reprensentation
    pub fn from_bytes(b: &[u8]) -> Result<Self, &'static str> {
        Ok(Self(uuid::Uuid::from_slice(b).map_err(|_| Self::ERROR)?))
    }

    /// From text reprensentation
    pub fn parse(b: &str) -> Result<Self, &'static str> {
        Ok(Self(uuid::Uuid::parse_str(b).map_err(|_| Self::ERROR)?))
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
    type Error = &'static str;
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
