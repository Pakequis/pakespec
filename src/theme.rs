//! Colors of the interface. Views read their colors from the active theme and
//! never hard-code them.

use serde::{Deserialize, Serialize};

pub type Rgb = [u8; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeId {
    Dark,
    Light,
    /// Green on black, like a phosphor screen.
    Phosphor,
}

impl ThemeId {
    pub const ALL: [ThemeId; 3] = [ThemeId::Dark, ThemeId::Light, ThemeId::Phosphor];

    /// English source string of the theme's display name (translated by the UI).
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Phosphor => "Phosphor",
        }
    }

    pub fn theme(self) -> Theme {
        match self {
            Self::Dark => Theme {
                id: self,
                background: [17, 17, 17],
                panel: [27, 27, 27],
                text: [207, 207, 207],
                text_dim: [154, 154, 154],
                grid: [47, 47, 47],
                axis: [85, 85, 85],
                trace: [255, 210, 74],
                trace_secondary: [120, 170, 255],
                accent: [74, 163, 255],
                marker: [255, 120, 90],
            },
            Self::Phosphor => Theme {
                id: self,
                background: [5, 8, 5],
                panel: [12, 18, 12],
                text: [143, 227, 155],
                text_dim: [76, 154, 92],
                grid: [31, 58, 36],
                axis: [47, 107, 58],
                trace: [57, 255, 106],
                trace_secondary: [120, 200, 140],
                accent: [57, 255, 106],
                marker: [255, 138, 106],
            },
            Self::Light => Theme {
                id: self,
                background: [255, 255, 255],
                panel: [240, 242, 246],
                text: [29, 35, 48],
                text_dim: [93, 102, 121],
                grid: [225, 229, 236],
                axis: [150, 158, 174],
                trace: [31, 95, 214],
                trace_secondary: [200, 90, 30],
                accent: [31, 95, 214],
                marker: [200, 50, 40],
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub id: ThemeId,
    pub background: Rgb,
    pub panel: Rgb,
    pub text: Rgb,
    pub text_dim: Rgb,
    pub grid: Rgb,
    pub axis: Rgb,
    pub trace: Rgb,
    pub trace_secondary: Rgb,
    pub accent: Rgb,
    pub marker: Rgb,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Rgb) -> f64 {
        let f = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
    }

    fn contrast(a: Rgb, b: Rgb) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn text_and_trace_are_readable_on_the_background() {
        for id in ThemeId::ALL {
            let t = id.theme();
            assert!(contrast(t.text, t.background) >= 7.0, "{id:?} text");
            assert!(contrast(t.trace, t.background) >= 4.5, "{id:?} trace");
            assert!(contrast(t.text_dim, t.background) >= 3.0, "{id:?} dim text");
        }
    }
}
