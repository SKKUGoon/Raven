pub mod tibs;
pub mod trbs;
pub mod vibs;
pub mod vpin;

use crate::config::TibsConfig;

pub(crate) fn imbalance_bounds_from_config(config: &TibsConfig, initial_size: f64) -> (f64, f64) {
    if let (Some(min_pct), Some(max_pct)) = (config.size_min_pct, config.size_max_pct) {
        let min = initial_size * (1.0 - min_pct);
        let max = initial_size * (1.0 + max_pct);
        return (min, max);
    }
    (initial_size * 0.9, initial_size * 1.1)
}
