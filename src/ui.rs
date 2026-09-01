use chrono::{DateTime, Local, Utc};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols,
    text::Line,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Tabs},
};

use crate::app::{App, Metrics};

pub fn ui(frame: &mut Frame, app: &App) {
    let title = Line::from(format!(
        " qe-monitor - {} ",
        app.filename.to_str().unwrap_or("")
    ));

    let last_modified_str = app
        .last_modified
        .map(|t| {
            DateTime::<Utc>::from(t)
                .with_timezone(&Local)
                .format(" %Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "unknown".to_string());
    let last_modified_line = Line::from(last_modified_str);

    let root = Block::new()
        .borders(Borders::TOP)
        .title(title.centered())
        .title(last_modified_line.right_aligned())
        .title(Line::from("q to quit ").left_aligned());
    let inner = root.inner(frame.area());
    frame.render_widget(root, frame.area());

    // 3 rows (same layout for all file types)
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Percentage(70),
            Constraint::Min(7),
        ])
        .split(inner);

    // header row
    let header = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(rows[0]);

    // main widgets
    let main_widgets = Layout::default()
        .direction(app.main_widget_orientation)
        .constraints([
            Constraint::Percentage(app.main_widget_split),
            Constraint::Percentage(100 - app.main_widget_split),
        ])
        .split(rows[1]);

    // footer row
    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(rows[2]);

    // render header
    render_run_info(frame, header[0], app);
    render_summary(frame, header[1], &app.metrics);
    render_header_right(frame, header[2], &app.metrics);

    // render main widgets
    render_main_1(frame, main_widgets[0], app);
    render_main_2(frame, main_widgets[1], app);

    // render footer
    render_latest_output_lines(frame, footer[0], app);
    render_footer_right(frame, footer[1], app);
}

fn render_run_info(frame: &mut Frame, area: Rect, app: &App) {
    let title = Line::from(" Run Info ").bold();
    let block = Block::new()
        .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
        .title(title.centered());

    let text = ratatui::text::Text::from(vec![
        Line::from(format!(
            "VER: {}",
            app.run_info.version.as_deref().unwrap_or("Unknown")
        )),
        Line::from(format!(
            "EX:  {}",
            app.run_info.executable.as_deref().unwrap_or("Unknown")
        )),
        Line::from(format!(
            "ST:  {}",
            app.run_info.start_time.as_deref().unwrap_or("Unknown")
        )),
        Line::from(format!(
            "MPI: {}",
            app.run_info.mpi_ranks.as_deref().unwrap_or("Unknown")
        )),
        Line::from(format!(
            "OMP: {}",
            app.run_info.omp_threads.as_deref().unwrap_or("Unknown")
        )),
    ]);

    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_summary(frame: &mut Frame, area: Rect, metrics: &Metrics) {
    match metrics {
        Metrics::Pw(pw) => crate::pw::ui::render_scf_summary(frame, area, pw),
        Metrics::Ph(ph) => crate::ph::ui::render_phonon_summary(frame, area, ph),
        Metrics::Wannier90(wannier90) => {
            crate::wannier90::ui::render_summary(frame, area, wannier90)
        }
    }
}

fn render_header_right(frame: &mut Frame, area: Rect, metrics: &Metrics) {
    match metrics {
        Metrics::Pw(pw) => {
            let block = Block::new()
                .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
                .title(Line::from(" Thresholds ").bold().centered());

            let fmt = |v: Option<f64>| {
                v.map(|x| format!("{:.2e}", x))
                    .unwrap_or_else(|| "-".to_string())
            };
            let fmt_cur = |v: Option<f64>| v.map(|x| format!("{:.2e}", x)).unwrap_or_default();

            let is_relax = pw.etot_conv_thr.is_some()
                || pw.forc_conv_thr.is_some()
                || pw.press_conv_thr.is_some();

            let mut lines = vec![Line::from(format!(
                "{:<12} {}",
                "scf [Ry]:",
                fmt(pw.scf_conv_thr)
            ))];

            if is_relax {
                let cur_e = pw.ion_dyn_etot_err.last().copied();
                let cur_f = pw.ion_dyn_forc_err.last().copied();
                let cur_p = pw.ion_dyn_press_err.last().copied();

                let converged = |cur: Option<f64>, thr: Option<f64>| -> Color {
                    match (cur, thr) {
                        (Some(c), Some(t)) if c < t => Color::LightGreen,
                        (Some(_), Some(_)) => Color::LightRed,
                        _ => Color::Reset,
                    }
                };

                let row = |label: &str, thr: Option<f64>, cur: Option<f64>| -> Line {
                    let color = converged(cur, thr);
                    let thr_str = fmt(thr);
                    let cur_str = fmt_cur(cur);
                    Line::from(vec![
                        ratatui::text::Span::raw(format!("{:<12} {:<10}", label, thr_str)),
                        ratatui::text::Span::styled(cur_str, Style::default().fg(color)),
                    ])
                };

                lines.push(Line::from(format!(
                    "{:<12} {:<9} {}",
                    "", "target", "current"
                )));
                lines.push(row("E [Ry]:", pw.etot_conv_thr, cur_e));
                lines.push(row("F [Ry/Bohr]:", pw.forc_conv_thr, cur_f));
                lines.push(row("P [kbar]:", pw.press_conv_thr, cur_p));
            }

            frame.render_widget(ratatui::widgets::Paragraph::new(lines).block(block), area);
        }
        Metrics::Ph(_) => {
            frame.render_widget(
                Block::new()
                    .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
                    .title(Line::from(" Header Right ").centered()),
                area,
            );
        }
        Metrics::Wannier90(_) => {
            frame.render_widget(
                Block::new()
                    .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
                    .title(Line::from(" Header Right ").centered()),
                area,
            );
        }
    }
}

fn render_main_1(frame: &mut Frame, area: Rect, app: &App) {
    app.left_charts.render(frame, area);
}

fn render_main_2(frame: &mut Frame, area: Rect, app: &App) {
    app.right_charts.render(frame, area);
}

fn render_footer_right(frame: &mut Frame, area: Rect, app: &App) {
    let shortcuts = if app.main_widget_orientation == Direction::Horizontal {
        "adj. layout: A←→D Space "
    } else {
        "adj. layout: W↑↓S Space "
    };
    frame.render_widget(
        Block::bordered()
            .title(Line::from(" Footer Right ").centered())
            .title_bottom(Line::from(shortcuts).right_aligned()),
        area,
    );
}

fn render_latest_output_lines(frame: &mut Frame, area: Rect, app: &App) {
    let n_lines = area.height as usize - 2; // leave space for borders

    let parse_time_str = app
        .last_parse_duration
        .map(|d| format!(" parsed in {:.1}ms ", d.as_secs_f64() * 1000.0))
        .unwrap_or_default();
    let block = Block::bordered()
        .title(Line::from(parse_time_str).left_aligned())
        .title(Line::from(" Latest Output ").bold().centered());

    let mut output_lines: Vec<Line> = app
        .output_file
        .lines()
        .rev()
        .take(n_lines)
        .map(|l| Line::from(l.to_string()))
        .collect();

    output_lines.reverse();

    frame.render_widget(Paragraph::new(output_lines).block(block), area);
}

/// Generic convergence chart that plots multiple datasets of (x, y) points
/// with log-scale y-axis labels. Used for SCF accuracy tracking.
///
/// If `threshold` is provided, a horizontal dashed line is drawn at log10(threshold).
/// Log-scale convergence scatter chart with optional threshold line.
/// Supports multiple datasets (colored differently) for overlaying e.g. last N blocks.
pub struct ConvergenceChart {
    pub title: String,
    pub x_label: &'static str,
    pub y_label: &'static str,
    pub datasets: Vec<Vec<(f64, f64)>>,
    pub threshold: Option<f64>,
}

impl ConvergenceChart {
    pub fn new(
        title: impl Into<String>,
        x_label: &'static str,
        y_label: &'static str,
        datasets: Vec<Vec<(f64, f64)>>,
        threshold: Option<f64>,
    ) -> Self {
        Self {
            title: title.into(),
            x_label,
            y_label,
            datasets,
            threshold,
        }
    }
}

impl Renderable for ConvergenceChart {
    fn render(&self, frame: &mut Frame, area: Rect) {
        use core::f64;

        let block =
            Block::bordered().title(Line::from(format!(" {} ", self.title)).bold().centered());

        if self.datasets.is_empty() || self.datasets.iter().all(|pts| pts.is_empty()) {
            frame.render_widget(block, area);
            return;
        }

        // Calculate bounds
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;

        for dataset in &self.datasets {
            for &(x, y) in dataset {
                x_min = x_min.min(x);
                x_max = x_max.max(x);
                y_min = y_min.min(y);
                y_max = y_max.max(y);
            }
        }

        if !x_min.is_finite() || x_min == x_max {
            x_min = 0.0;
            x_max = 10.0;
        }
        if let Some(thr) = self.threshold {
            let y_thr = thr.max(1e-22).log10();
            y_min = y_min.min(y_thr);
            y_max = y_max.max(y_thr);
        }

        if !y_min.is_finite() || y_min == y_max {
            y_min = -16.0;
            y_max = -6.0;
        } else {
            let pad = ((y_max - y_min).abs() * 0.10).max(0.1);
            y_min -= pad;
            y_max += pad;
        }
        let x_mid = (x_min + x_max) / 2.0;
        let y_mid = (y_min + y_max) / 2.0;

        let colors = [Color::LightRed, Color::LightYellow, Color::LightGreen];

        let threshold_line: Option<Vec<(f64, f64)>> = self.threshold.map(|thr| {
            let y_threshold = thr.max(1e-22).log10();
            vec![(x_min, y_threshold), (x_max, y_threshold)]
        });

        let mut ratatui_datasets: Vec<Dataset> = Vec::new();

        if let Some(ref line) = threshold_line {
            ratatui_datasets.push(
                Dataset::default()
                    .name("thr")
                    .graph_type(GraphType::Line)
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(Color::DarkGray))
                    .data(line),
            );
        }

        let n = self.datasets.len();
        for (i, points) in self.datasets.iter().enumerate() {
            let color_idx = i % colors.len();
            let mut dataset = Dataset::default()
                .graph_type(GraphType::Scatter)
                .style(Style::default().fg(colors[color_idx]))
                .data(points)
                .marker(symbols::Marker::Dot);
            if n > 1 {
                dataset = dataset.name(format!("-{}", n - i));
            }
            ratatui_datasets.push(dataset);
        }

        let chart = Chart::new(ratatui_datasets)
            .block(block)
            .x_axis(
                Axis::default()
                    .title(self.x_label)
                    .bounds([x_min, x_max])
                    .labels([
                        Line::from(format!("{:.0}", x_min)),
                        Line::from(format!("{:.0}", x_mid)),
                        Line::from(format!("{:.0}", x_max)),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .title(self.y_label)
                    .bounds([y_min, y_max])
                    .labels([
                        Line::from(format!("{:.1e}", 10f64.powf(y_min))),
                        Line::from(format!("{:.1e}", 10f64.powf(y_mid))),
                        Line::from(format!("{:.1e}", 10f64.powf(y_max))),
                    ]),
            );

        frame.render_widget(chart, area);
    }
}

// ========================================
// TabGroup - generic tabbed widget container
// ========================================

pub trait Renderable {
    fn render(&self, frame: &mut Frame, area: Rect);
}

/// Which keys select tabs in a `TabGroup`. Lets two panels coexist without
/// clashing: the left panel uses function keys, the right uses number keys.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabKeys {
    #[default]
    Digit,
    Function,
}

#[derive(Default)]
pub struct TabGroup {
    tabs: Vec<(&'static str, Box<dyn Renderable>)>,
    active: usize,
    keys: TabKeys,
    /// Until the user selects a tab, track the last tab as the set grows. Lets the
    /// right panel default to a different plot than the left instead of duplicating it.
    prefer_last: bool,
    user_selected: bool,
}

impl std::fmt::Debug for TabGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabGroup")
            .field("active", &self.active)
            .field("num_tabs", &self.tabs.len())
            .field("keys", &self.keys)
            .finish()
    }
}

impl TabGroup {
    /// Set which key family selects tabs (builder style).
    pub fn with_keys(mut self, keys: TabKeys) -> Self {
        self.keys = keys;
        self
    }

    /// Default to the last tab (until the user picks one). Builder style.
    pub fn with_default_last(mut self) -> Self {
        self.prefer_last = true;
        self
    }

    /// Replace all tabs, preserving the active index if still in range.
    pub fn rebuild(&mut self, tabs: Vec<(&'static str, Box<dyn Renderable>)>) {
        self.tabs = tabs;
        if self.tabs.is_empty() {
            self.active = 0;
        } else if self.prefer_last && !self.user_selected {
            self.active = self.tabs.len() - 1;
        } else if self.active >= self.tabs.len() {
            self.active = 0;
        }
    }

    /// Handle a key event (number or function keys, depending on `keys`).
    /// Returns true if consumed.
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        let idx = match (self.keys, code) {
            (TabKeys::Digit, KeyCode::Char(c)) => match c.to_digit(10) {
                Some(digit) => (digit as usize).wrapping_sub(1), // '1' -> 0
                None => return false,
            },
            (TabKeys::Function, KeyCode::F(n)) => (n as usize).wrapping_sub(1), // F1 -> 0
            _ => return false,
        };
        if idx < self.tabs.len() {
            self.active = idx;
            self.user_selected = true;
            return true;
        }
        false
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.tabs.is_empty() {
            frame.render_widget(Block::bordered(), area);
            return;
        }

        // Single tab - no tab bar, just render the widget directly
        if self.tabs.len() == 1 {
            self.tabs[0].1.render(frame, area);
            return;
        }

        // Multiple tabs: tab bar on top, content below
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);

        let titles: Vec<Line> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, (label, _))| {
                let text = match self.keys {
                    TabKeys::Function => format!(" F{} {} ", i + 1, label),
                    TabKeys::Digit => format!(" {} {} ", i + 1, label),
                };
                if i == self.active {
                    Line::from(text).style(Style::default().fg(Color::White).bold())
                } else {
                    Line::from(text).style(Style::default().fg(Color::DarkGray))
                }
            })
            .collect();

        let tabs = Tabs::new(titles)
            .select(self.active)
            .divider("│")
            .highlight_style(Style::default().fg(Color::White).bold());

        frame.render_widget(tabs, chunks[0]);

        self.tabs[self.active].1.render(frame, chunks[1]);
    }
}

/// A `ConvergenceChart` when there is data, otherwise an empty bordered block
/// (optionally titled). Lets a fixed right-hand chart live inside a `TabGroup`
/// while preserving the previous "empty run shows a blank titled panel" look.
pub struct ChartOrEmpty {
    pub chart: Option<ConvergenceChart>,
    pub empty_title: Option<&'static str>,
}

impl Renderable for ChartOrEmpty {
    fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.chart {
            Some(chart) => chart.render(frame, area),
            None => {
                let mut block = Block::bordered();
                if let Some(title) = self.empty_title {
                    block = block.title(Line::from(title).bold().centered());
                }
                frame.render_widget(block, area);
            }
        }
    }
}

pub fn stats_line(label: &str, xs: &[f64], decimals: usize) -> String {
    if xs.is_empty() {
        return format!("{label} -");
    }
    let (mu, sigma) = mean_std(xs);
    format!("{label} {mu:.d$} ± {sigma:.d$}", d = decimals)
}

pub fn mean_std(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
    (mean, var.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;
    use ratatui::backend::TestBackend;

    fn buffer_text(term: &ratatui::Terminal<TestBackend>) -> String {
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// Render a `build_tabs` set into left (F-keys) and right (numbers, default
    /// last) panels; assert both draw and that the two panels default to different
    /// tabs when more than one plot exists.
    fn check(tabs_fn: impl Fn() -> Vec<(&'static str, Box<dyn Renderable>)>, expect: &str) {
        let n = tabs_fn().len();
        let mut left = TabGroup::default().with_keys(TabKeys::Function);
        let mut right = TabGroup::default()
            .with_keys(TabKeys::Digit)
            .with_default_last();
        left.rebuild(tabs_fn());
        right.rebuild(tabs_fn());

        // Different default plot per panel when there is a choice.
        if n > 1 {
            assert_ne!(
                left.active, right.active,
                "{expect}: panels default to same tab"
            );
            assert_eq!(
                right.active,
                n - 1,
                "{expect}: right should default to last"
            );
        }

        let mut term = ratatui::Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| {
            let cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(f.area());
            left.render(f, cols[0]);
            right.render(f, cols[1]);
        })
        .unwrap();
        let text = buffer_text(&term);
        assert!(text.contains(expect), "{expect}: not rendered -> {text:?}");

        // F-keys drive left, digits drive right; they must not collide.
        assert!(left.handle_key(KeyCode::F(1)));
        assert!(!left.handle_key(KeyCode::Char('1')));
        assert!(right.handle_key(KeyCode::Char('1')));
        assert!(!right.handle_key(KeyCode::F(1)));
    }

    #[test]
    fn all_calc_types_have_both_panels() {
        use crate::{ph, pw, wannier90};
        check(|| pw::ui::build_tabs(&pw::PwMetrics::default()), "SCF acc.");
        check(|| ph::ui::build_tabs(&ph::PhMetrics::default()), "SCF acc.");
        check(
            || wannier90::ui::build_tabs(&wannier90::WannierMetrics::default()),
            "Spread abs",
        );
    }
}
