//! Vertical scale of the plot: a reference level at the top and a fixed step
//! per division, like on a bench analyzer. It follows the data but does not
//! jump around: it only moves when the trace leaves the scale or uses too
//! little of it.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    /// Value at the top line.
    pub top: f32,
    /// Value difference between two grid lines.
    pub per_div: f32,
    /// Number of divisions from top to bottom.
    pub rows: usize,
}

impl Scale {
    pub fn bottom(&self) -> f32 {
        self.top - self.per_div * self.rows as f32
    }

    /// A scale that holds `min..=max` with a margin, with a step of 1, 2 or 5 times a power of ten.
    pub fn fit(min: f32, max: f32, rows: usize) -> Scale {
        let span = (max - min).max(1.0) * 1.2;
        let raw_step = span / rows as f32;
        let magnitude = 10f32.powf(raw_step.log10().floor());
        let per_div = [1.0, 2.0, 5.0, 10.0]
            .iter()
            .map(|m| m * magnitude)
            .find(|s| *s >= raw_step)
            .unwrap_or(10.0 * magnitude);
        let top = ((max + span * 0.05) / per_div).ceil() * per_div;
        Scale { top, per_div, rows }
    }

    /// Keeps this scale while the data fits it well; otherwise refits.
    pub fn follow(self, min: f32, max: f32) -> Scale {
        let height = self.per_div * self.rows as f32;
        let inside = min >= self.bottom() && max <= self.top;
        let uses_enough = (max - min) >= 0.4 * height;
        if inside && uses_enough {
            self
        } else {
            Scale::fit(min, max, self.rows)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_holds_the_data_with_a_round_step() {
        let s = Scale::fit(142.0, 177.0, 8);
        assert!(s.bottom() <= 142.0 && s.top >= 177.0);
        assert!([1.0, 2.0, 5.0].contains(&(s.per_div / 10f32.powf(s.per_div.log10().floor()))));
        assert_eq!(s.top % s.per_div, 0.0);
    }

    #[test]
    fn follow_keeps_a_good_scale_and_refits_a_bad_one() {
        let s = Scale::fit(100.0, 200.0, 8);
        assert_eq!(s.follow(110.0, 190.0), s);
        assert_ne!(s.follow(100.0, 400.0), s); // data left the scale
        assert_ne!(s.follow(150.0, 155.0), s); // data uses too little of it
    }

    #[test]
    fn flat_data_gets_a_usable_scale() {
        let s = Scale::fit(150.0, 150.0, 8);
        assert!(s.per_div > 0.0);
        assert!(s.bottom() < 150.0 && s.top > 150.0);
    }
}
