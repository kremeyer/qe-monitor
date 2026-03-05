use ratatui::style::Stylize;
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Bar, BarChart, BarGroup, Block, Paragraph},
};

use crate::ph::PhMetrics;

pub fn render_phonon_summary(frame: &mut Frame, area: Rect, pm: &PhMetrics) {
    let block = Block::bordered().title(Line::from(" Phonon stats ").bold().centered());

    let total_reps = pm.num_representations.iter().sum::<u32>();
    let completed = pm.num_representations_completed as usize;

    // Only sum completed blocks (exclude in-progress last block)
    let completed_blocks =
        &pm.representation_blocks[..completed.min(pm.representation_blocks.len())];
    let total_time: f64 = completed_blocks
        .iter()
        .filter_map(|b| b.time_per_calculation())
        .filter(|&v| v.is_finite() && v >= 0.0)
        .sum::<f64>();

    // Average time per representation
    let time_per_rep = if completed > 0 {
        total_time / completed as f64
    } else {
        0.0
    };

    let mut est_time_left = f64::NAN;
    if pm.num_representations_completed >= total_reps {
        est_time_left = 0.0;
    } else if completed > 0 && time_per_rep.is_finite() {
        let reps_left = (total_reps - pm.num_representations_completed) as f64;
        est_time_left = time_per_rep * reps_left;

        // Subtract time already spent on current in-progress representation
        if pm.representation_blocks.len() > completed {
            let current_rep_time = pm.representation_blocks[completed]
                .time_per_calculation()
                .unwrap_or(0.0);
            if current_rep_time.is_finite() && current_rep_time > 0.0 {
                est_time_left -= current_rep_time;
                est_time_left = est_time_left.max(0.0);
            }
        }
    }

    // Pick a common time unit based on the larger value
    let max_val = if est_time_left.is_finite() {
        est_time_left.max(time_per_rep)
    } else {
        time_per_rep
    };
    let (time_unit, divisor) = if max_val >= 36000.0 {
        ("h", 3600.0)
    } else if max_val >= 600.0 {
        ("m", 60.0)
    } else {
        ("s", 1.0)
    };
    est_time_left /= divisor;

    // Collect iteration counts for completed blocks
    let iters: Vec<f64> = completed_blocks
        .iter()
        .filter_map(|b| b.iterations_to_converge().map(|v| v as f64))
        .filter(|&v| v > 0.0)
        .collect();

    // Collect time per representation for completed blocks
    let sec_per_rep: Vec<f64> = completed_blocks
        .iter()
        .filter_map(|b| b.time_per_calculation())
        .filter(|&v| v.is_finite() && v >= 0.0)
        .collect();

    // Convert time to the right unit
    let sec_per_rep_scaled: Vec<f64> = sec_per_rep.iter().map(|&v| v / divisor).collect();

    // Calculate std deviation for time per representation
    let (_, std_per_rep) = if !sec_per_rep_scaled.is_empty() {
        crate::ui::mean_std(&sec_per_rep_scaled)
    } else {
        (0.0, 0.0)
    };

    // Propagate error to time left estimate
    let reps_left = (total_reps.saturating_sub(pm.num_representations_completed)) as f64;
    let est_time_left_error = std_per_rep * reps_left;

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(format!(
        "NQ:            {}/{}",
        pm.num_qpoints_completed, pm.num_qpoints
    )));
    lines.push(Line::from(format!(
        "NR:            {}/{}",
        pm.num_representations_completed, total_reps
    )));
    lines.push(Line::from(crate::ui::stats_line(
        "iter/R:       ",
        &iters,
        1,
    )));
    lines.push(Line::from(crate::ui::stats_line(
        &format!("time/R [{}]:   ", time_unit),
        &sec_per_rep_scaled,
        2,
    )));
    lines.push(Line::from(format!(
        "time left [{}]: {:.2} ± {:.2}",
        time_unit, est_time_left, est_time_left_error
    )));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub fn render_representation_iterations_chart(
    frame: &mut Frame,
    area: Rect,
    pm: &PhMetrics,
    _scroll_offset: usize,
) {
    let n_iters: Vec<u32> = pm
        .representation_blocks
        .iter()
        .filter_map(|b| b.iterations_to_converge())
        .collect();

    if n_iters.is_empty() {
        let block =
            Block::bordered().title(Line::from(" Representation Iterations ").bold().centered());
        frame.render_widget(block, area);
        return;
    }

    // Calculate how many bars fit in the available area (each bar takes 1 row)
    let content_height = area.height.saturating_sub(2) as usize; // minus borders
    let total_bars = n_iters.len();

    // Show the last bars that fit in the viewport
    let start = total_bars.saturating_sub(content_height);
    let visible_iters = &n_iters[start..];

    let n_dig_i = format!("{}", pm.num_representations_completed).len();
    let n_dig_v = n_iters
        .iter()
        .map(|&v| format!("{}", v).len())
        .max()
        .unwrap_or(1);

    let bars: Vec<Bar> = visible_iters
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let actual_index = start + i;
            Bar::default()
                .value(u64::from(v))
                .label(Line::from(format!("{:n_dig_i$}", actual_index + 1)))
                .text_value(format!("{:n_dig_v$}", v))
        })
        .collect();

    let bar_chart = BarChart::default()
        .block(Block::bordered().title(Line::from(" Representation Iterations ").bold().centered()))
        .data(BarGroup::default().bars(&bars))
        .bar_gap(0)
        .bar_width(1)
        .direction(ratatui::layout::Direction::Horizontal);
    frame.render_widget(bar_chart, area);
}

pub fn render_scf_accuracy_chart(frame: &mut Frame, area: Rect, ph: &PhMetrics) {
    let representation_blocks = &ph.representation_blocks;
    if representation_blocks.is_empty() {
        let block = Block::bordered().title(Line::from(" SCF Accuracy ").bold().centered());
        frame.render_widget(block, area);
        return;
    }

    // Take last 3 blocks to reduce clutter
    let mut last3_blocks: Vec<&crate::pw::ScfBlock> =
        representation_blocks.iter().rev().take(3).collect();
    last3_blocks.reverse();

    // Build points for each block
    let mut all_points: Vec<Vec<(f64, f64)>> = Vec::new();
    for block in &last3_blocks {
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
        ph.conv_threshold,
    );
}
