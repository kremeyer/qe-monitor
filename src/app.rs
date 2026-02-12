use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{CalcType, ph, pw, ui};

#[derive(Debug, Default)]
pub struct RunInfo {
    pub qe_version: Option<String>,
    pub executable: Option<String>,
    pub start_time: Option<String>,
    pub mpi_ranks: Option<String>,
    pub omp_threads: Option<String>,
}

#[derive(Debug)]
pub enum Metrics {
    Pw(pw::PwMetrics),
    Ph(ph::PhMetrics),
}

impl Default for Metrics {
    fn default() -> Self {
        Metrics::Pw(pw::PwMetrics::default())
    }
}

#[derive(Debug)]
pub struct App {
    exit: bool,
    pub main_widget_split: u16, // percentage for main widget split
    pub main_widget_orientation: ratatui::layout::Direction, // horizontal or vertical

    pub filename: PathBuf,
    pub calc_type: CalcType,

    pub run_info: RunInfo,
    pub metrics: Metrics,

    pub qe_output: String,
    pub last_modified: Option<SystemTime>,
    last_size: u64,
}

impl App {
    pub fn new(filename: PathBuf, calc_type: CalcType) -> Self {
        let mut app = Self {
            exit: false,
            main_widget_split: 50,
            main_widget_orientation: ratatui::layout::Direction::Horizontal,
            filename,
            calc_type,
            run_info: RunInfo::default(),
            metrics: match calc_type {
                CalcType::Pw => Metrics::Pw(pw::PwMetrics::default()),
                CalcType::Ph => Metrics::Ph(ph::PhMetrics::default()),
            },
            qe_output: String::new(),
            last_modified: None,
            last_size: 0,
        };

        // initial read + parse
        app.qe_output = std::fs::read_to_string(&app.filename).unwrap_or_default();
        app.parse_content();

        // store metadata snapshot
        if let Ok(md) = std::fs::metadata(&app.filename) {
            app.last_modified = md.modified().ok();
            app.last_size = md.len();
        }

        app
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        let tick_rate = Duration::from_millis(250);
        let mut last_tick = Instant::now();

        while !self.exit {
            terminal.draw(|frame| ui::ui(frame, self))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)?
                && let Event::Key(key_event) = event::read()?
                && key_event.kind == KeyEventKind::Press
            {
                self.handle_key_event(key_event);
            }

            if last_tick.elapsed() >= tick_rate {
                let _ = self.refresh(); // ignore transient read errors
                last_tick = Instant::now();
            }
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if let KeyCode::Char('q') = key_event.code {
            self.exit = true;
        }

        if let KeyCode::Char(' ') = key_event.code {
            if self.main_widget_orientation == ratatui::layout::Direction::Horizontal {
                self.main_widget_orientation = ratatui::layout::Direction::Vertical
            } else {
                self.main_widget_orientation = ratatui::layout::Direction::Horizontal
            }
        }
        if self.main_widget_orientation == ratatui::layout::Direction::Horizontal {
            if let KeyCode::Char('a') | KeyCode::Left = key_event.code {
                self.main_widget_split = self.main_widget_split.saturating_sub(15).max(20);
            }
            if let KeyCode::Char('d') | KeyCode::Right = key_event.code {
                self.main_widget_split = (self.main_widget_split + 15).min(80);
            }
        } else {
            if let KeyCode::Char('w') | KeyCode::Up = key_event.code {
                self.main_widget_split = self.main_widget_split.saturating_sub(15).max(20);
            }
            if let KeyCode::Char('s') | KeyCode::Down = key_event.code {
                self.main_widget_split = (self.main_widget_split + 15).min(80);
            }
        }
    }

    fn refresh(&mut self) -> io::Result<()> {
        let metadata = std::fs::metadata(&self.filename)?;
        let modified = metadata.modified().ok();
        let size = metadata.len();

        if modified == self.last_modified && size == self.last_size {
            return Ok(()); // no changes
        }

        self.qe_output = std::fs::read_to_string(&self.filename).unwrap_or_default();
        self.parse_content();

        self.last_modified = modified;
        self.last_size = size;
        Ok(())
    }

    fn parse_content(&mut self) {
        match self.calc_type {
            CalcType::Pw => {
                self.run_info = pw::parse_run_info(&self.qe_output);
                self.metrics = Metrics::Pw(pw::parse_metrics(&self.qe_output));
            }
            CalcType::Ph => {
                self.run_info = ph::parse_run_info(&self.qe_output);
                self.metrics = Metrics::Ph(ph::parse_metrics(&self.qe_output));
            }
        }
    }
}
