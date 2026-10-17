//! Drawing pieces shared by the screen styles.

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

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
    pub trace: &'a Trace,
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
        self.tr.tr_args(
            "Ref {ref} · {div}/div · Avg {avg} · Sweep {time} s",
            &[
                ("ref", &format!("{:.0}", self.scale.top)),
                ("div", &format_step(self.scale.per_div)),
                ("avg", &avg),
                ("time", &time),
            ],
        )
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

fn format_step(step: f32) -> String {
    if step.fract() == 0.0 {
        format!("{step:.0}")
    } else {
        format!("{step}")
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

    /// Text such as `98.50 MHz  123` for a point.
    pub fn readout(&self, index: usize) -> String {
        format!(
            "{:.2} MHz  {:.0}",
            self.cx.trace.freq_mhz[index],
            self.cx.trace.levels()[index]
        )
    }
}

/// Color `c` with the given opacity.
pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}
