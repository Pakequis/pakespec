//! Modern style: flat plot with max-hold shading and peak frequency labels.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Shape, Stroke};

use super::draw::{Plot, ViewContext, with_alpha};
use super::rgb;

pub fn show(ui: &mut egui::Ui, cx: &ViewContext) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
    let outer = response.rect;
    let th = cx.theme;
    let font = FontId::proportional(11.5);
    let rect = Rect::from_min_max(
        Pos2::new(outer.left() + 52.0, outer.top() + 14.0),
        Pos2::new(outer.right() - 12.0, outer.bottom() - 28.0),
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
    plot.y_labels(false, rgb(th.text_dim), &font);
    plot.x_labels(rgb(th.text_dim), &font);

    if cx.show_max_hold {
        // Shade the area under the max-hold envelope, one convex quad per segment.
        let line = plot.polyline(cx.trace.max_hold());
        let fill = with_alpha(rgb(th.marker), 38);
        for pair in line.windows(2) {
            painter.add(Shape::convex_polygon(
                vec![
                    pair[0],
                    pair[1],
                    Pos2::new(pair[1].x, rect.bottom()),
                    Pos2::new(pair[0].x, rect.bottom()),
                ],
                fill,
                Stroke::NONE,
            ));
        }
        plot.line(
            cx.trace.max_hold(),
            Stroke::new(1.0, with_alpha(rgb(th.marker), 160)),
        );
    }
    if cx.show_min_hold {
        plot.line(
            cx.trace.min_hold(),
            Stroke::new(1.0, rgb(th.trace_secondary)),
        );
    }
    plot.line(cx.trace.levels(), Stroke::new(1.5, rgb(th.trace)));
    plot.sweep_cursor(rgb(th.axis));
    if let Some(m) = cx.marker {
        plot.marker(
            m,
            rgb(th.marker),
            &format!("M1  {}", plot.readout(m)),
            &font,
        );
    }

    // Frequency of the four strongest peaks, written above them.
    for &p in &cx.trace.peaks(4, 5) {
        let x = plot.x(p);
        let y = plot.y(cx.trace.levels()[p]) - 6.0;
        painter.text(
            Pos2::new(x, y.max(rect.top() + 8.0)),
            Align2::CENTER_BOTTOM,
            format!("{:.1}", cx.trace.freq_mhz[p]),
            font.clone(),
            rgb(th.marker),
        );
    }

    if let Some(i) = response
        .hover_pos()
        .filter(|p| rect.contains(*p))
        .and_then(|p| plot.index_at(p.x))
    {
        let (x, y) = (plot.x(i), plot.y(cx.trace.levels()[i]));
        let guide = Stroke::new(1.0, with_alpha(rgb(th.text_dim), 200));
        painter.vline(x, rect.y_range(), guide);
        painter.hline(rect.x_range(), y, guide);
        let right = x > rect.right() - 170.0;
        painter.text(
            Pos2::new(x + if right { -8.0 } else { 8.0 }, rect.top() + 10.0),
            if right {
                Align2::RIGHT_CENTER
            } else {
                Align2::LEFT_CENTER
            },
            plot.readout(i),
            font,
            rgb(th.text),
        );
    }
}
