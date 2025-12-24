use std::collections::HashSet;
use std::str::FromStr;

use crate::domain::venue::{default_venues, VenueId};

#[derive(Clone, Debug, Default)]
pub struct VenueSelector {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl VenueSelector {
    pub fn resolve(&self) -> Vec<VenueId> {
        let mut includes: Vec<VenueId> = if self.include.is_empty() {
            default_venues()
        } else {
            self.include
                .iter()
                .filter_map(|v| VenueId::from_str(v).ok())
                .collect()
        };

        // Deduplicate
        let mut seen = HashSet::new();
        includes.retain(|v| seen.insert(v.as_wire()));

        if self.exclude.is_empty() {
            return includes;
        }
        let exclude_set: HashSet<String> = self
            .exclude
            .iter()
            .filter_map(|v| VenueId::from_str(v).ok())
            .map(|v| v.as_wire())
            .collect();

        includes
            .into_iter()
            .filter(|v| !exclude_set.contains(&v.as_wire()))
            .collect()
    }
}


