use crate::clients::bars::{BarStateMachine, TickDirection, UpdateBar};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct TickImbalanceBar {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub trade_size: f64,

    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,

    /// Tick size is the number of ticks used to create the bar.
    pub tick_size: u32,
    pub buy_ticks: u32,
    pub sell_ticks: u32,

    /// Theta is the amount of information the bar currently holds
    pub theta: i32,

    /// Bar open is a flag indicating if the bar is currently being updated
    pub bar_open: bool,
}

impl UpdateBar for TickImbalanceBar {
    fn update(&mut self, price: f64, volume: f64, b_t: TickDirection, ts: DateTime<Utc>) {
        if self.bar_open {
            // Update price
            self.high = price.max(self.high);
            self.low = price.min(self.low);
            self.close = price;

            // Update volume
            self.trade_size += volume;

            // Update time
            self.close_time = ts;

            // Update information
            self.tick_size += 1u32;
            self.theta += b_t.to_i32();
        }
    }

    fn close(&mut self) {
        self.bar_open = false;
    }
}

impl TickImbalanceBar {
    pub fn new(price: f64, volume: f64, b_t: TickDirection, ts: DateTime<Utc>) -> Self {
        // First buy tick and first sell tick
        let (fbt, fst) = match b_t {
            TickDirection::Buy => (1, 0),
            TickDirection::Sell => (0, 1),
        };

        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            trade_size: volume,
            open_time: ts,
            close_time: ts,
            tick_size: 1,
            buy_ticks: fbt,
            sell_ticks: fst,
            theta: b_t.to_i32(),
            bar_open: true,
        }
    }

    #[inline]
    pub fn p_buy(&self) -> f64 {
        self.buy_ticks as f64 / self.tick_size as f64
    }
}

pub struct TickImbalanceState {
    pub threshold: f64,

    /// E[T] from Prodo de Lopez
    pub size_ewma: f64,
    /// 2 * P[b_t = 1] - 1 from Prodo de Lopez
    pub imbl_ewma: f64,

    pub alpha_size: f64, // Constant for EWMA. Weight on the window
    pub alpha_imbl: f64, // Constant for EWMA.

    /// Boundary value for candle size to stop the information bar explosion
    pub size_boundary: (f64, f64),

    pub current_bar: Option<TickImbalanceBar>,
}

impl TickImbalanceState {
    pub fn new(
        initial_size: f64, // Initial size of the bar. How many ticks are needed to create the bar?
        initial_p_buy: f64, // Initial probability of a buy tick. How much of a strong signal is needed? e.g. 0.7 means 70% of directional is a strong information
        alpha_size: f64,    // Constant for EWMA. Weight on the window
        alpha_imbl: f64,    // Constant for EWMA. Weight on the window
        size_boundary: (f64, f64), // Boundary value for candle size to stop the information bar explosion
    ) -> Self {
        let size_ewma = initial_size;
        let imbl_ewma = 2.0 * initial_p_buy - 1.0;
        let threshold = size_ewma * imbl_ewma;

        Self {
            threshold,
            size_ewma,
            imbl_ewma,
            alpha_size,
            alpha_imbl,
            size_boundary,
            current_bar: None,
        }
    }

    fn bound(val: f64, min: f64, max: f64) -> f64 {
        val.max(min).min(max)
    }
}

impl BarStateMachine for TickImbalanceState {
    type Bar = TickImbalanceBar;

    fn on_tick(
        &mut self,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Option<Self::Bar> {
        if self.current_bar.is_none() {
            self.current_bar = Some(TickImbalanceBar::new(price, volume, b_t, ts));
            return None;
        }

        let bar = self.current_bar.as_mut().unwrap();
        bar.update(price, volume, b_t, ts);

        if (bar.theta as f64).abs() > self.threshold.abs() {
            bar.close();
            let closed = self.current_bar.take().unwrap();

            // Update EWMA imbalance
            let new_imbl = 2.0 * closed.p_buy() - 1.0;
            self.imbl_ewma = self.alpha_imbl * new_imbl + (1.0 - self.alpha_imbl) * self.imbl_ewma;

            // Update EWMA candle size
            let new_size = closed.tick_size as f64;
            self.size_ewma = self.alpha_size * new_size + (1.0 - self.alpha_size) * self.size_ewma;
            self.size_ewma =
                Self::bound(self.size_ewma, self.size_boundary.0, self.size_boundary.1);

            self.threshold = self.size_ewma * self.imbl_ewma;

            self.current_bar = None;
            return Some(closed);
        }

        None
    }

    /// Force flush the current bar even if it is not closed
    fn flush(&mut self) -> Option<Self::Bar> {
        self.current_bar.take()
    }

    fn threshold(&self) -> f64 {
        self.threshold
    }

    fn size_ewma(&self) -> f64 {
        self.size_ewma
    }

    fn imbl_ewma(&self) -> f64 {
        self.imbl_ewma
    }
}
