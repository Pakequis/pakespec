//! Graphical application.

use eframe::egui;
use pakespec::ui::PakespecApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pakespec")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Pakespec",
        options,
        Box::new(|cc| Ok(Box::new(PakespecApp::new(cc)))),
    )
}
