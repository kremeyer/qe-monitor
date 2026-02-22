use core::f64;

use ratatui::style::{Color, Stylize};
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};

use crate::pw::{PwCalcType, PwMetrics};

pub fn render_scf_summary(frame: &mut Frame, area: Rect, pw: &PwMetrics) {
    match pw.calc_type {
        PwCalcType::Nscf => {
            let block = Block::bordered().title(Line::from(" NSCF stats ").bold().centered());

            let total_kpts = pw.band_blocks.last().and_then(|b| b.num_kpts).unwrap_or(0);
            let completed = pw
                .band_blocks
                .last()
                .map(|b| b.kpt_number.len() as u32)
                .unwrap_or(0);

            // Calculate individual k-point durations from cumulative times
            let kpt_durations: Vec<f64> = pw
                .band_blocks
                .last()
                .map(|b| {
                    b.cpu_time
                        .windows(2)
                        .map(|w| w[1] - w[0])
                        .filter(|&dt| dt.is_finite() && dt > 0.0)
                        .collect()
                })
                .unwrap_or_default();

            // Calculate average time per k-point
            let time_per_kpt = pw
                .band_blocks
                .last()
                .and_then(|b| b.time_per_iteration())
                .unwrap_or(0.0);

            let mut est_time_left = f64::NAN;
            if completed >= total_kpts {
                est_time_left = 0.0;
            } else if completed > 0 && time_per_kpt.is_finite() && time_per_kpt > 0.0 {
                let kpts_left = (total_kpts - completed) as f64;
                est_time_left = time_per_kpt * kpts_left;
            }

            // Pick a common time unit based on the larger value
            let max_val = if est_time_left.is_finite() {
                est_time_left.max(time_per_kpt)
            } else {
                time_per_kpt
            };
            let (time_unit, divisor) = if max_val >= 36000.0 {
                ("h", 3600.0)
            } else if max_val >= 600.0 {
                ("m", 60.0)
            } else {
                ("s", 1.0)
            };

            // Scale durations to the chosen unit
            let kpt_durations_scaled: Vec<f64> =
                kpt_durations.iter().map(|&dt| dt / divisor).collect();

            let est_time_left_scaled = est_time_left / divisor;

            // Calculate std deviation for time per k-point
            let (_, std_per_kpt) = if !kpt_durations_scaled.is_empty() {
                crate::ui::mean_std(&kpt_durations_scaled)
            } else {
                (0.0, 0.0)
            };

            // Propagate error to time left estimate
            let kpts_left = (total_kpts.saturating_sub(completed)) as f64;
            let est_time_left_error = std_per_kpt * kpts_left;

            let mut lines = Vec::new();
            lines.push(Line::from(format!(
                "kpts:          {}/{}",
                completed, total_kpts
            )));
            lines.push(Line::from(crate::ui::stats_line(
                &format!("time/kpt [{}]: ", time_unit),
                &kpt_durations_scaled,
                2,
            )));

            if est_time_left.is_finite() {
                lines.push(Line::from(format!(
                    "time left [{}]: {:.2} ± {:.2}",
                    time_unit, est_time_left_scaled, est_time_left_error
                )));
            }

            frame.render_widget(Paragraph::new(lines).block(block), area);
        }
        PwCalcType::Scf => {
            let block = Block::bordered().title(Line::from(" SCF stats ").bold().centered());

            let scf_blocks = &pw.scf_blocks;

            let completed_slice: &[crate::pw::ScfBlock] = if scf_blocks.len() >= 2 {
                &scf_blocks[..scf_blocks.len() - 1]
            } else {
                &scf_blocks[..]
            };

            let mut completed: Vec<&crate::pw::ScfBlock> = completed_slice.iter().collect();
            if completed.is_empty() {
                completed = scf_blocks
                    .iter()
                    .filter(|b| b.conv_iters.is_some())
                    .collect();
            }
            if completed.is_empty() {
                completed = scf_blocks.iter().collect();
            }

            let iters: Vec<f64> = completed
                .iter()
                .filter_map(|b| b.iterations_to_converge().map(|v| v as f64))
                .filter(|&v| v > 0.0)
                .collect();

            let sec_per_iter: Vec<f64> = completed
                .iter()
                .filter_map(|b| b.time_per_iteration())
                .filter(|&v| v.is_finite() && v > 0.0)
                .collect();

            let sec_per_calc: Vec<f64> = completed
                .iter()
                .filter_map(|b| b.time_per_calculation())
                .filter(|&v| v.is_finite() && v >= 0.0)
                .collect();

            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(format!(
                "blocks:        {} (used: {})",
                scf_blocks.len(),
                completed.len()
            )));
            lines.push(Line::from(crate::ui::stats_line(
                "iters/calc:   ",
                &iters,
                1,
            )));
            lines.push(Line::from(crate::ui::stats_line(
                "time/iter [s]:",
                &sec_per_iter,
                2,
            )));
            lines.push(Line::from(crate::ui::stats_line(
                "time/calc [s]:",
                &sec_per_calc,
                2,
            )));

            frame.render_widget(Paragraph::new(lines).block(block), area);
        }
    }
}

pub fn render_total_energy_chart(frame: &mut Frame, area: Rect, pm: &PwMetrics) {
    // For NSCF calculations, show k-point timing chart
    if pm.calc_type == PwCalcType::Nscf {
        render_kpt_time_chart(frame, area, pm);
        return;
    }

    render_energy_delta_chart(frame, area, &pm.total_energy);
}

/// Dedicated chart for relaxation energy changes.
/// Splits points into "energy went down" (green ▼) and "energy went up" (red ▲)
/// so the direction is immediately visible.
fn render_energy_delta_chart(frame: &mut Frame, area: Rect, energies: &[f64]) {
    use core::f64;
    use ratatui::style::Style;
    use ratatui::symbols;

    let title = " |ΔE| ";
    let block = Block::bordered().title(Line::from(title).bold().centered());

    if energies.len() < 2 {
        frame.render_widget(block, area);
        return;
    }

    // Split into down (E decreased) and up (E increased) datasets
    let mut down_pts: Vec<(f64, f64)> = Vec::new();
    let mut up_pts: Vec<(f64, f64)> = Vec::new();

    for (i, w) in energies.windows(2).enumerate() {
        let de = w[1] - w[0];
        let abs_de = de.abs().max(1e-30);
        let pt = ((i + 1) as f64, abs_de.log10());
        if de <= 0.0 {
            down_pts.push(pt);
        } else {
            up_pts.push(pt);
        }
    }

    // Calculate bounds
    let all_pts = down_pts.iter().chain(up_pts.iter());
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for &(x, y) in all_pts {
        x_min = x_min.min(x);
        x_max = x_max.max(x);
        y_min = y_min.min(y);
        y_max = y_max.max(y);
    }

    if !x_min.is_finite() || x_min == x_max {
        x_min = 0.0;
        x_max = 10.0;
    }
    if !y_min.is_finite() || y_min == y_max {
        y_min = -16.0;
        y_max = -6.0;
    } else {
        let pad = ((y_max - y_min).abs() * 0.10).max(0.5);
        y_min -= pad;
        y_max += pad;
    }

    let x_mid = (x_min + x_max) / 2.0;
    let y_mid = (y_min + y_max) / 2.0;

    let mut datasets: Vec<Dataset> = Vec::new();

    if !down_pts.is_empty() {
        datasets.push(
            Dataset::default()
                .name("▼")
                .graph_type(GraphType::Scatter)
                .marker(symbols::Marker::Dot)
                .style(Style::default().fg(Color::LightGreen))
                .data(&down_pts),
        );
    }
    if !up_pts.is_empty() {
        datasets.push(
            Dataset::default()
                .name("▲")
                .graph_type(GraphType::Scatter)
                .marker(symbols::Marker::Dot)
                .style(Style::default().fg(Color::LightRed))
                .data(&up_pts),
        );
    }

    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .title("step")
                .bounds([x_min, x_max])
                .labels([
                    Line::from(format!("{:.0}", x_min)),
                    Line::from(format!("{:.0}", x_mid)),
                    Line::from(format!("{:.0}", x_max)),
                ]),
        )
        .y_axis(
            Axis::default()
                .title("|ΔE| [Ry]")
                .bounds([y_min, y_max])
                .labels([
                    Line::from(format!("{:.1e}", 10f64.powf(y_min))),
                    Line::from(format!("{:.1e}", 10f64.powf(y_mid))),
                    Line::from(format!("{:.1e}", 10f64.powf(y_max))),
                ]),
        );

    frame.render_widget(chart, area);

    // Post-process: replace dot markers with ▼/▲ based on color
    let buf = frame.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = &buf[(x, y)];
            let sym = cell.symbol().to_string();
            if sym == "•" {
                let fg = cell.fg;
                let replacement = if fg == Color::LightGreen {
                    "▼"
                } else if fg == Color::LightRed {
                    "▲"
                } else {
                    continue;
                };
                buf[(x, y)].set_symbol(replacement);
            }
        }
    }
}

pub fn render_scf_accuracy_chart(frame: &mut Frame, area: Rect, pw: &PwMetrics) {
    let scf_blocks = &pw.scf_blocks;
    if scf_blocks.is_empty() {
        let block = match pw.calc_type {
            PwCalcType::Nscf => Block::bordered(),
            PwCalcType::Scf => {
                Block::bordered().title(Line::from(" SCF Accuracy ").bold().centered())
            }
        };
        frame.render_widget(block, area);
        return;
    }

    // Take last 3 blocks to reduce clutter
    let mut last3_scf_blocks: Vec<&crate::pw::ScfBlock> = scf_blocks.iter().rev().take(3).collect();
    last3_scf_blocks.reverse();

    // Build points for each block
    let mut all_points: Vec<Vec<(f64, f64)>> = Vec::new();
    for block in &last3_scf_blocks {
        let pts: Vec<(f64, f64)> = block
            .iteration
            .iter()
            .zip(block.accuracy.iter())
            .map(|(&iteration, &accuracy)| (iteration as f64, accuracy.max(1e-30).log10()))
            .collect();
        all_points.push(pts)
    }

    crate::ui::render_convergence_chart(
        frame,
        area,
        "SCF Accuracy",
        "iteration",
        "accuracy",
        all_points,
        pw.conv_threshold,
    );
}

pub fn render_kpt_time_chart(frame: &mut Frame, area: Rect, pw: &PwMetrics) {
    let block = Block::bordered().title(Line::from(" K-point Time ").bold().centered());

    // Get the last band block
    let band_block = match pw.band_blocks.last() {
        Some(b) => b,
        None => {
            frame.render_widget(block, area);
            return;
        }
    };

    // Calculate individual k-point durations from cumulative times
    let kpt_durations: Vec<f64> = band_block
        .cpu_time
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&dt| dt.is_finite() && dt > 0.0)
        .collect();

    if kpt_durations.is_empty() {
        frame.render_widget(block, area);
        return;
    }

    // Create points (k-point index, time)
    let points: Vec<(f64, f64)> = kpt_durations
        .iter()
        .enumerate()
        .map(|(i, &time)| ((i + 1) as f64, time))
        .collect();

    let x_min = 1.0;
    let x_max = points.len() as f64;

    // Calculate y bounds
    let (mut y_min, mut y_max) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &(_, y)| {
            (mn.min(y), mx.max(y))
        });

    let pad = ((y_max - y_min).abs() * 0.05).max(1e-6);
    y_min = (y_min - pad).max(0.0);
    y_max += pad;

    let x_mid = (x_min + x_max) / 2.0;
    let y_mid = (y_min + y_max) / 2.0;

    let datasets = vec![
        Dataset::default()
            .graph_type(GraphType::Scatter)
            .data(&points),
    ];

    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .title("k-point")
                .bounds([x_min, x_max])
                .labels([
                    Line::from(format!("{:.0}", x_min)),
                    Line::from(format!("{:.0}", x_mid)),
                    Line::from(format!("{:.0}", x_max)),
                ]),
        )
        .y_axis(
            Axis::default()
                .title("time [s]")
                .bounds([y_min, y_max])
                .labels([
                    Line::from(format!("{:.2}", y_min)),
                    Line::from(format!("{:.2}", y_mid)),
                    Line::from(format!("{:.2}", y_max)),
                ]),
        );

    frame.render_widget(chart, area);
}
