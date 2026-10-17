//! Generator screen: the frequency, the live detector reading and its history.

use std::collections::VecDeque;

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke};

use super::draw::ROWS;
use super::rgb;
use crate::i18n::Translator;
use crate::scale::Scale;
use crate::theme::Theme;

/// Readings at or below this value are at the noise floor of the detector.
const NOISE_LIMIT: u16 = 200;

pub fn show(
    ui: &mut egui::Ui,
    freq_mhz: f64,
    readings: &VecDeque<u16>,
    theme: &Theme,
    tr: &Translator,
) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
    let outer = response.rect;
    let big = FontId::monospace(34.0);
    let medium = FontId::monospace(18.0);
    let small = FontId::proportional(13.0);

    painter.text(
        Pos2::new(outer.left() + 24.0, outer.top() + 34.0),
        Align2::LEFT_CENTER,
        format!("{} {:.3} MHz", tr.tr("Generator"), freq_mhz),
        big,
        rgb(theme.text),
    );
    let last = readings.back().copied();
    painter.text(
        Pos2::new(outer.left() + 24.0, outer.top() + 76.0),
        Align2::LEFT_CENTER,
        format!(
            "{}: {}",
            tr.tr("Level (raw)"),
            last.map_or("-".to_string(), |v| v.to_string())
        ),
        medium,
        rgb(theme.trace),
    );
    let hint = match last {
        Some(v) if v <= NOISE_LIMIT => {
            tr.tr("No signal: check the cable from RF OUT to RF IN and that the output is on.")
        }
        _ => tr.tr("The output works without S1; sweeps with the tracking source need S1 on."),
    };
    let hint_color = match last {
        Some(v) if v <= NOISE_LIMIT => rgb(theme.marker),
        _ => rgb(theme.text_dim),
    };
    painter.text(
        Pos2::new(outer.left() + 24.0, outer.top() + 106.0),
        Align2::LEFT_CENTER,
        hint,
        small.clone(),
        hint_color,
    );

    let plot = Rect::from_min_max(
        Pos2::new(outer.left() + 60.0, outer.top() + 130.0),
        Pos2::new(outer.right() - 20.0, outer.bottom() - 24.0),
    );
    if plot.width() < 60.0 || plot.height() < 60.0 || readings.len() < 2 {
        return;
    }
    let lo = readings.iter().copied().min().unwrap_or(0) as f32;
    let hi = readings.iter().copied().max().unwrap_or(1) as f32;
    let scale = Scale::fit(lo, hi, ROWS);
    let grid = Stroke::new(1.0, rgb(theme.grid));
    for row in 0..=ROWS {
        let y = plot.top() + plot.height() * row as f32 / ROWS as f32;
        painter.hline(plot.x_range(), y, grid);
        painter.text(
            Pos2::new(plot.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            format!("{}", scale.top - scale.per_div * row as f32),
            small.clone(),
            rgb(theme.text_dim),
        );
    }
    let y_of = |v: f32| {
        let t = (v - scale.bottom()) / (scale.top - scale.bottom());
        (plot.bottom() - plot.height() * t).clamp(plot.top(), plot.bottom())
    };
    let line: Vec<Pos2> = readings
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let x = plot.right() - plot.width() * (readings.len() - 1 - i) as f32 / 299.0;
            Pos2::new(x, y_of(f32::from(v)))
        })
        .collect();
    painter.add(egui::Shape::line(line, Stroke::new(1.5, rgb(theme.trace))));
}
