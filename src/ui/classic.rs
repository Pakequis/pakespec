//! Classic style: fine grid, live trace with max and min hold, cursor readout.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke};

use super::draw::{Plot, ViewContext, with_alpha};
use super::rgb;

pub fn show(ui: &mut egui::Ui, cx: &ViewContext) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
    let outer = response.rect;
    let th = cx.theme;
    let font = FontId::proportional(11.5);
    let rect = Rect::from_min_max(
        Pos2::new(outer.left() + 52.0, outer.top() + 30.0),
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
    plot.grid(rgb(th.axis), Some(with_alpha(rgb(th.grid), 140)));
    plot.y_labels(false, rgb(th.text_dim), &font);
    plot.x_labels(rgb(th.text_dim), &font);
    painter.text(
        Pos2::new(rect.left(), outer.top() + 14.0),
        Align2::LEFT_CENTER,
        cx.header_text(),
        font.clone(),
        rgb(th.text_dim),
    );

    if cx.show_min_hold {
        plot.line(
            cx.trace.min_hold(),
            Stroke::new(1.0, rgb(th.trace_secondary)),
        );
    }
    if cx.show_max_hold {
        plot.line(cx.trace.max_hold(), Stroke::new(1.0, rgb(th.marker)));
    }
    plot.line(cx.trace.levels(), Stroke::new(1.4, rgb(th.trace)));
    plot.sweep_cursor(rgb(th.axis));
    if let Some(m) = cx.marker {
        plot.marker(
            m,
            rgb(th.marker),
            &format!("M1  {}", plot.readout(m)),
            &font,
        );
    }

    if let Some(i) = response
        .hover_pos()
        .filter(|p| rect.contains(*p))
        .and_then(|p| plot.index_at(p.x))
    {
        let (x, y) = (plot.x(i), plot.y(cx.trace.levels()[i]));
        let guide = Stroke::new(1.0, with_alpha(rgb(th.text), 150));
        painter.vline(x, rect.y_range(), guide);
        painter.hline(rect.x_range(), y, guide);
        painter.text(
            Pos2::new(rect.right() - 6.0, outer.top() + 14.0),
            Align2::RIGHT_CENTER,
            plot.readout(i),
            FontId::monospace(12.0),
            rgb(th.text),
        );
    }
}
