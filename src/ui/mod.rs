//! The graphical application (egui).

mod bench;
mod classic;
mod draw;
mod generator;
mod instrument;
mod modern;
mod panel;

use eframe::egui;

use crate::calibration::{self, Calibration};
use crate::i18n::{LANGUAGES, Language, Translator};
use crate::ports::{self, PortInfo};
use crate::protocol::SweepParams;
use crate::scale::Scale;
use crate::settings::{self, PRESETS, Settings, ViewStyle};
use crate::theme::{Theme, ThemeId};
use crate::trace::Trace;
use crate::worker::{Event, Source, Worker};
use draw::{ROWS, ViewContext};

/// Name stored in the settings for the built-in simulator.
pub(crate) const SIMULATOR: &str = "simulator";

pub struct PakespecApp {
    settings: Settings,
    settings_path: Option<std::path::PathBuf>,
    translator: Translator,
    theme: Theme,
    ports: Vec<PortInfo>,
    worker: Option<Worker>,
    worker_source: Option<Source>,
    running: bool,
    trace: Option<Trace>,
    /// dBm calibration of the board; the screen shows raw values without it.
    calibration: Option<Calibration>,
    /// Parameters the worker is sweeping with right now.
    applied: Option<SweepParams>,
    scale: Option<Scale>,
    panel: panel::PanelState,
    /// Generator frequency in MHz (real).
    gen_mhz: f64,
    gen_on: bool,
    readings: std::collections::VecDeque<u16>,
    last_sweep_done: Option<std::time::Instant>,
    sweep_secs: Option<f32>,
    status: String,
    failed: bool,
    start_mhz: f64,
    stop_mhz: f64,
    points: u16,
}

impl PakespecApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings_path = settings::default_path();
        let settings = settings_path
            .as_deref()
            .map(Settings::load)
            .unwrap_or_default();
        let mut app = Self::with_settings(cc, settings, settings_path);
        app.calibration = Some(Calibration::load_or_standard(
            calibration::default_path().as_deref(),
        ));
        app
    }

    /// Creates the application with the given settings; they are saved to
    /// `settings_path` when it is `Some`. Tests pass `None` to leave the user's
    /// settings file alone.
    pub fn with_settings(
        cc: &eframe::CreationContext<'_>,
        settings: Settings,
        settings_path: Option<std::path::PathBuf>,
    ) -> Self {
        let ports = ports::list_ports();
        let mut app = Self {
            translator: Translator::new(settings.effective_language()),
            theme: settings.theme.theme(),
            start_mhz: settings.start_hz as f64 / 1e6,
            stop_mhz: settings.stop_hz as f64 / 1e6,
            // A saved file may hold too few points for its range.
            points: settings
                .points
                .max(SweepParams::min_points(settings.start_hz, settings.stop_hz)),
            settings,
            settings_path,
            ports,
            worker: None,
            worker_source: None,
            running: false,
            trace: None,
            calibration: None,
            applied: None,
            scale: None,
            panel: panel::PanelState::default(),
            gen_mhz: 100.0,
            gen_on: false,
            readings: std::collections::VecDeque::new(),
            last_sweep_done: None,
            sweep_secs: None,
            status: String::new(),
            failed: false,
        };
        if app.settings.port.is_none() {
            app.settings.port = ports::suggest(&app.ports).map(|p| p.path.clone());
        }
        app.status = app.translator.tr("Ready").to_string();
        apply_theme(&cc.egui_ctx, &app.theme);
        app
    }

    fn tr<'a>(&'a self, text: &'a str) -> &'a str {
        self.translator.tr(text)
    }

    fn save_settings(&self) {
        if let Some(path) = &self.settings_path {
            let _ = self.settings.save(path);
        }
    }

    fn source(&self) -> Option<Source> {
        match self.settings.port.as_deref() {
            Some(SIMULATOR) => Some(Source::Simulator),
            Some(path) => Some(Source::Serial(path.to_string())),
            None => None,
        }
    }

    /// Makes sure a worker exists for the selected port; reports a problem on screen.
    fn ensure_worker(&mut self, ctx: &egui::Context) -> bool {
        let Some(source) = self.source() else {
            self.status = self.tr("Select a port first").to_string();
            self.failed = true;
            return false;
        };
        if self.worker_source.as_ref() != Some(&source) {
            let repaint = ctx.clone();
            self.worker = Some(Worker::spawn(source.clone(), move || {
                repaint.request_repaint()
            }));
            self.worker_source = Some(source);
        }
        true
    }

    fn toggle_generator(&mut self, ctx: &egui::Context) {
        if self.gen_on {
            if let Some(w) = &self.worker {
                w.generator_off();
            }
            self.gen_on = false;
            self.status = self.tr("Stopped").to_string();
            return;
        }
        if !self.ensure_worker(ctx) {
            return;
        }
        if let Some(w) = &self.worker {
            w.generator_on((self.gen_mhz * 1e6).round() as u64);
        }
        self.running = false;
        self.gen_on = true;
        self.failed = false;
        self.readings.clear();
        self.status = self.tr("Generator").to_string();
    }

    fn toggle_sweep(&mut self, ctx: &egui::Context) {
        if self.running {
            if let Some(w) = &self.worker {
                w.stop();
            }
            return;
        }
        let params = match self.current_params() {
            Ok(p) => p,
            Err(message) => {
                self.status = message;
                self.failed = true;
                return;
            }
        };
        if !self.ensure_worker(ctx) {
            return;
        }
        if let Some(w) = &self.worker {
            w.start(params);
        }
        self.gen_on = false;
        self.applied = Some(params);
        self.remember(params);
        self.running = true;
        self.failed = false;
        self.status = self.tr("Sweeping").to_string();
    }

    /// Parameters typed into the controls, checked.
    fn current_params(&self) -> Result<SweepParams, String> {
        SweepParams::new(
            (self.start_mhz * 1e6).round() as u64,
            (self.stop_mhz * 1e6).round() as u64,
            self.points,
        )
        .map_err(|e| e.to_string())
    }

    fn remember(&mut self, params: SweepParams) {
        self.settings.start_hz = params.start_hz;
        self.settings.stop_hz = params.stop_hz;
        self.settings.points = params.points;
        self.save_settings();
    }

    /// While sweeping, a change in the frequency range or point count takes
    /// effect at the next sweep.
    fn apply_changed_params(&mut self) {
        if !self.running {
            return;
        }
        match self.current_params() {
            Ok(params) if Some(params) != self.applied => {
                if let Some(w) = &self.worker {
                    w.start(params);
                }
                self.applied = Some(params);
                self.remember(params);
                self.failed = false;
                self.status = self.tr("Sweeping").to_string();
            }
            Ok(_) => {}
            Err(message) => {
                self.status = message;
                self.failed = true;
            }
        }
    }

    fn poll_worker(&mut self) {
        let Some(worker) = &self.worker else { return };
        let mut failure = None;
        let mut stopped = false;
        while let Ok(event) = worker.events.try_recv() {
            match event {
                Event::Progress {
                    params,
                    offset,
                    samples,
                } => {
                    // Points of an older range may still arrive after a change.
                    if Some(params) != self.applied {
                        continue;
                    }
                    let trace = match &mut self.trace {
                        Some(t) if t.params == params => t,
                        slot => {
                            self.scale = None;
                            self.sweep_secs = None;
                            self.last_sweep_done = None;
                            slot.insert(Trace::new(params))
                        }
                    };
                    trace.apply(offset, &samples, self.settings.averaging);
                }
                Event::Sweep(_) => {
                    let now = std::time::Instant::now();
                    if let Some(previous) = self.last_sweep_done {
                        self.sweep_secs = Some((now - previous).as_secs_f32());
                    }
                    self.last_sweep_done = Some(now);
                }
                Event::Reading(v) => {
                    self.readings.push_back(v);
                    while self.readings.len() > 300 {
                        self.readings.pop_front();
                    }
                }
                Event::Failed(reason) => failure = Some(reason),
                Event::Stopped => stopped = true,
            }
        }
        if let Some(reason) = failure {
            self.running = false;
            self.gen_on = false;
            self.failed = true;
            self.worker = None;
            self.worker_source = None;
            self.status = format!(
                "{} {}",
                self.translator
                    .tr_args("Sweep failed: {reason}", &[("reason", &reason)]),
                self.tr("If the board stays mute, unplug and replug the USB cable.")
            );
        } else if stopped {
            self.running = false;
            self.gen_on = false;
            self.status = self.tr("Stopped").to_string();
        }
    }

    fn port_label(&self, path: &str) -> String {
        if path == SIMULATOR {
            return self.tr("Simulator").to_string();
        }
        match self.ports.iter().find(|p| p.path == path) {
            Some(p) => {
                let product = p.product.as_deref().unwrap_or("");
                let mark = if p.is_board_candidate() { " *" } else { "" };
                format!("{path} {product}{mark}")
            }
            None => path.to_string(),
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let mut settings_changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(self.tr("Port"));
            let current = self.settings.port.clone().map_or_else(
                || self.tr("No port selected").to_string(),
                |p| self.port_label(&p),
            );
            egui::ComboBox::from_id_salt("port")
                .selected_text(current)
                .width(240.0)
                .show_ui(ui, |ui| {
                    let mut choices: Vec<String> =
                        self.ports.iter().map(|p| p.path.clone()).collect();
                    choices.push(SIMULATOR.to_string());
                    for path in choices {
                        let label = self.port_label(&path);
                        let selected = self.settings.port.as_deref() == Some(path.as_str());
                        if ui.selectable_label(selected, label).clicked() {
                            self.settings.port = Some(path);
                            settings_changed = true;
                        }
                    }
                });
            if ui.button(self.tr("Refresh ports")).clicked() {
                self.ports = ports::list_ports();
                if let Some(p) = ports::suggest(&self.ports) {
                    ui.label(self.tr("Auto-detected"));
                    if self.settings.port.is_none() {
                        self.settings.port = Some(p.path.clone());
                        settings_changed = true;
                    }
                }
            }
            ui.separator();
            ui.label(self.tr("Start (MHz)"));
            ui.add(
                egui::DragValue::new(&mut self.start_mhz)
                    .range(35.0..=4399.0)
                    .speed(5.0),
            );
            ui.label(self.tr("Stop (MHz)"));
            ui.add(
                egui::DragValue::new(&mut self.stop_mhz)
                    .range(36.0..=4400.0)
                    .speed(5.0),
            );
            ui.label(self.tr("Points"));
            // The board's step field limits how few points a wide range can have.
            let min_points = SweepParams::min_points(
                (self.start_mhz * 1e6).round() as u64,
                (self.stop_mhz * 1e6).round() as u64,
            );
            self.points = self.points.clamp(min_points, 2001.max(min_points));
            ui.add(egui::DragValue::new(&mut self.points).range(min_points..=2001));
            ui.label(self.tr("Averaging"));
            for n in [1u8, 4, 16] {
                let text = if n == 1 {
                    self.tr("Off").to_string()
                } else {
                    n.to_string()
                };
                if ui
                    .selectable_label(self.settings.averaging == n, text)
                    .clicked()
                {
                    self.settings.averaging = n;
                    settings_changed = true;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            let label = if self.running {
                self.tr("Stop")
            } else {
                self.tr("Sweep")
            };
            if ui.button(label).clicked() {
                self.toggle_sweep(ui.ctx());
            }
            ui.separator();
            ui.label(self.tr("Presets"));
            for preset in &PRESETS {
                let active =
                    self.settings.style == preset.style && self.settings.theme == preset.theme;
                if ui.selectable_label(active, self.tr(preset.label)).clicked() {
                    self.settings.style = preset.style;
                    self.settings.theme = preset.theme;
                    self.theme = preset.theme.theme();
                    apply_theme(ui.ctx(), &self.theme);
                    settings_changed = true;
                }
            }
            ui.separator();
            if ui
                .selectable_label(self.settings.show_max_hold, self.tr("Max hold"))
                .clicked()
            {
                self.settings.show_max_hold = !self.settings.show_max_hold;
                settings_changed = true;
            }
            if ui
                .selectable_label(self.settings.show_min_hold, self.tr("Min hold"))
                .clicked()
            {
                self.settings.show_min_hold = !self.settings.show_min_hold;
                settings_changed = true;
            }
            if ui.button(self.tr("Clear hold")).clicked()
                && let Some(t) = &mut self.trace
            {
                t.clear_hold();
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(self.tr("Style"));
            for style in ViewStyle::ALL {
                if ui
                    .selectable_label(self.settings.style == style, self.tr(style.label()))
                    .clicked()
                {
                    self.settings.style = style;
                    settings_changed = true;
                }
            }
            ui.separator();
            ui.label(self.tr("Theme"));
            for id in ThemeId::ALL {
                if ui
                    .selectable_label(self.settings.theme == id, self.tr(id.label()))
                    .clicked()
                {
                    self.settings.theme = id;
                    self.theme = id.theme();
                    apply_theme(ui.ctx(), &self.theme);
                    settings_changed = true;
                }
            }
            ui.separator();
            ui.label(self.tr("Language"));
            for info in &LANGUAGES {
                if ui
                    .selectable_label(self.settings.language == Some(info.id), info.native_name)
                    .clicked()
                {
                    self.settings.language = Some(info.id);
                    self.translator = Translator::new(info.id);
                    settings_changed = true;
                }
            }
            ui.separator();
            // Back to the front panel (the SYSTEM menu that switches modes is not shown here).
            if ui.button(self.tr("Front panel")).clicked() {
                self.settings.compact_controls = false;
                settings_changed = true;
            }
        });
        if settings_changed {
            self.save_settings();
        }
    }
}

impl eframe::App for PakespecApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_worker();
        self.apply_changed_params();
        let theme_panel = rgb(self.theme.panel);
        if self.settings.compact_controls {
            egui::Panel::top("controls")
                .frame(egui::Frame::new().fill(theme_panel).inner_margin(8.0))
                .show(ui, |ui| self.controls(ui));
        }
        egui::Panel::bottom("status")
            .frame(egui::Frame::new().fill(theme_panel).inner_margin(6.0))
            .show(ui, |ui| {
                let color = if self.failed {
                    rgb(self.theme.marker)
                } else {
                    rgb(self.theme.text_dim)
                };
                ui.colored_label(color, &self.status);
            });
        if !self.settings.compact_controls {
            egui::Panel::bottom("front-panel")
                .frame(egui::Frame::new().fill(theme_panel).inner_margin(8.0))
                .show(ui, |ui| self.hard_key_panel(ui));
            egui::Panel::right("soft-keys")
                .exact_size(166.0)
                .frame(egui::Frame::new().fill(theme_panel).inner_margin(8.0))
                .show(ui, |ui| self.soft_key_panel(ui));
        }
        let marker = self.marker_index();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(rgb(self.theme.background)))
            .show(ui, |ui| match &self.trace {
                _ if self.gen_on => generator::show(
                    ui,
                    self.gen_mhz,
                    &self.readings,
                    &self.theme,
                    &self.translator,
                ),
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(
                            rgb(self.theme.text_dim),
                            self.tr("Waiting for the first sweep"),
                        );
                    });
                }
                Some(trace) if trace.levels().is_empty() => {}
                Some(raw) => {
                    let shown = self
                        .calibration
                        .as_ref()
                        .map(|c| raw.mapped(|v| c.raw_to_dbm(f64::from(v)) as f32));
                    let trace = shown.as_ref().unwrap_or(raw);
                    // Fit the scale to everything that is drawn.
                    let mut lo_hi = trace.range().unwrap_or((0.0, 1.0));
                    if self.settings.show_max_hold {
                        lo_hi.1 = trace.max_hold().iter().copied().fold(lo_hi.1, f32::max);
                    }
                    if self.settings.show_min_hold {
                        lo_hi.0 = trace.min_hold().iter().copied().fold(lo_hi.0, f32::min);
                    }
                    let scale = if self.settings.scale_auto {
                        match self.scale {
                            Some(s) => s.follow(lo_hi.0, lo_hi.1),
                            None => Scale::fit(lo_hi.0, lo_hi.1, ROWS),
                        }
                    } else {
                        Scale {
                            top: self.settings.scale_top,
                            per_div: self.settings.scale_div,
                            rows: ROWS,
                        }
                    };
                    self.scale = Some(scale);
                    let cx = ViewContext {
                        trace,
                        raw,
                        calibration: self.calibration.as_ref(),
                        scale,
                        theme: &self.theme,
                        tr: &self.translator,
                        show_max_hold: self.settings.show_max_hold,
                        show_min_hold: self.settings.show_min_hold,
                        averaging: self.settings.averaging,
                        sweep_secs: self.sweep_secs,
                        marker,
                    };
                    match self.settings.style {
                        ViewStyle::Bench => bench::show(ui, &cx),
                        ViewStyle::Classic => classic::show(ui, &cx),
                        ViewStyle::Instrument => instrument::show(ui, &cx),
                        ViewStyle::Modern => modern::show(ui, &cx),
                    }
                }
            });
    }
}

pub(crate) fn rgb(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

pub(crate) fn apply_theme(ctx: &egui::Context, theme: &Theme) {
    let mut visuals = match theme.id {
        ThemeId::Dark | ThemeId::Phosphor => egui::Visuals::dark(),
        ThemeId::Light => egui::Visuals::light(),
    };
    visuals.panel_fill = rgb(theme.panel);
    visuals.window_fill = rgb(theme.panel);
    visuals.extreme_bg_color = rgb(theme.background);
    visuals.override_text_color = Some(rgb(theme.text));
    visuals.selection.bg_fill = rgb(theme.accent);
    ctx.set_visuals(visuals);
}

/// Language names are shown in their own language; this keeps the import used
/// when more languages are added.
#[allow(dead_code)]
fn default_language() -> Language {
    Language::from_environment()
}
