use chrono::{DateTime, Local, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols,
    text::Line,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph},
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
    render_main_2(frame, main_widgets[1], &app.metrics);

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
                    .unwrap_or_else(|| "—".to_string())
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
    match &app.metrics {
        Metrics::Pw(pw) => crate::pw::ui::render_total_energy_chart(frame, area, pw),
        Metrics::Ph(ph) => {
            crate::ph::ui::render_representation_iterations_chart(frame, area, ph, 0)
        }
        Metrics::Wannier90(wannier90) => {
            crate::wannier90::ui::render_subspace_disentanglement_chart(frame, area, wannier90)
        }
    }
}

fn render_main_2(frame: &mut Frame, area: Rect, metrics: &Metrics) {
    match metrics {
        Metrics::Pw(pw) => crate::pw::ui::render_scf_accuracy_chart(frame, area, pw),
        Metrics::Ph(ph) => crate::ph::ui::render_scf_accuracy_chart(frame, area, ph),
        Metrics::Wannier90(wannier90) => {
            crate::wannier90::ui::render_spread_chart(frame, area, wannier90)
        }
    }
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
pub fn render_convergence_chart(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    x_label: &str,
    y_label: &str,
    all_points: Vec<Vec<(f64, f64)>>,
    threshold: Option<f64>,
) {
    use core::f64;

    let block = Block::bordered().title(Line::from(format!(" {} ", title)).bold().centered());

    if all_points.is_empty() || all_points.iter().all(|pts| pts.is_empty()) {
        frame.render_widget(block, area);
        return;
    }

    // Calculate bounds
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    for dataset in &all_points {
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
    // Extend bounds to always include the threshold line
    if let Some(thr) = threshold {
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

    // Prepare threshold line data
    let threshold_line: Option<Vec<(f64, f64)>> = threshold.map(|thr| {
        let y_threshold = thr.max(1e-22).log10();
        vec![(x_min, y_threshold), (x_max, y_threshold)]
    });

    let mut datasets: Vec<Dataset> = Vec::new();

    // Add threshold line dataset
    if let Some(ref line) = threshold_line {
        datasets.push(
            Dataset::default()
                .name("thr")
                .graph_type(GraphType::Line)
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::DarkGray))
                .data(line),
        );
    }

    // Add data points
    let n = all_points.len();
    for (i, points) in all_points.iter().enumerate() {
        let color_idx = i % colors.len();
        let mut dataset = Dataset::default()
            .graph_type(GraphType::Scatter)
            .style(Style::default().fg(colors[color_idx]))
            .data(points)
            .marker(symbols::Marker::Dot);
        if n > 1 {
            dataset = dataset.name(format!("-{}", n - i));
        }
        datasets.push(dataset);
    }

    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .title(x_label)
                .bounds([x_min, x_max])
                .labels([
                    Line::from(format!("{:.0}", x_min)),
                    Line::from(format!("{:.0}", x_mid)),
                    Line::from(format!("{:.0}", x_max)),
                ]),
        )
        .y_axis(
            Axis::default()
                .title(y_label)
                .bounds([y_min, y_max])
                .labels([
                    Line::from(format!("{:.1e}", 10f64.powf(y_min))),
                    Line::from(format!("{:.1e}", 10f64.powf(y_mid))),
                    Line::from(format!("{:.1e}", 10f64.powf(y_max))),
                ]),
        );

    frame.render_widget(chart, area);
}

pub fn stats_line(label: &str, xs: &[f64], decimals: usize) -> String {
    if xs.is_empty() {
        return format!("{label} —");
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
