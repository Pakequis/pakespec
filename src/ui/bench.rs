//! Bench style: parameter header, grid with level labels inside, thin trace, footer.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke};

use super::draw::{Plot, ViewContext};
use super::rgb;

pub fn show(ui: &mut egui::Ui, cx: &ViewContext) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
    let outer = response.rect;
    let th = cx.theme;
    let font = FontId::proportional(12.0);
    let bar = 24.0;

    let header = Rect::from_min_max(outer.min, Pos2::new(outer.max.x, outer.min.y + bar));
    let footer = Rect::from_min_max(Pos2::new(outer.min.x, outer.max.y - bar), outer.max);
    painter.rect_filled(header, 0.0, rgb(th.panel));
    painter.rect_filled(footer, 0.0, rgb(th.panel));
    painter.text(
        Pos2::new(header.left() + 10.0, header.center().y),
        Align2::LEFT_CENTER,
        cx.header_text(),
        font.clone(),
        rgb(th.text),
    );
    painter.text(
        Pos2::new(footer.left() + 10.0, footer.center().y),
        Align2::LEFT_CENTER,
        cx.footer_text(),
        font.clone(),
        rgb(th.text),
    );

    let rect = Rect::from_min_max(
        Pos2::new(outer.left() + 8.0, header.bottom() + 8.0),
        Pos2::new(outer.right() - 8.0, footer.top() - 8.0),
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
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, rgb(th.axis)),
        egui::StrokeKind::Inside,
    );
    plot.y_labels(true, rgb(th.text_dim), &font);

    if cx.show_min_hold {
        plot.line(
            cx.trace.min_hold(),
            Stroke::new(1.0, rgb(th.trace_secondary)),
        );
    }
    if cx.show_max_hold {
        plot.line(cx.trace.max_hold(), Stroke::new(1.0, rgb(th.marker)));
    }
    plot.line(cx.trace.levels(), Stroke::new(1.3, rgb(th.trace)));
    plot.sweep_cursor(rgb(th.axis));

    let hovered = response
        .hover_pos()
        .filter(|p| rect.contains(*p))
        .and_then(|p| plot.index_at(p.x));
    if let Some(m) = hovered
        .or(cx.marker)
        .or_else(|| cx.trace.peaks(1, 5).first().copied())
    {
        plot.marker(
            m,
            rgb(th.marker),
            &format!("M1  {}", plot.readout(m)),
            &font,
        );
    }
}
