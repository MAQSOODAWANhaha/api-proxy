use std::fmt;
use std::time::Duration;

pub type UserId = i32;
pub type ProviderTypeId = i32;
pub type ProviderKeyId = i32;
pub type ServiceApiId = i32;
pub type TraceId = i32;

pub type RequestCount = u64;
pub type TokenCount = u64;

pub type CostValue = f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeoutSeconds(pub u64);

impl TimeoutSeconds {
    #[must_use]
    pub const fn new(seconds: u64) -> Self {
        Self(seconds)
    }

    #[must_use]
    pub const fn as_secs(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn as_duration(self) -> Duration {
        Duration::from_secs(self.0)
    }
}

impl fmt::Display for TimeoutSeconds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Percentage(pub f64);
