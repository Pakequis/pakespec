//! Drawing pieces shared by the screen styles.

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

use crate::calibration::Calibration;
use crate::i18n::Translator;
use crate::scale::Scale;
use crate::theme::Theme;
use crate::trace::Trace;

/// Vertical divisions of every style.
pub const ROWS: usize = 8;
/// Horizontal divisions of every style.
pub const COLUMNS: usize = 10;

/// Everything a style needs to draw one frame.
pub struct ViewContext<'a> {
    /// The trace in display units: dBm when `calibration` is set, raw otherwise.
    pub trace: &'a Trace,
    /// The same trace in raw detector values.
    pub raw: &'a Trace,
    pub calibration: Option<&'a Calibration>,
    pub scale: Scale,
    pub theme: &'a Theme,
    pub tr: &'a Translator,
    pub show_max_hold: bool,
    pub show_min_hold: bool,
    pub averaging: u8,
    pub sweep_secs: Option<f32>,
    /// Point of the user's marker, if one is set.
    pub marker: Option<usize>,
}

impl ViewContext<'_> {
    /// `true` when the dBm values are an extrapolation of the calibration to
    /// frequencies where it was not measured.
    pub fn is_approximate(&self) -> bool {
        self.calibration.is_some_and(|c| {
            let f = &self.trace.freq_mhz;
            let (lo, hi) = (f[0] * 1e6, f[f.len() - 1] * 1e6);
            lo < c.min_freq_hz as f64 || hi > c.max_freq_hz as f64
        })
    }

    /// Parameter line: reference level, scale, averaging, sweep time.
    pub fn header_text(&self) -> String {
        let avg = if self.averaging <= 1 {
            self.tr.tr("Off").to_string()
        } else {
            self.averaging.to_string()
        };
        let time = self
            .sweep_secs
            .map_or("-".to_string(), |s| format!("{s:.2}"));
        let (unit, div_unit) = if self.calibration.is_some() {
            (" dBm", " dB")
        } else {
            ("", "")
        };
        let mut text = self.tr.tr_args(
            "Ref {ref} · {div}/div · Avg {avg} · Sweep {time} s",
            &[
                ("ref", &format!("{:.0}{unit}", self.scale.top)),
                (
                    "div",
                    &format!("{}{div_unit}", format_step(self.scale.per_div)),
                ),
                ("avg", &avg),
                ("time", &time),
            ],
        );
        if self.is_approximate() {
            text.push_str(" · ");
            text.push_str(self.tr.tr("approximate dBm"));
        }
        text
    }

    /// Frequency line: center, point count, span.
    pub fn footer_text(&self) -> String {
        let f = &self.trace.freq_mhz;
        let (first, last) = (f[0], f[f.len() - 1]);
        self.tr.tr_args(
            "CF {cf} MHz · {points} pts · Span {span} MHz",
            &[
                ("cf", &format!("{:.2}", (first + last) / 2.0)),
                ("points", &self.trace.total().to_string()),
                ("span", &format!("{:.2}", last - first)),
            ],
        )
    }
}

/// A level or step for the axes: up to three decimals, without trailing zeros,
/// so that `f32` rounding noise such as `-33.600002` never reaches the screen.
pub(crate) fn format_step(step: f32) -> String {
    let text = format!("{step:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// The plot area of one frame, with the mapping from data to screen.
pub struct Plot<'a> {
    pub painter: &'a Painter,
    pub rect: Rect,
    pub cx: &'a ViewContext<'a>,
}

impl Plot<'_> {
    pub fn x(&self, i: usize) -> f32 {
        let total = self.cx.trace.total().max(2);
        self.rect.left() + self.rect.width() * i as f32 / (total - 1) as f32
    }

    pub fn y(&self, v: f32) -> f32 {
        let s = &self.cx.scale;
        let t = (v - s.bottom()) / (s.top - s.bottom());
        (self.rect.bottom() - self.rect.height() * t).clamp(self.rect.top(), self.rect.bottom())
    }

    /// Index of the measured point under screen position `x`.
    pub fn index_at(&self, x: f32) -> Option<usize> {
        let measured = self.cx.trace.levels().len();
        if measured == 0 {
            return None;
        }
        let total = self.cx.trace.total().max(2);
        let t = ((x - self.rect.left()) / self.rect.width()).clamp(0.0, 1.0);
        Some(((t * (total - 1) as f32).round() as usize).min(measured - 1))
    }

    pub fn polyline(&self, values: &[f32]) -> Vec<Pos2> {
        values
            .iter()
            .enumerate()
            .map(|(i, &v)| Pos2::new(self.x(i), self.y(v)))
            .collect()
    }

    pub fn line(&self, values: &[f32], stroke: Stroke) {
        self.painter.add(Shape::line(self.polyline(values), stroke));
    }

    /// Grid lines; `minor` adds fine lines (5 per division) in that color.
    pub fn grid(&self, major: Color32, minor: Option<Color32>) {
        let r = self.rect;
        if let Some(color) = minor {
            for row in 0..ROWS {
                for k in 1..5 {
                    let y = r.top() + r.height() * (row as f32 + k as f32 / 5.0) / ROWS as f32;
                    self.painter.hline(r.x_range(), y, Stroke::new(1.0, color));
                }
            }
            for col in 0..COLUMNS {
                for k in 1..5 {
                    let x = r.left() + r.width() * (col as f32 + k as f32 / 5.0) / COLUMNS as f32;
                    self.painter.vline(x, r.y_range(), Stroke::new(1.0, color));
                }
            }
        }
        for row in 0..=ROWS {
            let y = r.top() + r.height() * row as f32 / ROWS as f32;
            self.painter.hline(r.x_range(), y, Stroke::new(1.0, major));
        }
        for col in 0..=COLUMNS {
            let x = r.left() + r.width() * col as f32 / COLUMNS as f32;
            self.painter.vline(x, r.y_range(), Stroke::new(1.0, major));
        }
    }

    /// Level labels on the horizontal grid lines, inside or outside the left edge.
    pub fn y_labels(&self, inside: bool, color: Color32, font: &FontId) {
        let s = &self.cx.scale;
        for row in 0..=ROWS {
            if inside && row == ROWS {
                continue;
            }
            let value = s.top - s.per_div * row as f32;
            let y = self.rect.top() + self.rect.height() * row as f32 / ROWS as f32;
            let (pos, align) = if inside {
                (
                    Pos2::new(self.rect.left() + 4.0, y + 8.0),
                    Align2::LEFT_CENTER,
                )
            } else {
                (Pos2::new(self.rect.left() - 6.0, y), Align2::RIGHT_CENTER)
            };
            self.painter
                .text(pos, align, format_step(value), font.clone(), color);
        }
    }

    /// Frequency labels under the plot, every second grid column.
    pub fn x_labels(&self, color: Color32, font: &FontId) {
        let f = &self.cx.trace.freq_mhz;
        let (first, last) = (f[0], f[f.len() - 1]);
        let decimals = if last - first < 100.0 { 1 } else { 0 };
        for col in (0..=COLUMNS).step_by(2) {
            let x = self.rect.left() + self.rect.width() * col as f32 / COLUMNS as f32;
            let value = first + (last - first) * col as f64 / COLUMNS as f64;
            self.painter.text(
                Pos2::new(x, self.rect.bottom() + 6.0),
                Align2::CENTER_TOP,
                format!("{value:.decimals$}"),
                font.clone(),
                color,
            );
        }
    }

    /// Vertical line where the board is measuring right now.
    pub fn sweep_cursor(&self, color: Color32) {
        if let Some(c) = self.cx.trace.cursor() {
            self.painter
                .vline(self.x(c), self.rect.y_range(), Stroke::new(1.0, color));
        }
    }

    /// Triangle marker on a point, with an optional label beside it.
    pub fn marker(&self, index: usize, color: Color32, label: &str, font: &FontId) {
        let l = self.cx.trace.levels();
        let p = Pos2::new(self.x(index), self.y(l[index]));
        self.painter.add(Shape::convex_polygon(
            vec![
                Pos2::new(p.x, p.y - 3.0),
                Pos2::new(p.x - 5.0, p.y - 12.0),
                Pos2::new(p.x + 5.0, p.y - 12.0),
            ],
            color,
            Stroke::NONE,
        ));
        if !label.is_empty() {
            let right = p.x > self.rect.right() - 140.0;
            let (dx, align) = if right {
                (-8.0, Align2::RIGHT_CENTER)
            } else {
                (8.0, Align2::LEFT_CENTER)
            };
            self.painter.text(
                Pos2::new(p.x + dx, (p.y - 20.0).max(self.rect.top() + 8.0)),
                align,
                label,
                font.clone(),
                color,
            );
        }
    }

    /// Text such as `98.50 MHz  123` for a point, or `98.50 MHz  -21.4 dBm` when
    /// calibrated. A `~` marks a value outside what the calibration measured.
    pub fn readout(&self, index: usize) -> String {
        let mhz = self.cx.trace.freq_mhz[index];
        let level = self.cx.trace.levels()[index];
        match self.cx.calibration {
            Some(c) => {
                let raw = f64::from(self.cx.raw.levels()[index]);
                let mark = if c.is_trusted(raw, (mhz * 1e6).round() as u64) {
                    ""
                } else {
                    "~"
                };
                format!("{mhz:.2} MHz  {mark}{level:.1} dBm")
            }
            None => format!("{mhz:.2} MHz  {level:.0}"),
        }
    }
}

/// Color `c` with the given opacity.
pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

#[cfg(test)]
mod tests {

    #[test]
    fn axis_values_show_no_rounding_noise() {
        assert_eq!(format_step(-33.600002), "-33.6");
        assert_eq!(format_step(-34.800003), "-34.8");
        assert_eq!(format_step(-20.0), "-20");
        assert_eq!(format_step(0.2), "0.2");
        assert_eq!(format_step(-0.0001), "0");
        assert_eq!(format_step(2.5), "2.5");
    }
    use super::*;
    use crate::i18n::Language;
    use crate::protocol::{Sample, SweepParams};

    fn calibration() -> Calibration {
        Calibration {
            counts_per_db: 10.0,
            raw_at_0_dbm: 400.0,
            min_raw: 200.0,
            max_raw: 400.0,
            min_freq_hz: 35_000_000,
            max_freq_hz: 60_000_000,
        }
    }

    /// Shows `raw` readings over `start..stop` Hz and returns (header, readout of point 0).
    fn texts(start: u64, stop: u64, raw: u16, cal: Option<&Calibration>) -> (String, String) {
        let mut trace = Trace::new(SweepParams::new(start, stop, 300).unwrap());
        trace.apply(0, &[Sample { ch1: raw, ch2: raw }; 300], 1);
        let shown = cal.map(|c| trace.mapped(|v| c.raw_to_dbm(f64::from(v)) as f32));
        let theme = crate::theme::ThemeId::Dark.theme();
        let tr = Translator::new(Language::En);
        let cx = ViewContext {
            trace: shown.as_ref().unwrap_or(&trace),
            raw: &trace,
            calibration: cal,
            scale: Scale {
                top: -10.0,
                per_div: 10.0,
                rows: ROWS,
            },
            theme: &theme,
            tr: &tr,
            show_max_hold: false,
            show_min_hold: false,
            averaging: 1,
            sweep_secs: None,
            marker: None,
        };
        let ctx = eframe::egui::Context::default();
        let painter = ctx.layer_painter(eframe::egui::LayerId::background());
        let plot = Plot {
            painter: &painter,
            rect: Rect::from_min_size(Pos2::ZERO, eframe::egui::vec2(100.0, 100.0)),
            cx: &cx,
        };
        (cx.header_text(), plot.readout(0))
    }

    #[test]
    fn raw_values_without_calibration() {
        let (header, readout) = texts(35_000_000, 55_000_000, 300, None);
        assert!(header.starts_with("Ref -10 · 10/div"));
        assert!(readout.ends_with("300"));
    }

    #[test]
    fn dbm_inside_the_calibrated_range() {
        let c = calibration();
        let (header, readout) = texts(35_000_000, 55_000_000, 300, Some(&c));
        assert!(header.starts_with("Ref -10 dBm · 10 dB/div"));
        assert!(!header.contains("approximate"));
        assert!(readout.ends_with("-10.0 dBm") && !readout.contains('~'));
    }

    #[test]
    fn extrapolated_values_are_marked() {
        let c = calibration();
        let (header, readout) = texts(35_000_000, 2_000_000_000, 300, Some(&c));
        assert!(header.contains("approximate dBm"));
        let (_, noise) = texts(35_000_000, 55_000_000, 150, Some(&c));
        assert!(noise.contains("~-25.0 dBm"));
        assert!(readout.contains("-10.0 dBm"));
    }
}
