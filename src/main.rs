use std::{env, io, path::PathBuf};

mod app;
mod ph;
mod pw;
mod ui;
mod wannier90;

use app::App;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

const QE_MARKER: &str = "This program is part of the open-source Quantum ESPRESSO suite";
pub(crate) const WANN_MARKER: &str = "Welcome to the Maximally-Localized";
/// EPW drives wannier90 in library mode and appends to an existing `.wout`. When
/// it does, the banner can be missing entirely and the file starts straight at
/// the resume line, so this is the only marker such a file carries.
pub(crate) const WANN_RESUME_MARKER: &str = "Resuming Wannier90";

/// Whether `content` looks like wannier90 output, banner or not.
pub(crate) fn is_wannier90(content: &str) -> bool {
    content.contains(WANN_MARKER) || content.contains(WANN_RESUME_MARKER)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcType {
    Pw,
    Ph,
    Wannier90,
}

fn main() -> io::Result<()> {
    let logfile = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("Usage: qe-monitor <logfile>");
        std::process::exit(1);
    });

    // confirm that the file exists and is readable
    match std::fs::metadata(&logfile) {
        Ok(metadata) => {
            if !metadata.is_file() {
                eprintln!("Error: {} is not a file", logfile.display());
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("Error: could not access {}: {}", logfile.display(), err);
            std::process::exit(1);
        }
    }

    // check that we have a QE output file
    let qe_output = std::fs::read_to_string(&logfile)?;
    if !qe_output.contains(QE_MARKER) && !is_wannier90(&qe_output) {
        eprintln!(
            "Error: {} does not appear to be a QE/Wannier90 output file",
            logfile.display()
        );
        std::process::exit(1);
    }

    // detect calculation type
    let calc_type = detect_calc_type(&qe_output).unwrap_or_else(|| {
        eprintln!("Error: could not determine calculation type (pw.x or ph.x or wannier90.x)");
        std::process::exit(1);
    });

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // run app
    let mut app = App::new(logfile, calc_type);
    let res = app.run(&mut terminal);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn detect_calc_type(content: &str) -> Option<CalcType> {
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

#[cfg(test)]
mod tests {
    use super::{CalcType, detect_calc_type, is_wannier90};

    #[test]
    fn detects_pw_and_ph() {
        assert_eq!(
            detect_calc_type("     Program PWSCF v.7.5 starts on 1Jan2026"),
            Some(CalcType::Pw)
        );
        assert_eq!(
            detect_calc_type("     Program PHONON v.7.5 starts on 1Jan2026"),
            Some(CalcType::Ph)
        );
    }

    #[test]
    fn detects_wannier90_from_the_banner() {
        let banner = "             |                   WANNIER90                       |\n\
                      \x20            |        Welcome to the Maximally-Localized         |\n";
        assert_eq!(detect_calc_type(banner), Some(CalcType::Wannier90));
    }

    /// EPW appends to an existing `.wout` in library mode, and such a file can
    /// begin straight at the resume line with no banner anywhere. It used to be
    /// rejected as "not a QE/Wannier90 output file".
    #[test]
    fn detects_wannier90_without_a_banner() {
        let resumed = "\n Resuming Wannier90 at 01:11:28 \n";
        assert!(is_wannier90(resumed));
        assert_eq!(detect_calc_type(resumed), Some(CalcType::Wannier90));
    }

    /// EPW's own output carries neither marker, so it stays undetected rather
    /// than being mistaken for a wannier90 run.
    #[test]
    fn unrelated_output_is_not_wannier90() {
        assert!(!is_wannier90("     Program EPW v.6.0 starts on 22May2026"));
        assert_eq!(detect_calc_type("some unrelated text"), None);
    }
}
