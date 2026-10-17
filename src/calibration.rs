//! Conversion of the board's raw detector value to dBm.
//!
//! The detector is a straight line in dB over its working range: `raw = slope * dBm + offset`.
//! A calibration stores that line, the raw values it was measured over and the
//! frequency range where it was measured, and is kept in one TOML file per board.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Readings this close to the lowest measured raw value are treated as noise
/// floor and left out of the fit (raw counts).
const FLOOR_MARGIN: f64 = 10.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// Raw counts per dB.
    pub counts_per_db: f64,
    /// Raw value that the line gives at 0 dBm.
    pub raw_at_0_dbm: f64,
    /// Lowest and highest raw value the line was measured over; outside it the
    /// detector is at the noise floor or the line was not checked.
    pub min_raw: f64,
    pub max_raw: f64,
    /// Frequency range (Hz) where the line was measured. Elsewhere the detector
    /// response may differ.
    pub min_freq_hz: u64,
    pub max_freq_hz: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FitError {
    /// Fewer than three readings above the noise floor.
    TooFewPoints,
    /// The readings cover less than 10 dB, or the fit is not increasing.
    BadRange,
}

impl std::fmt::Display for FitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewPoints => f.write_str("need at least three readings above the noise floor"),
            Self::BadRange => {
                f.write_str("the readings must cover at least 10 dB and rise with the level")
            }
        }
    }
}

impl std::error::Error for FitError {}

impl Calibration {
    /// Least-squares line through `(dbm, raw)` pairs measured at frequencies
    /// from `min_freq_hz` to `max_freq_hz`. Readings at the noise floor (within
    /// `FLOOR_MARGIN` of the lowest raw value) are ignored.
    pub fn fit(
        points: &[(f64, f64)],
        min_freq_hz: u64,
        max_freq_hz: u64,
    ) -> Result<Calibration, FitError> {
        let floor = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        let used: Vec<(f64, f64)> = points
            .iter()
            .copied()
            .filter(|p| p.1 > floor + FLOOR_MARGIN)
            .collect();
        if used.len() < 3 {
            return Err(FitError::TooFewPoints);
        }
        let n = used.len() as f64;
        let mean_x = used.iter().map(|p| p.0).sum::<f64>() / n;
        let mean_y = used.iter().map(|p| p.1).sum::<f64>() / n;
        let sxx: f64 = used.iter().map(|p| (p.0 - mean_x).powi(2)).sum();
        let sxy: f64 = used.iter().map(|p| (p.0 - mean_x) * (p.1 - mean_y)).sum();
        let (lo, hi) = used.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.0), hi.max(p.0))
        });
        if sxx == 0.0 || hi - lo < 10.0 || sxy <= 0.0 {
            return Err(FitError::BadRange);
        }
        let slope = sxy / sxx;
        Ok(Calibration {
            counts_per_db: slope,
            raw_at_0_dbm: mean_y - slope * mean_x,
            min_raw: used.iter().map(|p| p.1).fold(f64::MAX, f64::min),
            max_raw: used.iter().map(|p| p.1).fold(f64::MIN, f64::max),
            min_freq_hz,
            max_freq_hz,
        })
    }

    pub fn raw_to_dbm(&self, raw: f64) -> f64 {
        (raw - self.raw_at_0_dbm) / self.counts_per_db
    }

    pub fn dbm_to_raw(&self, dbm: f64) -> f64 {
        dbm * self.counts_per_db + self.raw_at_0_dbm
    }

    /// `true` when the reading is inside the raw range and the frequency inside
    /// the frequency range that were measured, so the dBm value can be trusted.
    pub fn is_trusted(&self, raw: f64, freq_hz: u64) -> bool {
        (self.min_raw..=self.max_raw).contains(&raw)
            && (self.min_freq_hz..=self.max_freq_hz).contains(&freq_hz)
    }

    /// Loads a calibration file; `None` when it is missing or damaged.
    pub fn load(path: &Path) -> Option<Calibration> {
        let text = std::fs::read_to_string(path).ok()?;
        toml::from_str(&text).ok()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(
            path,
            toml::to_string_pretty(self).map_err(io::Error::other)?,
        )
    }
}

/// Parses a ladder file: one `dbm,raw` pair per line, `#` comments and an
/// optional header line are ignored.
pub fn parse_ladder(text: &str) -> Vec<(f64, f64)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('#') {
                return None;
            }
            let (a, b) = line.split_once(',')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect()
}

/// Location of the calibration file for this platform.
pub fn default_path() -> Option<PathBuf> {
    crate::settings::default_path().map(|p| p.with_file_name("calibration.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Measured on the board with a function generator at 50 MHz.
    const LADDER: &[(f64, f64)] = &[
        (-56.0, 150.4),
        (-50.0, 150.8),
        (-42.0, 152.6),
        (-36.0, 153.9),
        (-32.5, 168.0),
        (-30.0, 184.9),
        (-26.5, 214.6),
        (-22.0, 254.1),
        (-19.1, 281.4),
        (-16.0, 305.6),
        (-12.5, 337.1),
        (-10.0, 359.4),
        (-6.5, 388.7),
        (-4.0, 408.7),
        (-2.0, 424.7),
    ];

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pakespec-cal-{}-{name}", std::process::id()))
    }

    #[test]
    fn fits_the_measured_ladder_and_ignores_the_noise_floor() {
        let c = Calibration::fit(LADDER, 35_000_000, 60_000_000).unwrap();
        assert!((c.counts_per_db - 8.55).abs() < 0.1);
        assert!((c.raw_at_0_dbm - 443.3).abs() < 1.0);
        assert_eq!(c.min_raw, 168.0);
        assert_eq!(c.max_raw, 424.7);
        for &(dbm, raw) in &LADDER[4..] {
            assert!((c.raw_to_dbm(raw) - dbm).abs() < 0.5, "{dbm} dBm");
        }
    }

    #[test]
    fn conversion_round_trips() {
        let c = Calibration::fit(LADDER, 35_000_000, 60_000_000).unwrap();
        assert!((c.raw_to_dbm(c.dbm_to_raw(-17.3)) + 17.3).abs() < 1e-9);
    }

    #[test]
    fn trust_needs_both_raw_and_frequency_in_range() {
        let c = Calibration::fit(LADDER, 35_000_000, 60_000_000).unwrap();
        assert!(c.is_trusted(300.0, 50_000_000));
        assert!(!c.is_trusted(150.0, 50_000_000)); // noise floor
        assert!(!c.is_trusted(450.0, 50_000_000)); // above what was measured
        assert!(!c.is_trusted(300.0, 2_400_000_000)); // other frequency
    }

    #[test]
    fn rejects_unusable_data() {
        assert_eq!(
            Calibration::fit(&[(-50.0, 150.0), (-40.0, 151.0)], 0, 1),
            Err(FitError::TooFewPoints)
        );
        let narrow = [
            (-20.0, 150.0),
            (-19.0, 200.0),
            (-18.0, 210.0),
            (-17.0, 220.0),
        ];
        assert_eq!(Calibration::fit(&narrow, 0, 1), Err(FitError::BadRange));
        let falling = [
            (-50.0, 150.0),
            (-40.0, 300.0),
            (-30.0, 250.0),
            (-20.0, 200.0),
        ];
        assert_eq!(Calibration::fit(&falling, 0, 1), Err(FitError::BadRange));
    }

    #[test]
    fn round_trip_through_a_file() {
        let c = Calibration::fit(LADDER, 35_000_000, 60_000_000).unwrap();
        let path = temp_file("a").join("calibration.toml");
        c.save(&path).unwrap();
        assert_eq!(Calibration::load(&path), Some(c));
        assert_eq!(Calibration::load(&temp_file("missing")), None);
    }

    #[test]
    fn parses_a_ladder_file_with_header_and_comments() {
        let text = "# generator ladder\ndbm,raw\n-30,184.9\n -20 , 260.5 \n";
        assert_eq!(parse_ladder(text), vec![(-30.0, 184.9), (-20.0, 260.5)]);
    }
}
