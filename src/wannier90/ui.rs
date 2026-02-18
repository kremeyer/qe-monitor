use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use ratatui::{Frame, layout::Rect, text::Line, widgets::Block};

use crate::wannier90::WannierMetrics;

pub fn render_summary(frame: &mut Frame, area: Rect, wm: &WannierMetrics) {
    let block = Block::bordered().title(Line::from(" Disentanglement ").bold().centered());

    let mut lines: Vec<Line> = Vec::new();

    let has_disentanglement_str = if wm.has_disentanglement { "Yes" } else { "No" };
    lines.push(Line::from(
        "Band Disentanglement: ".to_string() + has_disentanglement_str,
    ));
    let disentanglement_converged_str = match wm.disentanglement_converged {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "-",
    };
    lines.push(Line::from(
        "Disentanglement Converged: ".to_string() + disentanglement_converged_str,
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub fn render_subspace_disentanglement_chart(frame: &mut Frame, area: Rect, wm: &WannierMetrics) {
    let points: Vec<(f64, f64)> = wm
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

    crate::ui::render_convergence_chart(
        frame,
        area,
        "Disentanglement ΔΩ",
        "iter",
        "ΔΩ",
        vec![points],
        wm.disentanglement_conv_threshold,
    );
}

pub fn render_spread_chart(frame: &mut Frame, area: Rect, wm: &WannierMetrics) {
    let points: Vec<(f64, f64)> = wm
        .spread_block
        .delta_spread
        .iter()
        .enumerate()
        .map(|(i, &d)| (i as f64, d.abs().max(1e-30).log10()))
        .collect();

    crate::ui::render_convergence_chart(
        frame,
        area,
        "Wannierisation ΔΩ",
        "iter",
        "ΔΩ",
        vec![points],
        None,
    );
}
