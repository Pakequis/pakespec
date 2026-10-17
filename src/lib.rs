//! Pakespec: open-source control software for LTDZ 35-4400M spectrum analyzer boards.

pub mod board;
pub mod calibration;
pub mod i18n;
pub mod panel;
pub mod ports;
pub mod protocol;
pub mod scale;
pub mod settings;
pub mod sim;
pub mod theme;
pub mod trace;
pub mod worker;

#[cfg(feature = "gui")]
pub mod ui;
