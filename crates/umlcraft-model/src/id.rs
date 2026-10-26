use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identifier of model elements and diagram presentations.
///
/// New ids are UUIDv7, so they sort roughly by creation time. Files list elements sorted by id,
/// which keeps diffs append-only for newly created elements.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(Uuid);

impl Id {
    /// Generates a fresh, time-ordered id.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Deterministic ids for tests and fixtures.
    pub const fn from_u128(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }

    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for Id {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The last 8 hex digits are enough to tell ids apart in test output.
        let s = self.0.simple().to_string();
        write!(f, "Id(…{})", &s[s.len() - 8..])
    }
}
