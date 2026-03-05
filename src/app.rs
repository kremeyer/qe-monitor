use std::{
    io,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{CalcType, ph, pw, ui, ui::TabGroup, wannier90};

#[derive(Debug, Default)]
pub struct RunInfo {
    pub version: Option<String>,
    pub executable: Option<String>,
    pub start_time: Option<String>,
    pub mpi_ranks: Option<String>,
    pub omp_threads: Option<String>,
}

#[derive(Debug)]
pub enum Metrics {
    Pw(pw::PwMetrics),
    Ph(ph::PhMetrics),
    Wannier90(wannier90::WannierMetrics),
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

    pub left_charts: TabGroup,

    pub output_file: String,
    pub last_modified: Option<SystemTime>,
    last_size: u64,
    pub last_parse_duration: Option<Duration>,
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
                CalcType::Wannier90 => Metrics::Wannier90(wannier90::WannierMetrics::default()),
            },
            left_charts: TabGroup::default(),
            output_file: String::new(),
            last_modified: None,
            last_size: 0,
            last_parse_duration: None,
        };

        // initial read + parse
        app.output_file = std::fs::read_to_string(&app.filename).unwrap_or_default();
        let t0 = Instant::now();
        app.parse_content();
        app.last_parse_duration = Some(t0.elapsed());

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

        // Let TabGroup handle number keys first
        if self.left_charts.handle_key(key_event.code) {
            return;
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

        // Only read the new bytes appended since the last check.
        // If the file shrank (e.g. replaced), fall back to a full re-read.
        if size >= self.last_size && self.last_size > 0 {
            let mut f = std::fs::File::open(&self.filename)?;
            f.seek(SeekFrom::Start(self.last_size))?;
            let mut new_content = String::new();
            f.read_to_string(&mut new_content)?;
            self.output_file.push_str(&new_content);
        } else {
            self.output_file = std::fs::read_to_string(&self.filename).unwrap_or_default();
        }

        let t0 = Instant::now();
        self.parse_content();
        self.last_parse_duration = Some(t0.elapsed());

        self.last_modified = modified;
        self.last_size = size;
        Ok(())
    }

    fn parse_content(&mut self) {
        match self.calc_type {
            CalcType::Pw => {
                self.run_info = pw::parse_run_info(&self.output_file);
                self.metrics = Metrics::Pw(pw::parse_metrics(&self.output_file));
            }
            CalcType::Ph => {
                self.run_info = ph::parse_run_info(&self.output_file);
                self.metrics = Metrics::Ph(ph::parse_metrics(&self.output_file));
            }
            CalcType::Wannier90 => {
                self.run_info = wannier90::parse_run_info(&self.output_file);
                self.metrics = Metrics::Wannier90(wannier90::parse_metrics(&self.output_file));
            }
        }
        self.build_left_charts();
    }

    fn build_left_charts(&mut self) {
        match &self.metrics {
            Metrics::Pw(pm) => {
                self.left_charts.rebuild(pw::ui::build_left_tabs(pm));
            }
            Metrics::Ph(pm) => {
                self.left_charts.rebuild(ph::ui::build_left_tabs(pm));
            }
            Metrics::Wannier90(wm) => {
                self.left_charts.rebuild(wannier90::ui::build_left_tabs(wm));
            }
        }
    }
}
