use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Asset(String);

impl Asset {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Asset {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        if t.is_empty() {
            return Err("asset is empty");
        }
        // Keep it simple: normalize to uppercase and reject whitespace.
        if t.chars().any(|c| c.is_whitespace()) {
            return Err("asset contains whitespace");
        }
        Ok(Self(t.to_uppercase()))
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
