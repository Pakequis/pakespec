//! Instrument style: graticule, glowing trace and automatic peak markers, like a bench analyzer.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke};

use super::draw::{Plot, ViewContext, with_alpha};
use super::rgb;

pub fn show(ui: &mut egui::Ui, cx: &ViewContext) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
    let outer = response.rect;
    let th = cx.theme;
    let font = FontId::monospace(12.0);
    let small = FontId::monospace(10.5);
    let rect = Rect::from_min_max(
        Pos2::new(outer.left() + 16.0, outer.top() + 30.0),
        Pos2::new(outer.right() - 16.0, outer.bottom() - 30.0),
    );
    if rect.width() < 60.0 || rect.height() < 60.0 {
        return;
    }
    let plot = Plot {
        painter: &painter,
        rect,
        cx,
    };
    plot.grid(rgb(th.grid), None);

    // Fine ticks on the center line.
    let mid = rect.center().y;
    for t in 0..=50 {
        let x = rect.left() + rect.width() * t as f32 / 50.0;
        painter.vline(x, (mid - 3.0)..=(mid + 3.0), Stroke::new(1.0, rgb(th.axis)));
    }
    plot.y_labels(true, rgb(th.text_dim), &small);
    painter.text(
        Pos2::new(rect.left(), outer.top() + 14.0),
        Align2::LEFT_CENTER,
        cx.header_text().to_uppercase(),
        font.clone(),
        rgb(th.text_dim),
    );
    let f = &cx.trace.freq_mhz;
    let (first, last) = (f[0], f[f.len() - 1]);
    let y = outer.bottom() - 14.0;
    painter.text(
        Pos2::new(rect.left(), y),
        Align2::LEFT_CENTER,
        format!("START {first:.2} MHz"),
        font.clone(),
        rgb(th.text_dim),
    );
    painter.text(
        Pos2::new(rect.center().x, y),
        Align2::CENTER_CENTER,
        format!("CF {:.2} MHz", (first + last) / 2.0),
        font.clone(),
        rgb(th.text_dim),
    );
    painter.text(
        Pos2::new(rect.right(), y),
        Align2::RIGHT_CENTER,
        format!("STOP {last:.2} MHz"),
        font.clone(),
        rgb(th.text_dim),
    );

    if cx.show_min_hold {
        plot.line(
            cx.trace.min_hold(),
            Stroke::new(1.0, with_alpha(rgb(th.trace_secondary), 170)),
        );
    }
    if cx.show_max_hold {
        plot.line(
            cx.trace.max_hold(),
            Stroke::new(1.0, with_alpha(rgb(th.marker), 170)),
        );
    }
    let glow = with_alpha(rgb(th.trace), 50);
    plot.line(cx.trace.levels(), Stroke::new(4.0, glow));
    plot.line(cx.trace.levels(), Stroke::new(1.4, rgb(th.trace)));
    plot.sweep_cursor(rgb(th.axis));

    // Automatic markers on the three strongest peaks, readouts at the top right.
    let marker = rgb(th.marker);
    let markers: Vec<usize> = match cx.marker {
        Some(m) => vec![m],
        None => cx.trace.peaks(3, 5),
    };
    for (n, &p) in markers.iter().enumerate() {
        plot.marker(p, marker, &format!("M{}", n + 1), &font);
        painter.text(
            Pos2::new(rect.right() - 6.0, outer.top() + 14.0 + 13.0 * n as f32),
            Align2::RIGHT_CENTER,
            format!("M{} {}", n + 1, plot.readout(p)),
            small.clone(),
            marker,
        );
    }
    if let Some(i) = response
        .hover_pos()
        .filter(|p| rect.contains(*p))
        .and_then(|p| plot.index_at(p.x))
    {
        painter.vline(
            plot.x(i),
            rect.y_range(),
            Stroke::new(1.0, with_alpha(rgb(th.text), 120)),
        );
    }
}
