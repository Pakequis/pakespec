//! Front panel: hard keys, soft keys, number keypad and a knob, like a bench analyzer.

use eframe::egui::{self, Color32, RichText, Sense, Vec2};

use super::{PakespecApp, SIMULATOR, rgb};
use crate::i18n::{LANGUAGES, Translator};
use crate::panel::{Entry, Field, Menu, Range, Unit, knob_step};
use crate::protocol::SweepParams;
use crate::settings::{PRESETS, ViewStyle};
use crate::theme::ThemeId;

/// State of the front panel.
pub struct PanelState {
    pub menu: Menu,
    /// Digits being typed, if any.
    pub entry: Option<Entry>,
    /// The field the knob and the arrow keys change.
    pub active: Option<Field>,
    /// Marker frequency in MHz; `None` means no marker.
    pub marker_mhz: Option<f64>,
    knob_angle: f32,
    knob_pending: f32,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            menu: Menu::Freq,
            entry: None,
            active: None,
            marker_mhz: None,
            knob_angle: 0.0,
            knob_pending: 0.0,
        }
    }
}

/// What pressing a soft key does.
#[derive(Clone, Copy)]
enum Act {
    Field(Field),
    FullRange,
    ZoomIn,
    ZoomOut,
    AutoScale,
    Averaging,
    Run,
    MaxHold,
    MinHold,
    ClearHold,
    MarkerPeak,
    MarkerNext,
    MarkerCenter,
    MarkerOff,
    CyclePort,
    RefreshPorts,
    CycleStyle,
    CycleTheme,
    CyclePreset,
    CycleLanguage,
    ToggleCompact,
    GenToggle,
}

struct Soft {
    label: String,
    value: String,
    lit: bool,
    act: Act,
}

impl PakespecApp {
    fn range(&self) -> Range {
        Range {
            start: self.start_mhz,
            stop: self.stop_mhz,
        }
    }

    fn set_range(&mut self, r: Range) {
        self.start_mhz = r.start;
        self.stop_mhz = r.stop;
        // The board's step field limits how few points a wide range can have.
        let min = SweepParams::min_points(
            (r.start * 1e6).round() as u64,
            (r.stop * 1e6).round() as u64,
        );
        self.points = self.points.max(min);
    }

    fn current_scale(&self) -> Option<crate::scale::Scale> {
        self.scale
    }

    fn field_value(&self, f: Field) -> f64 {
        let r = self.range();
        match f {
            Field::Center => r.center(),
            Field::Span => r.span(),
            Field::Start => r.start,
            Field::Stop => r.stop,
            Field::Points => f64::from(self.points),
            Field::GenFreq => self.gen_mhz,
            Field::RefLevel => f64::from(
                self.current_scale()
                    .map_or(self.settings.scale_top, |s| s.top),
            ),
            Field::ScaleDiv => f64::from(
                self.current_scale()
                    .map_or(self.settings.scale_div, |s| s.per_div),
            ),
        }
    }

    fn set_field(&mut self, f: Field, v: f64) {
        let r = self.range();
        match f {
            Field::Center => self.set_range(r.with_center(v)),
            Field::Span => self.set_range(r.with_span(v)),
            Field::Start => self.set_range(r.with_start(v)),
            Field::Stop => self.set_range(r.with_stop(v)),
            Field::GenFreq => {
                self.gen_mhz = v.clamp(crate::panel::MIN_MHZ, crate::panel::MAX_MHZ);
                // Changing the frequency while the generator runs takes effect at once.
                if self.gen_on
                    && let Some(w) = &self.worker
                {
                    w.generator_on((self.gen_mhz * 1e6).round() as u64);
                    self.readings.clear();
                }
            }
            Field::Points => {
                let min = SweepParams::min_points(
                    (r.start * 1e6).round() as u64,
                    (r.stop * 1e6).round() as u64,
                );
                self.points = (v.round() as i64).clamp(i64::from(min), 2001) as u16;
            }
            Field::RefLevel | Field::ScaleDiv => {
                // Typing a scale value switches to the manual scale.
                if self.settings.scale_auto {
                    if let Some(s) = self.current_scale() {
                        self.settings.scale_top = s.top;
                        self.settings.scale_div = s.per_div;
                    }
                    self.settings.scale_auto = false;
                }
                if f == Field::RefLevel {
                    self.settings.scale_top = v as f32;
                } else {
                    self.settings.scale_div = (v as f32).max(0.1);
                }
                self.save_settings();
            }
        }
    }

    fn format_field(&self, f: Field) -> String {
        let v = self.field_value(f);
        match f {
            Field::Center | Field::Span | Field::Start | Field::Stop | Field::GenFreq => {
                format!("{v:.3} MHz")
            }
            Field::Points => format!("{v:.0}"),
            Field::RefLevel | Field::ScaleDiv => format!("{v}"),
        }
    }

    pub(super) fn marker_index(&self) -> Option<usize> {
        let m = self.panel.marker_mhz?;
        let trace = self.trace.as_ref()?;
        trace
            .freqs()
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - m).abs().total_cmp(&(b.1 - m).abs()))
            .map(|(i, _)| i)
    }

    fn soft_keys(&self, menu: Menu) -> Vec<Soft> {
        let tr = |s: &str| self.translator.tr(s).to_string();
        let field = |f: Field| Soft {
            label: tr(f.label()),
            value: self.format_field(f),
            lit: self.panel.active == Some(f),
            act: Act::Field(f),
        };
        let plain = |label: &str, act: Act| Soft {
            label: tr(label),
            value: String::new(),
            lit: false,
            act,
        };
        let toggle = |label: &str, on: bool, act: Act| Soft {
            label: tr(label),
            value: if on { tr("On") } else { tr("Off") },
            lit: on,
            act,
        };
        match menu {
            Menu::Freq => vec![
                field(Field::Center),
                field(Field::Start),
                field(Field::Stop),
                plain("Full range", Act::FullRange),
                plain("Marker to center", Act::MarkerCenter),
            ],
            Menu::Span => vec![
                field(Field::Span),
                plain("Zoom in", Act::ZoomIn),
                plain("Zoom out", Act::ZoomOut),
                plain("Full range", Act::FullRange),
            ],
            Menu::Ampt => vec![
                toggle("Auto scale", self.settings.scale_auto, Act::AutoScale),
                field(Field::RefLevel),
                field(Field::ScaleDiv),
            ],
            Menu::Sweep => vec![
                Soft {
                    label: tr(if self.running { "Stop" } else { "Run" }),
                    value: String::new(),
                    lit: self.running,
                    act: Act::Run,
                },
                field(Field::Points),
                Soft {
                    label: tr("Averaging"),
                    value: if self.settings.averaging <= 1 {
                        tr("Off")
                    } else {
                        self.settings.averaging.to_string()
                    },
                    lit: self.settings.averaging > 1,
                    act: Act::Averaging,
                },
            ],
            Menu::Trace => vec![
                toggle("Max hold", self.settings.show_max_hold, Act::MaxHold),
                toggle("Min hold", self.settings.show_min_hold, Act::MinHold),
                plain("Clear hold", Act::ClearHold),
            ],
            Menu::Marker => vec![
                toggle("Marker", self.panel.marker_mhz.is_some(), Act::MarkerPeak),
                plain("Marker to center", Act::MarkerCenter),
                plain("Marker off", Act::MarkerOff),
            ],
            Menu::Peak => vec![
                plain("Peak search", Act::MarkerPeak),
                plain("Next peak", Act::MarkerNext),
                plain("Marker to center", Act::MarkerCenter),
            ],
            Menu::Gen => vec![
                field(Field::GenFreq),
                toggle("Output", self.gen_on, Act::GenToggle),
                Soft {
                    label: tr("Reading"),
                    value: self
                        .readings
                        .back()
                        .map_or_else(|| "-".to_string(), |v| v.to_string()),
                    lit: false,
                    act: Act::GenToggle,
                },
            ],
            Menu::System => vec![
                Soft {
                    label: tr("Port"),
                    value: self.settings.port.as_deref().map_or_else(
                        || tr("No port selected"),
                        |p| {
                            if p == SIMULATOR {
                                tr("Simulator")
                            } else {
                                p.to_string()
                            }
                        },
                    ),
                    lit: false,
                    act: Act::CyclePort,
                },
                plain("Refresh ports", Act::RefreshPorts),
                Soft {
                    label: tr("Style"),
                    value: tr(self.settings.style.label()),
                    lit: false,
                    act: Act::CycleStyle,
                },
                Soft {
                    label: tr("Theme"),
                    value: tr(self.settings.theme.label()),
                    lit: false,
                    act: Act::CycleTheme,
                },
                plain("Preset", Act::CyclePreset),
                Soft {
                    label: tr("Language"),
                    value: LANGUAGES
                        .iter()
                        .find(|l| Some(l.id) == self.settings.language)
                        .map_or_else(|| tr("Auto-detected"), |l| l.native_name.to_string()),
                    lit: false,
                    act: Act::CycleLanguage,
                },
                toggle(
                    "Compact controls",
                    self.settings.compact_controls,
                    Act::ToggleCompact,
                ),
            ],
        }
    }

    fn do_act(&mut self, act: Act, ctx: &egui::Context) {
        let mut save = false;
        match act {
            Act::Field(f) => {
                self.panel.active = Some(f);
                self.panel.entry = Some(Entry::new(f));
            }
            Act::FullRange => self.set_range(Range {
                start: crate::panel::MIN_MHZ,
                stop: crate::panel::MAX_MHZ,
            }),
            Act::ZoomIn => {
                let r = self.range();
                self.set_range(r.with_span(r.span() / 2.0));
            }
            Act::ZoomOut => {
                let r = self.range();
                self.set_range(r.with_span(r.span() * 2.0));
            }
            Act::AutoScale => {
                self.settings.scale_auto = !self.settings.scale_auto;
                self.scale = None;
                save = true;
            }
            Act::Averaging => {
                self.settings.averaging = match self.settings.averaging {
                    1 => 4,
                    4 => 16,
                    _ => 1,
                };
                save = true;
            }
            Act::Run => self.toggle_sweep(ctx),
            Act::MaxHold => {
                self.settings.show_max_hold = !self.settings.show_max_hold;
                save = true;
            }
            Act::MinHold => {
                self.settings.show_min_hold = !self.settings.show_min_hold;
                save = true;
            }
            Act::ClearHold => {
                if let Some(t) = &mut self.trace {
                    t.clear_hold();
                }
            }
            Act::MarkerPeak => {
                // Pressing it with a marker already set removes the marker.
                if matches!(self.panel.menu, Menu::Marker) && self.panel.marker_mhz.is_some() {
                    self.panel.marker_mhz = None;
                } else if let Some(t) = &self.trace
                    && let Some(&p) = t.peaks(1, 5).first()
                {
                    self.panel.marker_mhz = Some(t.freqs()[p]);
                }
            }
            Act::MarkerNext => {
                if let Some(t) = &self.trace {
                    let peaks = t.peaks(12, 5);
                    let current = self.marker_index();
                    let next = match current
                        .and_then(|c| peaks.iter().position(|&p| p.abs_diff(c) <= 5))
                    {
                        Some(pos) => peaks.get(pos + 1).or_else(|| peaks.first()),
                        None => peaks.first(),
                    };
                    if let Some(&p) = next {
                        self.panel.marker_mhz = Some(t.freqs()[p]);
                    }
                }
            }
            Act::MarkerCenter => {
                if let Some(m) = self.panel.marker_mhz {
                    let r = self.range();
                    self.set_range(r.with_center(m));
                }
            }
            Act::MarkerOff => self.panel.marker_mhz = None,
            Act::CyclePort => {
                let mut choices: Vec<String> = self.ports.iter().map(|p| p.path.clone()).collect();
                choices.push(SIMULATOR.to_string());
                let current = choices
                    .iter()
                    .position(|c| Some(c.as_str()) == self.settings.port.as_deref());
                let next = current.map_or(0, |i| (i + 1) % choices.len());
                self.settings.port = Some(choices[next].clone());
                save = true;
            }
            Act::RefreshPorts => {
                self.ports = crate::ports::list_ports();
                if self.settings.port.is_none() {
                    self.settings.port = crate::ports::suggest(&self.ports).map(|p| p.path.clone());
                    save = true;
                }
            }
            Act::CycleStyle => {
                let all = ViewStyle::ALL;
                let i = all
                    .iter()
                    .position(|s| *s == self.settings.style)
                    .unwrap_or(0);
                self.settings.style = all[(i + 1) % all.len()];
                save = true;
            }
            Act::CycleTheme => {
                let all = ThemeId::ALL;
                let i = all
                    .iter()
                    .position(|t| *t == self.settings.theme)
                    .unwrap_or(0);
                self.settings.theme = all[(i + 1) % all.len()];
                self.theme = self.settings.theme.theme();
                super::apply_theme(ctx, &self.theme);
                save = true;
            }
            Act::CyclePreset => {
                let i = PRESETS
                    .iter()
                    .position(|p| p.style == self.settings.style && p.theme == self.settings.theme);
                let p = &PRESETS[i.map_or(0, |i| (i + 1) % PRESETS.len())];
                self.settings.style = p.style;
                self.settings.theme = p.theme;
                self.theme = p.theme.theme();
                super::apply_theme(ctx, &self.theme);
                save = true;
            }
            Act::CycleLanguage => {
                let i = LANGUAGES
                    .iter()
                    .position(|l| Some(l.id) == self.settings.language);
                let next = &LANGUAGES[i.map_or(0, |i| (i + 1) % LANGUAGES.len())];
                self.settings.language = Some(next.id);
                self.translator = Translator::new(next.id);
                save = true;
            }
            Act::GenToggle => self.toggle_generator(ctx),
            Act::ToggleCompact => {
                self.settings.compact_controls = !self.settings.compact_controls;
                save = true;
            }
        }
        if save {
            self.save_settings();
        }
    }

    /// Applies the typed digits with `unit` (Enter has no unit).
    fn commit_entry(&mut self, unit: Unit) {
        if let Some(entry) = self.panel.entry.take()
            && let Some(v) = entry.value(unit)
        {
            self.set_field(entry.field, v);
        }
    }

    /// Moves the active field by `clicks` knob steps.
    fn turn_knob(&mut self, clicks: i32) {
        let Some(f) = self.panel.active else { return };
        let step = knob_step(
            f,
            self.range(),
            self.settings
                .scale_div
                .max(self.current_scale().map_or(0.0, |s| s.per_div)),
        );
        let v = self.field_value(f) + step * f64::from(clicks);
        self.panel.entry = None;
        self.set_field(f, v);
    }

    fn keyboard_entry(&mut self, ctx: &egui::Context) {
        let Some(f) = self.panel.active else { return };
        let (text, enter, back, escape) = ctx.input(|i| {
            let text: String = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .collect();
            (
                text,
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Backspace),
                i.key_pressed(egui::Key::Escape),
            )
        });
        for c in text.chars().filter(|c| "0123456789.-".contains(*c)) {
            self.panel
                .entry
                .get_or_insert_with(|| Entry::new(f))
                .push(c);
        }
        if back && let Some(e) = &mut self.panel.entry {
            e.backspace();
        }
        if escape {
            self.panel.entry = None;
        }
        if enter {
            self.commit_entry(Unit::None);
        }
    }

    pub(super) fn soft_key_panel(&mut self, ui: &mut egui::Ui) {
        let keys = self.soft_keys(self.panel.menu);
        let mut pressed = None;
        ui.vertical(|ui| {
            ui.add_space(4.0);
            for (i, k) in keys.iter().enumerate() {
                let text = if k.value.is_empty() {
                    RichText::new(&k.label).size(13.0).strong()
                } else {
                    RichText::new(format!("{}\n{}", k.label, k.value)).size(12.5)
                };
                let (fill, color) = if k.lit {
                    (
                        rgb(self.theme.accent),
                        contrast_text(rgb(self.theme.accent)),
                    )
                } else {
                    (key_fill(&self.theme), rgb(self.theme.text))
                };
                let button = egui::Button::new(text.color(color))
                    .fill(fill)
                    .min_size(Vec2::new(150.0, 46.0));
                if ui.add(button).clicked() {
                    pressed = Some(i);
                }
                ui.add_space(4.0);
            }
        });
        if let Some(i) = pressed {
            let act = keys[i].act;
            let ctx = ui.ctx().clone();
            self.do_act(act, &ctx);
        }
    }

    pub(super) fn hard_key_panel(&mut self, ui: &mut egui::Ui) {
        self.keyboard_entry(ui.ctx());
        let accent = rgb(self.theme.accent);
        ui.horizontal(|ui| {
            // Hard keys.
            ui.vertical(|ui| {
                egui::Grid::new("hard-keys")
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        for (n, menu) in Menu::ALL.iter().enumerate() {
                            let active = self.panel.menu == *menu;
                            let (fill, color) = if active {
                                (accent, contrast_text(accent))
                            } else {
                                (key_fill(&self.theme), rgb(self.theme.text))
                            };
                            let label = self.translator.tr(menu.label()).to_string();
                            let button =
                                egui::Button::new(RichText::new(label).strong().color(color))
                                    .fill(fill)
                                    .min_size(Vec2::new(78.0, 34.0));
                            if ui.add(button).clicked() {
                                self.panel.menu = *menu;
                            }
                            if n % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
                ui.add_space(4.0);
                let run_label = if self.running {
                    self.tr("Stop")
                } else {
                    self.tr("Sweep")
                };
                let (fill, color) = if self.running {
                    (Color32::from_rgb(190, 60, 50), Color32::WHITE)
                } else {
                    (Color32::from_rgb(40, 140, 70), Color32::WHITE)
                };
                let run =
                    egui::Button::new(RichText::new(run_label).strong().size(15.0).color(color))
                        .fill(fill)
                        .min_size(Vec2::new(78.0 * 4.0 + 18.0, 34.0));
                if ui.add(run).clicked() {
                    let ctx = ui.ctx().clone();
                    self.toggle_sweep(&ctx);
                }
            });
            ui.separator();
            // Entry display and keypad.
            ui.vertical(|ui| {
                let shown = match (&self.panel.entry, self.panel.active) {
                    (Some(e), _) => format!("{}: {}_", self.translator.tr(e.field.label()), e.text),
                    (None, Some(f)) => format!(
                        "{}: {}",
                        self.translator.tr(f.label()),
                        self.format_field(f)
                    ),
                    (None, None) => self.tr("Enter a value").to_string(),
                };
                ui.add(egui::Label::new(
                    RichText::new(shown)
                        .monospace()
                        .size(14.0)
                        .color(rgb(self.theme.text)),
                ));
                self.keypad(ui);
            });
            ui.separator();
            // Knob and arrow keys.
            ui.vertical(|ui| {
                let clicks = self.knob(ui);
                if clicks != 0 {
                    self.turn_knob(clicks);
                }
                ui.horizontal(|ui| {
                    if ui.button("-").clicked() {
                        self.turn_knob(-1);
                    }
                    if ui.button("+").clicked() {
                        self.turn_knob(1);
                    }
                });
            });
        });
    }

    fn keypad(&mut self, ui: &mut egui::Ui) {
        let size = Vec2::new(40.0, 28.0);
        let fill = key_fill(&self.theme);
        let text = rgb(self.theme.text);
        let key = |label: &str| {
            egui::Button::new(RichText::new(label).color(text))
                .fill(fill)
                .min_size(size)
        };
        let mut typed: Option<char> = None;
        let mut unit: Option<Unit> = None;
        let mut back = false;
        let mut cancel = false;
        egui::Grid::new("keypad")
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for (row, units) in [
                    (['7', '8', '9'], Some(("GHz", Unit::GHz))),
                    (['4', '5', '6'], Some(("MHz", Unit::MHz))),
                    (['1', '2', '3'], Some(("kHz", Unit::KHz))),
                ] {
                    for c in row {
                        if ui.add(key(&c.to_string())).clicked() {
                            typed = Some(c);
                        }
                    }
                    if let Some((label, u)) = units
                        && ui.add(key(label)).clicked()
                    {
                        unit = Some(u);
                    }
                    ui.end_row();
                }
                for c in ['0', '.', '-'] {
                    if ui.add(key(&c.to_string())).clicked() {
                        typed = Some(c);
                    }
                }
                if ui.add(key("Del")).clicked() {
                    back = true;
                }
                ui.end_row();
                if ui.add(key(self.translator.tr("Cancel"))).clicked() {
                    cancel = true;
                }
                ui.label("");
                ui.label("");
                if ui.add(key(self.translator.tr("Enter"))).clicked() {
                    unit = Some(Unit::None);
                }
                ui.end_row();
            });
        if let (Some(c), Some(f)) = (typed, self.panel.active) {
            self.panel
                .entry
                .get_or_insert_with(|| Entry::new(f))
                .push(c);
        }
        if back && let Some(e) = &mut self.panel.entry {
            e.backspace();
        }
        if cancel {
            self.panel.entry = None;
        }
        if let Some(u) = unit {
            self.commit_entry(u);
        }
    }

    /// The knob: drag or use the mouse wheel over it. Returns how many clicks it turned.
    fn knob(&mut self, ui: &mut egui::Ui) -> i32 {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(84.0), Sense::click_and_drag());
        // Drag distance and wheel notches both count toward a click.
        let mut moved = 0.0;
        if response.dragged() {
            let d = response.drag_delta();
            moved += d.x - d.y;
        }
        if response.hovered() {
            let wheel = ui.input(|i| i.smooth_scroll_delta.y);
            moved += wheel * 0.5;
        }
        self.panel.knob_pending += moved;
        const PER_CLICK: f32 = 10.0;
        let clicks = (self.panel.knob_pending / PER_CLICK).trunc();
        self.panel.knob_pending -= clicks * PER_CLICK;
        self.panel.knob_angle += clicks * 0.35;

        let painter = ui.painter_at(rect);
        let c = rect.center();
        let r = 38.0;
        painter.circle(
            c,
            r,
            key_fill(&self.theme),
            egui::Stroke::new(2.0, rgb(self.theme.axis)),
        );
        painter.circle(
            c,
            r - 8.0,
            rgb(self.theme.panel),
            egui::Stroke::new(1.0, rgb(self.theme.grid)),
        );
        let a = self.panel.knob_angle;
        let tip = c + Vec2::new(a.sin(), -a.cos()) * (r - 6.0);
        let base = c + Vec2::new(a.sin(), -a.cos()) * (r - 20.0);
        painter.line_segment([base, tip], egui::Stroke::new(3.0, rgb(self.theme.accent)));
        response.on_hover_text(self.translator.tr("Scroll or drag to adjust"));
        clicks as i32
    }
}

/// Fill of an unlit key: a step lighter or darker than the panel.
fn key_fill(theme: &crate::theme::Theme) -> Color32 {
    let p = theme.panel;
    let light = theme.id == ThemeId::Light;
    let shift = |v: u8| {
        if light {
            v.saturating_sub(22)
        } else {
            v.saturating_add(26)
        }
    };
    Color32::from_rgb(shift(p[0]), shift(p[1]), shift(p[2]))
}

/// Black or white text, whichever reads better on `fill`.
fn contrast_text(fill: Color32) -> Color32 {
    let l = 0.299 * f32::from(fill.r()) + 0.587 * f32::from(fill.g()) + 0.114 * f32::from(fill.b());
    if l > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
