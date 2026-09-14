use ratatui::style::Stylize;
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Bar, BarChart, BarGroup, Block, Paragraph},
};

use crate::ui::{ConvergenceChart, Renderable};
use crate::wannier90::WannierMetrics;

/// Helper: convert a value series into `(iteration, log10(|value|))` points.
fn to_log_points(values: &[f64]) -> Vec<(f64, f64)> {
    values
        .iter()
        .enumerate()
        .map(|(i, &d)| (i as f64, d.abs().max(1e-30).log10()))
        .collect()
}

/// Pick a time unit and its divisor from the largest value that will be shown
/// with it, so a long ETA is not printed as five figures of seconds.
fn time_unit(largest: f64) -> (&'static str, f64) {
    if largest >= 36000.0 {
        ("h", 3600.0)
    } else if largest >= 600.0 {
        ("m", 60.0)
    } else {
        ("s", 1.0)
    }
}

pub fn render_summary(frame: &mut Frame, area: Rect, wm: &WannierMetrics) {
    let block = Block::bordered().title(Line::from(" Wannierisation ").bold().centered());

    let mut lines: Vec<Line> = Vec::new();

    // Wannierization: iter 0 is the initial state, not a real iteration
    let n_w = wm.spread_block.spread.len().saturating_sub(1);
    let max_w = wm.wannierize_max_iterations.unwrap_or(0);

    let w_iter_times = wm.spread_block.iter_times();
    let d_iter_times = wm
        .disentanglement_block
        .as_ref()
        .map(|db| db.iter_times())
        .unwrap_or_default();

    let n_d = wm
        .disentanglement_block
        .as_ref()
        .map(|db| db.cpu_time.len())
        .unwrap_or(0);
    let max_d = wm.dis_max_iterations.unwrap_or(0);

    let eta_w = crate::wannier90::eta_seconds(&w_iter_times, n_w, max_w);
    let eta_d = crate::wannier90::eta_seconds(&d_iter_times, n_d, max_d);

    // While disentanglement is still the only phase with timings, report its own
    // ETA rather than nothing at all - but flag that wannierisation still follows,
    // so the number is not mistaken for the time left in the whole run.
    let (eta, pending_wann) = match (eta_w, eta_d) {
        (None, Some(d)) if wm.has_disentanglement => (Some(d), true),
        (w, _) => (w, false),
    };

    // The two per-iteration lines share a unit so the phases stay comparable; the
    // ETA gets its own, since it is orders of magnitude larger.
    let slowest_iter = d_iter_times
        .iter()
        .chain(w_iter_times.iter())
        .copied()
        .fold(0.0, f64::max);
    let (it_unit, it_div) = time_unit(slowest_iter);
    let (eta_unit, eta_div) = time_unit(eta.map(|(v, _)| v).unwrap_or(0.0));

    let scaled = |xs: &[f64]| -> Vec<f64> { xs.iter().map(|&v| v / it_div).collect() };

    if wm.has_disentanglement {
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
        lines.push(Line::from(crate::ui::stats_line(
            &format!("time/D.it [{}]: ", it_unit),
            &scaled(&d_iter_times),
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
        &format!("time/W.it [{}]: ", it_unit),
        &scaled(&w_iter_times),
        2,
    )));

    lines.push(match eta {
        Some((est, err)) => {
            let mut spans = vec![ratatui::text::Span::raw(format!(
                "time left [{}]: {:.2} ± {:.2}",
                eta_unit,
                est / eta_div,
                err / eta_div
            ))];
            if pending_wann {
                spans.push(
                    ratatui::text::Span::raw(" + WANN")
                        .style(ratatui::style::Style::default().dim()),
                );
            }
            Line::from(spans)
        }
        None => Line::from(format!("time left [{}]: -", eta_unit)),
    });

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Bar chart of the per-Wannier-function spreads of the last complete iteration.
struct WfSpreadBarChart {
    spreads: Vec<f64>,
}

impl Renderable for WfSpreadBarChart {
    fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered().title(Line::from(" WF Spreads (Ang²) ").bold().centered());

        if self.spreads.is_empty() {
            frame.render_widget(block, area);
            return;
        }

        let n_dig = format!("{}", self.spreads.len()).len();
        let bars: Vec<Bar> = self
            .spreads
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                Bar::default()
                    // scale to integer length; keeps relative bar heights
                    .value((s.max(0.0) * 1000.0) as u64)
                    .label(Line::from(format!("{:n_dig$}", i + 1)))
                    .text_value(format!("{s:.3}"))
            })
            .collect();

        let bar_chart = BarChart::default()
            .block(block)
            .data(BarGroup::default().bars(&bars))
            .bar_gap(0)
            .bar_width(1)
            .direction(ratatui::layout::Direction::Horizontal);
        frame.render_widget(bar_chart, area);
    }
}

/// Build the full set of wannier90 plots. Both main panels offer this same set,
/// so the user can show any plot on the left (F-keys) and any on the right (numbers).
///
/// With disentanglement: Disent. abs / Disent. Δ / Spread abs / Spread Δ / WF spreads.
/// Without disentanglement, the two disentanglement tabs are omitted.
pub fn build_tabs(wm: &WannierMetrics) -> Vec<(&'static str, Box<dyn Renderable>)> {
    let mut tabs: Vec<(&'static str, Box<dyn Renderable>)> = Vec::new();

    if let Some(db) = wm.disentanglement_block.as_ref() {
        tabs.push((
            "Disent. abs",
            Box::new(ConvergenceChart::new(
                "Disentanglement Ω_I",
                "iter",
                "Ω_I",
                vec![to_log_points(&db.omega_i)],
                None,
            )),
        ));
        tabs.push((
            "Disent. Δ",
            Box::new(ConvergenceChart::new(
                "Disentanglement ΔΩ",
                "iter",
                "ΔΩ",
                vec![to_log_points(&db.delta_omega_i)],
                wm.disentanglement_conv_threshold,
            )),
        ));
    }

    tabs.push((
        "Spread abs",
        Box::new(ConvergenceChart::new(
            "Wannierisation Spread",
            "iter",
            "Spread (Ang^2)",
            vec![to_log_points(&wm.spread_block.spread)],
            None,
        )),
    ));

    let wann_thr = if wm.wannierize_conv_threshold > 0.0 {
        Some(wm.wannierize_conv_threshold)
    } else {
        None
    };
    tabs.push((
        "Spread Δ",
        Box::new(ConvergenceChart::new(
            "Wannierisation ΔΩ",
            "iter",
            "ΔΩ",
            vec![to_log_points(&wm.spread_block.delta_spread)],
            wann_thr,
        )),
    ));

    tabs.push((
        "WF spreads",
        Box::new(WfSpreadBarChart {
            spreads: wm.spread_block.wf_spreads_last.clone(),
        }),
    ));

    tabs
}
