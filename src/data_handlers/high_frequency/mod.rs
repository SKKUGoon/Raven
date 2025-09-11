// High Frequency Handler Module
// "The fastest ravens in the realm - delivering messages with sub-microsecond speed"

mod handler;
mod performance_stats;

pub use handler::HighFrequencyHandler;
pub use performance_stats::PerformanceStats;

#[cfg(test)]
mod tests;
