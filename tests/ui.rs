//! Tests of the graphical application: they click on its controls through
//! egui's test harness, with the board simulator as the port.

#![cfg(feature = "gui")]

use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use pakespec::settings::Settings;
use pakespec::ui::PakespecApp;

fn app() -> Harness<'static, PakespecApp> {
    let settings = Settings {
        port: Some("simulator".into()),
        language: Some(pakespec::i18n::Language::En),
        start_hz: 35_000_000,
        stop_hz: 400_000_000,
        points: 100,
        ..Settings::default()
    };
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1100.0, 700.0))
        .build_eframe(move |cc| PakespecApp::with_settings(cc, settings, None));
    harness.run();
    harness
}

/// Steps the harness until `done` holds or a few seconds pass; the simulator
/// answers from another thread.
fn wait_for(
    harness: &mut Harness<'static, PakespecApp>,
    done: impl Fn(&Harness<'static, PakespecApp>) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        harness.step();
        if done(harness) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    false
}

/// Text of the first control whose label (or value) contains `text`. Labels such
/// as the entry display keep their text in the value, buttons in the label.
fn first_label(harness: &Harness<'_, PakespecApp>, text: &str) -> Option<String> {
    harness.query_all_by_label_contains(text).find_map(|n| {
        let node = n.accesskit_node();
        node.label().or_else(|| node.value())
    })
}

fn click_first(harness: &mut Harness<'static, PakespecApp>, text: &str) {
    harness
        .query_all_by_label_contains(text)
        .next()
        .unwrap_or_else(|| panic!("no control with {text:?}"))
        .click();
    harness.run();
}

#[test]
fn starts_with_the_frequency_menu() {
    let harness = app();
    assert!(
        harness
            .query_all_by_label_contains("Center")
            .next()
            .is_some()
    );
    assert!(harness.query_by_label_contains("Start freq").is_some());
    assert!(
        harness
            .query_by_label_contains("Waiting for the first sweep")
            .is_some()
    );
}

#[test]
fn typing_a_center_frequency_changes_the_range() {
    let mut harness = app();
    click_first(&mut harness, "Center");
    for key in ["1", "5", "0"] {
        harness.get_by_label(key).click();
        harness.run();
    }
    harness.get_by_label("MHz").click();
    harness.run();
    // The center is kept as typed: 150 MHz, with the span shrunk to fit above 35 MHz.
    assert!(
        first_label(&harness, "Center")
            .unwrap()
            .contains("150.000 MHz")
    );
}

#[test]
fn a_sweep_draws_the_header_and_the_footer() {
    let mut harness = app();
    harness.get_by_label("Sweep").click();
    assert!(wait_for(&mut harness, |h| h
        .query_by_label_contains("Waiting")
        .is_none()));
    // The plot is painted, not a widget: the controls now offer to stop.
    assert!(harness.query_by_label("Stop").is_some());
}

#[test]
fn the_generator_screen_reads_the_detector() {
    let mut harness = app();
    harness.get_by_label("GEN").click();
    harness.run();
    assert!(harness.query_by_label_contains("Generator freq").is_some());
    click_first(&mut harness, "Output");
    // The simulator answers `m`; the "Reading" soft key then holds a number.
    assert!(wait_for(&mut harness, |h| {
        first_label(h, "Reading").is_some_and(|l| l.chars().any(|c| c.is_ascii_digit()))
    }));
    // The spectrum waiting text is replaced by the generator screen, and the
    // output key is lit as "On".
    assert!(first_label(&harness, "Output").unwrap().contains("On"));
    click_first(&mut harness, "Output");
    assert!(first_label(&harness, "Output").unwrap().contains("Off"));
}

#[test]
fn the_generator_frequency_can_be_typed() {
    let mut harness = app();
    harness.get_by_label("GEN").click();
    harness.run();
    click_first(&mut harness, "Generator freq");
    for key in ["9", "4", "0"] {
        harness.get_by_label(key).click();
        harness.run();
    }
    harness.get_by_label("MHz").click();
    harness.run();
    assert!(
        first_label(&harness, "Generator freq")
            .unwrap()
            .contains("940.000 MHz")
    );
}

#[test]
fn a_frequency_out_of_range_is_clamped() {
    let mut harness = app();
    harness.get_by_label("GEN").click();
    harness.run();
    click_first(&mut harness, "Generator freq");
    for key in ["9", "9", "9", "9"] {
        harness.get_by_label(key).click();
        harness.run();
    }
    harness.get_by_label("MHz").click();
    harness.run();
    assert!(
        first_label(&harness, "Generator freq")
            .unwrap()
            .contains("4400.000 MHz")
    );
}

/// Same flow as the simulator test, on the real board (S1 on, cable OUT to IN).
#[test]
#[ignore = "needs the board with the tracking source on"]
fn the_application_reads_the_real_generator() {
    let port = std::env::var("PAKESPEC_PORT").unwrap_or_else(|_| "/dev/ttyUSB0".into());
    let settings = Settings {
        port: Some(port),
        language: Some(pakespec::i18n::Language::En),
        ..Settings::default()
    };
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1100.0, 700.0))
        .build_eframe(move |cc| PakespecApp::with_settings(cc, settings, None));
    harness.run();
    harness.get_by_label("GEN").click();
    harness.run();
    click_first(&mut harness, "Output");
    // 100 MHz through the cable reads high on the real board.
    assert!(wait_for(&mut harness, |h| {
        first_label(h, "Reading")
            .and_then(|l| {
                l.split_whitespace()
                    .last()
                    .and_then(|n| n.parse::<u32>().ok())
            })
            .is_some_and(|v| v > 400)
    }));
    click_first(&mut harness, "Output");
}

/// A sweep started from the application runs on the real board without errors.
#[test]
#[ignore = "needs the board with the tracking source on"]
fn the_application_sweeps_the_real_board() {
    let port = std::env::var("PAKESPEC_PORT").unwrap_or_else(|_| "/dev/ttyUSB0".into());
    let settings = Settings {
        port: Some(port),
        language: Some(pakespec::i18n::Language::En),
        start_hz: 35_000_000,
        stop_hz: 4_400_000_000,
        points: 438,
        ..Settings::default()
    };
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1100.0, 700.0))
        .build_eframe(move |cc| PakespecApp::with_settings(cc, settings, None));
    harness.run();
    harness.get_by_label("Sweep").click();
    // Let a few full sweeps go by (each takes about 0.6 s).
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(4) {
        harness.step();
        std::thread::sleep(Duration::from_millis(30));
    }
    assert!(harness.query_by_label_contains("failed").is_none());
    assert!(harness.query_by_label_contains("Sweeping").is_some());
    harness.get_by_label("Stop").click();
    assert!(wait_for(&mut harness, |h| h
        .query_by_label_contains("Stopped")
        .is_some()));
}

#[test]
fn compact_controls_can_return_to_the_front_panel() {
    let settings = Settings {
        port: Some("simulator".into()),
        language: Some(pakespec::i18n::Language::En),
        compact_controls: true,
        ..Settings::default()
    };
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1100.0, 700.0))
        .build_eframe(move |cc| PakespecApp::with_settings(cc, settings, None));
    harness.run();
    // The plain controls bar is showing: no hard keys yet.
    assert!(harness.query_by_label("FREQ").is_none());
    harness.get_by_label("Front panel").click();
    harness.run();
    assert!(harness.query_by_label("FREQ").is_some());
    // And the SYSTEM menu switches back to the compact bar.
    harness.get_by_label("SYSTEM").click();
    harness.run();
    click_first(&mut harness, "Compact controls");
    assert!(harness.query_by_label("Front panel").is_some());
}
