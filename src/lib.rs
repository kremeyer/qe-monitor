//! Parsing and rendering for Quantum ESPRESSO and Wannier90 output files.
//!
//! The binary in `main.rs` is a thin wrapper around this library: it validates the
//! file, detects the calculation type and drives the terminal. Everything else -
//! parsers, metrics and widgets - lives here so integration tests can reach it.

pub mod app;
pub mod ph;
pub mod pw;
pub mod ui;
pub mod wannier90;

pub const QE_MARKER: &str = "This program is part of the open-source Quantum ESPRESSO suite";
pub const WANN_MARKER: &str = "Welcome to the Maximally-Localized";
pub const WANN_RESUME_MARKER: &str = "Resuming Wannier90";

pub fn is_wannier90(content: &str) -> bool {
    content.contains(WANN_MARKER) || content.contains(WANN_RESUME_MARKER)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcType {
    Pw,
    Ph,
    Wannier90,
}

/// Identify which program produced `content`, or `None` if it is not recognised.
pub fn detect_calc_type(content: &str) -> Option<CalcType> {
    if content.contains("Program PWSCF v.") {
        Some(CalcType::Pw)
    } else if content.contains("Program PHONON v.") {
        Some(CalcType::Ph)
    } else if is_wannier90(content) {
        Some(CalcType::Wannier90)
    } else {
        None
    }
}
