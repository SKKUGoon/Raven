pub mod tib;

use chrono::{DateTime, Utc};

/// Direction of tick imbalance
pub enum TickDirection {
    Buy,
    Sell,
}

impl TickDirection {
    #[inline]
    pub fn to_i32(&self) -> i32 {
        match self {
            TickDirection::Buy => 1,
            TickDirection::Sell => -1,
        }
    }
}

/// Trait implemented by all bar types
pub trait UpdateBar {
    fn update(&mut self, price: f64, volume: f64, b_t: TickDirection, ts: DateTime<Utc>);
    fn close(&mut self);
}

/// Trait implemented by all bar state machine
pub trait BarStateMachine {
    /// The bar type. e.g., TickImbalanceBar, TickRunsBar, etc.
    type Bar;

    /// Update the bar with a new tick
    fn on_tick(
        &mut self,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Option<Self::Bar>;

    /// Flush the bar even if it is not closed
    fn flush(&mut self) -> Option<Self::Bar>;
    fn threshold(&self) -> f64;
    fn size_ewma(&self) -> f64;
    fn imbl_ewma(&self) -> f64;
}

pub struct BarManager {}
