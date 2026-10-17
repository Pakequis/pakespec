//! Logic of the front panel: menus, value entry and the frequency range, kept
//! apart from drawing so it can be tested.

/// Lowest and highest frequency of the board, in MHz.
pub const MIN_MHZ: f64 = 35.0;
pub const MAX_MHZ: f64 = 4400.0;
/// Narrowest span that can be set, in MHz.
pub const MIN_SPAN_MHZ: f64 = 0.1;

/// The hard keys of the panel; each opens a menu of soft keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Menu {
    Freq,
    Span,
    Ampt,
    Sweep,
    Trace,
    Marker,
    Peak,
    Gen,
    System,
}

impl Menu {
    pub const ALL: [Menu; 9] = [
        Menu::Freq,
        Menu::Span,
        Menu::Ampt,
        Menu::Sweep,
        Menu::Trace,
        Menu::Marker,
        Menu::Peak,
        Menu::Gen,
        Menu::System,
    ];

    /// English source string of the key's label (translated by the UI).
    pub fn label(self) -> &'static str {
        match self {
            Self::Freq => "FREQ",
            Self::Span => "SPAN",
            Self::Ampt => "AMPT",
            Self::Sweep => "SWEEP",
            Self::Trace => "TRACE",
            Self::Marker => "MARKER",
            Self::Peak => "PEAK",
            Self::Gen => "GEN",
            Self::System => "SYSTEM",
        }
    }
}

/// A number the user can type or turn the knob on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Center,
    Span,
    Start,
    Stop,
    Points,
    RefLevel,
    ScaleDiv,
    /// Frequency of the generator.
    GenFreq,
}

impl Field {
    pub fn is_frequency(self) -> bool {
        matches!(
            self,
            Self::Center | Self::Span | Self::Start | Self::Stop | Self::GenFreq
        )
    }

    /// English source string of the field's name (translated by the UI).
    pub fn label(self) -> &'static str {
        match self {
            Self::Center => "Center",
            Self::Span => "Span",
            Self::Start => "Start freq",
            Self::Stop => "Stop freq",
            Self::Points => "Points",
            Self::RefLevel => "Ref level",
            Self::ScaleDiv => "Scale/div",
            Self::GenFreq => "Generator freq",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    GHz,
    MHz,
    KHz,
    /// No unit: the Enter key.
    None,
}

/// Frequency range of the sweep, in MHz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub start: f64,
    pub stop: f64,
}

impl Range {
    pub fn center(&self) -> f64 {
        (self.start + self.stop) / 2.0
    }

    pub fn span(&self) -> f64 {
        self.stop - self.start
    }

    /// Centers the range on `center`. The center is kept exactly as given: when
    /// the span does not fit around it inside the board's limits, the span shrinks.
    pub fn with_center(self, center: f64) -> Range {
        let center = center.clamp(MIN_MHZ + MIN_SPAN_MHZ / 2.0, MAX_MHZ - MIN_SPAN_MHZ / 2.0);
        let half = (self.span() / 2.0)
            .min(center - MIN_MHZ)
            .min(MAX_MHZ - center);
        Range {
            start: center - half,
            stop: center + half,
        }
    }

    /// Changes the span around the same center, staying inside the board's limits.
    pub fn with_span(self, span: f64) -> Range {
        let span = span.clamp(MIN_SPAN_MHZ, MAX_MHZ - MIN_MHZ);
        let c = self.center();
        let start = (c - span / 2.0).clamp(MIN_MHZ, MAX_MHZ - span);
        Range {
            start,
            stop: start + span,
        }
    }

    pub fn with_start(self, start: f64) -> Range {
        let start = start.clamp(MIN_MHZ, self.stop - MIN_SPAN_MHZ);
        Range {
            start,
            stop: self.stop,
        }
    }

    pub fn with_stop(self, stop: f64) -> Range {
        let stop = stop.clamp(self.start + MIN_SPAN_MHZ, MAX_MHZ);
        Range {
            start: self.start,
            stop,
        }
    }
}

/// Digits being typed for a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub field: Field,
    pub text: String,
}

impl Entry {
    pub fn new(field: Field) -> Entry {
        Entry {
            field,
            text: String::new(),
        }
    }

    /// Adds a character if it keeps the text a number (digits, one dot, a leading minus).
    pub fn push(&mut self, c: char) {
        let ok = match c {
            '0'..='9' => true,
            '.' => !self.text.contains('.'),
            '-' => self.text.is_empty(),
            _ => false,
        };
        if ok && self.text.len() < 12 {
            self.text.push(c);
        }
    }

    pub fn backspace(&mut self) {
        self.text.pop();
    }

    /// The typed number converted for the field: MHz for frequencies, the
    /// plain number otherwise. `None` when nothing valid was typed.
    pub fn value(&self, unit: Unit) -> Option<f64> {
        let n: f64 = self.text.parse().ok()?;
        if self.field.is_frequency() {
            Some(match unit {
                Unit::GHz => n * 1000.0,
                Unit::KHz => n / 1000.0,
                Unit::MHz | Unit::None => n,
            })
        } else {
            Some(n)
        }
    }
}

/// Rounds a reference level or scale per division to thousandths of a dB, so
/// that values kept as `f32` (0.1 is stored as 0.100000001...) do not show noise.
pub fn round_level(v: f64) -> f32 {
    ((v * 1000.0).round() / 1000.0) as f32
}

/// Text of a reference level or scale per division: no trailing noise digits.
pub fn format_level(v: f64) -> String {
    format!("{}", (v * 1000.0).round() / 1000.0)
}

/// How much one click of the knob changes `field`.
pub fn knob_step(field: Field, range: Range, scale_div: f32) -> f64 {
    match field {
        Field::Center | Field::Start | Field::Stop => (range.span() / 100.0).max(0.01),
        Field::Span => range.span() * 0.05,
        Field::Points => 10.0,
        Field::RefLevel | Field::ScaleDiv => f64::from(scale_div),
        Field::GenFreq => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Range = Range {
        start: 88.0,
        stop: 108.0,
    };

    #[test]
    fn levels_are_shown_without_float_noise() {
        assert_eq!(format_level(f64::from(0.1_f32)), "0.1");
        assert_eq!(format_level(f64::from(round_level(0.30000001192))), "0.3");
        assert_eq!(format_level(-57.0), "-57");
        assert_eq!(format_level(2.5), "2.5");
    }

    #[test]
    fn center_and_span() {
        assert_eq!(R.center(), 98.0);
        assert_eq!(R.span(), 20.0);
        let c = R.with_center(200.0);
        assert_eq!((c.start, c.stop), (190.0, 210.0));
        // A wide span that does not fit around the new center shrinks; the center is kept.
        let wide = Range {
            start: 4000.0,
            stop: 4400.0,
        }
        .with_center(150.0);
        assert_eq!((wide.start, wide.stop), (35.0, 265.0));
        assert_eq!(wide.center(), 150.0);
        let s = R.with_span(40.0);
        assert_eq!((s.start, s.stop), (78.0, 118.0));
    }

    #[test]
    fn ranges_stay_inside_the_board_limits() {
        let low = R.with_center(10.0);
        assert!(low.start >= MIN_MHZ && low.span() >= MIN_SPAN_MHZ - 1e-9);
        let high = R.with_center(9000.0);
        assert!(high.stop <= MAX_MHZ && high.span() >= MIN_SPAN_MHZ - 1e-9);
        let wide = R.with_span(99_999.0);
        assert_eq!((wide.start, wide.stop), (MIN_MHZ, MAX_MHZ));
        assert!(R.with_span(0.0).span() >= MIN_SPAN_MHZ - 1e-9);
        assert!(R.with_start(500.0).start < R.stop);
        assert!(R.with_stop(10.0).stop > R.start);
    }

    #[test]
    fn typing_and_units() {
        let mut e = Entry::new(Field::Center);
        for c in "9x8.5.2".chars() {
            e.push(c);
        }
        assert_eq!(e.text, "98.52");
        assert_eq!(e.value(Unit::MHz), Some(98.52));
        let mut g = Entry::new(Field::Stop);
        g.push('2');
        assert_eq!(g.value(Unit::GHz), Some(2000.0));
        let mut k = Entry::new(Field::Span);
        k.push('5');
        k.push('0');
        k.push('0');
        assert_eq!(k.value(Unit::KHz), Some(0.5));
        let mut p = Entry::new(Field::Points);
        p.push('2');
        p.push('0');
        p.push('1');
        assert_eq!(p.value(Unit::GHz), Some(201.0)); // units only apply to frequencies
        assert_eq!(Entry::new(Field::Span).value(Unit::MHz), None);
        let mut m = Entry::new(Field::RefLevel);
        m.push('-');
        m.push('3');
        m.push('-');
        assert_eq!(m.text, "-3");
        m.backspace();
        m.backspace();
        assert_eq!(m.text, "");
    }

    #[test]
    fn knob_steps_scale_with_the_span() {
        assert_eq!(knob_step(Field::Center, R, 10.0), 0.2);
        assert_eq!(knob_step(Field::Span, R, 10.0), 1.0);
        assert_eq!(knob_step(Field::Points, R, 10.0), 10.0);
    }
}
