//! The CPU temperature history behind the trend card.
//!
//! The module owns both the ring buffer and the viewbox mapping, so the plot
//! can be tested without a window.

use std::collections::VecDeque;

/// Seconds of history kept at the one-second refresh rate.
pub const CAPACITY: usize = 120;

/// Lowest and highest temperatures the plot's height covers.
const MIN_C: f32 = 30.0;
const MAX_C: f32 = 100.0;

#[derive(Debug, Default)]
pub struct History {
    samples: VecDeque<f32>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one sample, dropping the oldest when full.
    pub fn push(&mut self, celsius: f32) {
        if self.samples.len() >= CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(celsius);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// An SVG-ish polyline in a 1000x100 viewbox, or an empty string while
    /// there is nothing to draw. Slint scales the path to the card.
    pub fn plot_path(&self) -> String {
        self.points().unwrap_or_default()
    }

    fn points(&self) -> Option<String> {
        if self.samples.len() < 2 {
            return None;
        }
        let step = 1000.0 / (self.samples.len() - 1) as f32;
        let mut path = String::with_capacity(self.samples.len() * 12);
        for (i, temp) in self.samples.iter().enumerate() {
            let x = i as f32 * step;
            let normalized = ((temp.clamp(MIN_C, MAX_C) - MIN_C) / (MAX_C - MIN_C)).clamp(0.0, 1.0);
            let y = 100.0 - normalized * 100.0;
            path.push_str(if i == 0 { "M" } else { " L" });
            path.push_str(&format!("{x:.0} {y:.0}"));
        }
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_single_sample_draws_nothing() {
        let mut history = History::new();
        assert!(history.plot_path().is_empty());
        history.push(50.0);
        assert!(history.plot_path().is_empty());
    }

    #[test]
    fn plot_maps_full_scale_to_the_viewbox() {
        let mut history = History::new();
        history.push(MIN_C);
        history.push(MAX_C);
        assert_eq!(history.plot_path(), "M0 100 L1000 0");
    }

    #[test]
    fn plot_clamps_out_of_range_samples() {
        let mut history = History::new();
        for temp in [10.0, 120.0, 65.0] {
            history.push(temp);
        }
        assert_eq!(history.plot_path(), "M0 100 L500 0 L1000 50");
    }

    #[test]
    fn the_buffer_stays_at_capacity() {
        let mut history = History::new();
        for i in 0..CAPACITY + 30 {
            history.push(40.0 + (i % 10) as f32);
        }
        assert_eq!(history.len(), CAPACITY);
        assert_eq!(history.plot_path().matches('M').count(), 1);
    }

    #[test]
    fn one_sample_more_still_draws_a_line() {
        let mut history = History::new();
        history.push(40.0);
        history.push(60.0);
        assert_eq!(history.plot_path(), "M0 86 L1000 57");
    }
}
