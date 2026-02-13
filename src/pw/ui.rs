use core::f64;

use ratatui::style::Stylize;
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
            let kpt_durations_scaled: Vec<f64> = kpt_durations
                .iter()
                .map(|&dt| dt / divisor)
                .collect();

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
    let e = &pm.total_energy;

    if e.len() < 2 {
        let block = match pm.calc_type {
            PwCalcType::Nscf => Block::bordered(),
            PwCalcType::Scf => Block::bordered().title(Line::from(" log10(ΔE) ").bold().centered()),
        };
        frame.render_widget(block, area);
        return;
    }

    let block = Block::bordered().title(Line::from(" log10(ΔE) ").bold().centered());

    // Plot last N points (keep the chart readable)
    let n = 400usize.min(e.len());
    let start = e.len() - n;
    let slice = &e[start..];

    // shift energies to positive, then log10
    let e_min = slice.iter().copied().fold(f64::INFINITY, |mn, v| mn.min(v));
    let eps = 1e-7_f64;

    let points: Vec<(f64, f64)> = slice
        .iter()
        .enumerate()
        .map(|(i, &y)| {
            let x = (start + i) as f64;
            let y_log = ((y - e_min) + eps).log10();
            (x, y_log)
        })
        .collect();

    let x_min = start as f64;
    let x_max = (e.len() - 1) as f64;

    let (mut y_min, mut y_max) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &(_, y)| {
            (mn.min(y), mx.max(y))
        });

    let pad = ((y_max - y_min).abs() * 0.05).max(1e-6);
    y_min -= pad;
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
                .title("step")
                .bounds([x_min, x_max])
                .labels([
                    Line::from(format!("{:.0}", x_min)),
                    Line::from(format!("{:.0}", x_mid)),
                    Line::from(format!("{:.0}", x_max)),
                ]),
        )
        .y_axis(Axis::default().title("Ry").bounds([y_min, y_max]).labels([
            Line::from(format!("{:.6}", y_min)),
            Line::from(format!("{:.6}", y_mid)),
            Line::from(format!("{:.6}", y_max)),
        ]));

    frame.render_widget(chart, area);
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
