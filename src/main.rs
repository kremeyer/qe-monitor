use std::{env, io, path::PathBuf};

mod app;
mod ph;
mod pw;
mod ui;

use app::App;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

const QE_MARKER: &str = "This program is part of the open-source Quantum ESPRESSO suite";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcType {
    Pw,
    Ph,
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
    if !qe_output.contains(QE_MARKER) {
        eprintln!(
            "Error: {} does not appear to be a QE output file",
            logfile.display()
        );
        std::process::exit(1);
    }

    // detect calculation type
    let calc_type = detect_calc_type(&qe_output).unwrap_or_else(|| {
        eprintln!("Error: could not determine calculation type (pw.x or ph.x)");
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
    } else {
        None
    }
}
