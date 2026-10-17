//! The trace shown on screen: the latest sweep, filled in point by point as
//! the board measures, optionally averaged with the previous sweeps.

use crate::protocol::{Sample, SweepParams};

#[derive(Debug, Clone)]
pub struct Trace {
    pub params: SweepParams,
    /// Frequency of every point of the sweep, in MHz.
    pub freq_mhz: Vec<f64>,
    level: Vec<f32>,
    max_hold: Vec<f32>,
    min_hold: Vec<f32>,
    /// Points measured so far; the rest of `level` is not valid yet.
    filled: usize,
    /// Index of the next point the board will deliver, while a sweep is running.
    cursor: Option<usize>,
}

impl Trace {
    pub fn new(params: SweepParams) -> Trace {
        let n = usize::from(params.points);
        Trace {
            params,
            freq_mhz: (0..n).map(|i| params.freq_at(i) as f64 / 1e6).collect(),
            level: vec![0.0; n],
            max_hold: vec![f32::MIN; n],
            min_hold: vec![f32::MAX; n],
            filled: 0,
            cursor: None,
        }
    }

    /// Stores newly measured points that start at point `offset`. `averaging`
    /// is how many sweeps are averaged (1 means no averaging): points that
    /// already hold a value move toward the new one by `1 / averaging`.
    pub fn apply(&mut self, offset: usize, samples: &[Sample], averaging: u8) {
        let alpha = 1.0 / f32::from(averaging.max(1));
        for (j, s) in samples.iter().enumerate() {
            let i = offset + j;
            if i >= self.level.len() {
                break;
            }
            let v = f32::from(s.ch1);
            self.max_hold[i] = self.max_hold[i].max(v);
            self.min_hold[i] = self.min_hold[i].min(v);
            if i < self.filled {
                self.level[i] += alpha * (v - self.level[i]);
            } else {
                self.level[i] = v;
            }
        }
        let end = (offset + samples.len()).min(self.level.len());
        self.filled = self.filled.max(end);
        self.cursor = (end < self.level.len()).then_some(end);
    }

    /// A copy of the trace with every level (and hold level) passed through `f`,
    /// for showing the same data in other units. `f` must be increasing.
    pub fn mapped(&self, f: impl Fn(f32) -> f32) -> Trace {
        let map = |v: &[f32]| v.iter().map(|&x| f(x)).collect();
        Trace {
            params: self.params,
            freq_mhz: self.freq_mhz.clone(),
            level: map(&self.level),
            max_hold: map(&self.max_hold),
            min_hold: map(&self.min_hold),
            filled: self.filled,
            cursor: self.cursor,
        }
    }

    /// Total number of points of the sweep.
    pub fn total(&self) -> usize {
        self.level.len()
    }

    /// Frequencies (MHz) of the points that have a value.
    pub fn freqs(&self) -> &[f64] {
        &self.freq_mhz[..self.filled]
    }

    /// Levels of the points that have a value.
    pub fn levels(&self) -> &[f32] {
        &self.level[..self.filled]
    }

    /// Highest raw value seen at each measured point since the last `clear_hold`.
    pub fn max_hold(&self) -> &[f32] {
        &self.max_hold[..self.filled]
    }

    /// Lowest raw value seen at each measured point since the last `clear_hold`.
    pub fn min_hold(&self) -> &[f32] {
        &self.min_hold[..self.filled]
    }

    pub fn clear_hold(&mut self) {
        self.max_hold.fill(f32::MIN);
        self.min_hold.fill(f32::MAX);
        for i in 0..self.filled {
            self.max_hold[i] = self.level[i];
            self.min_hold[i] = self.level[i];
        }
    }

    /// Up to `count` strongest peaks, strongest first. A peak is a local
    /// maximum within `neighbours` points on each side, in the upper part of
    /// the trace, and at least `neighbours` points away from a stronger peak.
    pub fn peaks(&self, count: usize, neighbours: usize) -> Vec<usize> {
        let l = self.levels();
        let Some((lo, hi)) = self.range() else {
            return Vec::new();
        };
        let floor = lo + 0.3 * (hi - lo);
        let mut candidates: Vec<usize> = (0..l.len())
            .filter(|&i| {
                let from = i.saturating_sub(neighbours);
                let to = (i + neighbours + 1).min(l.len());
                l[i] >= floor && l[from..to].iter().all(|&v| v <= l[i])
            })
            .collect();
        candidates.sort_by(|&a, &b| l[b].total_cmp(&l[a]));
        let mut out: Vec<usize> = Vec::new();
        for c in candidates {
            if out.iter().all(|&o| o.abs_diff(c) > neighbours) {
                out.push(c);
            }
            if out.len() == count {
                break;
            }
        }
        out
    }

    /// Index of the next point to arrive, while a sweep is in progress.
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// Index of the highest level.
    pub fn peak(&self) -> Option<usize> {
        self.levels()
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
    }

    /// (min, max) of the levels, or `None` when nothing was measured yet.
    pub fn range(&self) -> Option<(f32, f32)> {
        let l = self.levels();
        if l.is_empty() {
            return None;
        }
        Some(
            l.iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> SweepParams {
        SweepParams::new(35_000_000, 45_000_000, 5).unwrap()
    }

    fn samples(values: &[u16]) -> Vec<Sample> {
        values.iter().map(|&v| Sample { ch1: v, ch2: v }).collect()
    }

    #[test]
    fn fills_progressively() {
        let mut t = Trace::new(params());
        assert!(t.levels().is_empty());
        assert_eq!(t.range(), None);
        t.apply(0, &samples(&[10, 20]), 4);
        assert_eq!(t.levels(), &[10.0, 20.0]);
        assert_eq!(t.cursor(), Some(2));
        t.apply(2, &samples(&[30, 40, 50]), 4);
        assert_eq!(t.levels().len(), 5);
        assert_eq!(t.cursor(), None);
        assert_eq!(t.peak(), Some(4));
        assert_eq!(t.range(), Some((10.0, 50.0)));
    }

    #[test]
    fn averages_later_sweeps_into_existing_values() {
        let mut t = Trace::new(params());
        t.apply(0, &samples(&[100; 5]), 4);
        t.apply(0, &samples(&[200; 5]), 4);
        assert_eq!(t.levels()[0], 125.0);
        t.apply(0, &samples(&[200; 5]), 1);
        assert_eq!(t.levels()[0], 200.0);
    }

    #[test]
    fn hold_traces_follow_extremes_and_clear() {
        let mut t = Trace::new(params());
        t.apply(0, &samples(&[10, 50, 30, 30, 30]), 1);
        t.apply(0, &samples(&[20, 40, 30, 30, 30]), 1);
        assert_eq!(t.max_hold()[..2], [20.0, 50.0]);
        assert_eq!(t.min_hold()[..2], [10.0, 40.0]);
        t.clear_hold();
        assert_eq!(t.max_hold()[0], t.levels()[0]);
        assert_eq!(t.min_hold()[1], t.levels()[1]);
    }

    #[test]
    fn finds_separated_peaks_strongest_first() {
        let p = SweepParams::new(35_000_000, 135_000_000, 12).unwrap();
        let mut t = Trace::new(p);
        t.apply(
            0,
            &samples(&[10, 80, 10, 10, 10, 60, 10, 10, 10, 70, 71, 10]),
            1,
        );
        assert_eq!(t.peaks(3, 2), vec![1, 10, 5]);
        assert_eq!(t.peaks(1, 2), vec![1]);
    }

    #[test]
    fn mapped_converts_levels_and_holds() {
        let mut t = Trace::new(params());
        t.apply(0, &samples(&[10, 20, 30, 40, 50]), 1);
        let m = t.mapped(|v| v / 10.0);
        assert_eq!(m.levels(), &[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(m.max_hold()[4], 5.0);
        assert_eq!(m.total(), 5);
        assert_eq!(t.levels()[0], 10.0);
    }

    #[test]
    fn ignores_points_past_the_end() {
        let mut t = Trace::new(params());
        t.apply(3, &samples(&[1, 2, 3, 4]), 1);
        assert_eq!(t.total(), 5);
        assert_eq!(t.levels().len(), 5);
    }
}
