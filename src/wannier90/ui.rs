use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use ratatui::{Frame, layout::Rect, text::Line, widgets::Block};

use crate::ui::{ConvergenceChart, Renderable};
use crate::wannier90::WannierMetrics;

pub fn render_summary(frame: &mut Frame, area: Rect, wm: &WannierMetrics) {
    let block = Block::bordered().title(Line::from(" Wannierisation ").bold().centered());

    let mut lines: Vec<Line> = Vec::new();

    // Wannierization: iter 0 is the initial state, not a real iteration
    let n_w = wm.spread_block.spread.len().saturating_sub(1);
    let max_w = wm.wannierize_max_iterations.unwrap_or(0);

    // Per-iteration times (cpu_time is cumulative)
    let w_iter_times: Vec<f64> = wm
        .spread_block
        .cpu_time
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&dt| dt.is_finite() && dt > 0.0)
        .collect();

    let mean_w = if !w_iter_times.is_empty() {
        w_iter_times.iter().sum::<f64>() / w_iter_times.len() as f64
    } else {
        f64::NAN
    };

    let iters_left_w = max_w.saturating_sub(n_w as u32) as f64;
    let mut est_time_left_w = if mean_w.is_finite() && max_w > 0 {
        mean_w * iters_left_w
    } else {
        f64::NAN
    };

    // Pick a common time unit based on the larger of mean and ETA
    let max_val = if est_time_left_w.is_finite() {
        est_time_left_w.max(mean_w)
    } else {
        mean_w
    };
    let (time_unit, divisor) = if max_val >= 36000.0 {
        ("h", 3600.0)
    } else if max_val >= 600.0 {
        ("m", 60.0)
    } else {
        ("s", 1.0)
    };
    est_time_left_w /= divisor;

    let w_scaled: Vec<f64> = w_iter_times.iter().map(|&v| v / divisor).collect();
    let (_, std_w) = if !w_scaled.is_empty() {
        crate::ui::mean_std(&w_scaled)
    } else {
        (0.0, 0.0)
    };
    let est_time_left_w_err = std_w * iters_left_w;

    if wm.has_disentanglement {
        let n_d = wm
            .disentanglement_block
            .as_ref()
            .map(|db| db.cpu_time.len())
            .unwrap_or(0);
        let max_d = wm.dis_max_iterations.unwrap_or(0);
        let max_d_str = if max_d > 0 {
            format!("{}", max_d)
        } else {
            "?".to_string()
        };
        let conv_suffix = match wm.disentanglement_converged {
            Some(true) => " conv",
            Some(false) => " not conv",
            None => "",
        };
        lines.push(Line::from(format!(
            "dis.iter:      {}/{}{}",
            n_d, max_d_str, conv_suffix
        )));

        let d_iter_times: Vec<f64> = wm
            .disentanglement_block
            .as_ref()
            .map(|db| {
                db.cpu_time
                    .windows(2)
                    .map(|w| w[1] - w[0])
                    .filter(|&dt| dt.is_finite() && dt > 0.0)
                    .collect()
            })
            .unwrap_or_default();
        let d_scaled: Vec<f64> = d_iter_times.iter().map(|&v| v / divisor).collect();
        lines.push(Line::from(crate::ui::stats_line(
            &format!("time/D.it [{}]: ", time_unit),
            &d_scaled,
            2,
        )));
    }

    let max_w_str = if max_w > 0 {
        format!("{}", max_w)
    } else {
        "?".to_string()
    };
    lines.push(Line::from(format!("wann.iter:     {}/{}", n_w, max_w_str)));
    lines.push(Line::from(crate::ui::stats_line(
        &format!("time/W.it [{}]: ", time_unit),
        &w_scaled,
        2,
    )));
    lines.push(Line::from(format!(
        "time left [{}]: {:.2} ± {:.2}",
        time_unit, est_time_left_w, est_time_left_w_err
    )));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub fn render_spread_chart(frame: &mut Frame, area: Rect, wm: &WannierMetrics) {
    let points: Vec<(f64, f64)> = wm
        .spread_block
        .spread
        .iter()
        .enumerate()
        .map(|(i, &d)| (i as f64, d.abs().max(1e-30).log10()))
        .collect();

    ConvergenceChart::new(
        "Wannierisation Spread",
        "iter",
        "Spread (Ang^2)",
        vec![points],
        None,
    )
    .render(frame, area);
}

/// Build the left-panel tabs for wannier90 calculations.
pub fn build_left_tabs(wm: &WannierMetrics) -> Vec<(&'static str, Box<dyn Renderable>)> {
    let points = wm
        .disentanglement_block
        .as_ref()
        .map(|db| {
            db.delta_omega_i
                .iter()
                .enumerate()
                .map(|(i, &d)| (i as f64, d.abs().max(1e-30).log10()))
                .collect()
        })
        .unwrap_or_default();

    vec![(
        "ΔΩ Disentangle",
        Box::new(ConvergenceChart::new(
            "Disentanglement ΔΩ",
            "iter",
            "ΔΩ",
            vec![points],
            wm.disentanglement_conv_threshold,
        )),
    )]
}
