// Re-export pw's parse_run_info since it works identically for PHONON
pub use crate::pw::parse_run_info;

use crate::ph::metrics::PhMetrics;
use crate::pw::ScfBlock as RepresentationBlock;
use crate::pw::{cap_f64, cap_u32};

use once_cell::sync::Lazy;
use regex::Regex;

pub fn parse_metrics(qe_output: &str) -> PhMetrics {
    let mut pm = PhMetrics::default();

    // for header before calculation
    let mut in_qpoint_list = false;

    // for representation-level block
    let mut open_repr_block: Option<RepresentationBlock> = None;
    let mut cur_iter: Option<u32> = None;
    let mut pending_cpu_time: Option<f64> = None;

    for raw in qe_output.lines() {
        let line = raw.trim_start();

        // parse convergence threshold from header
        if line.contains("convergence threshold     =")
            && let Some(pos) = line.find("convergence threshold     =")
        {
            let after = line[(pos + "convergence threshold     =".len())..].trim();
            pm.conv_threshold = after
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<f64>().ok());
        }

        // Parse PHONON timer line to seed pending_cpu_time before the first representation
        if let Some(t) = parse_phonon_cpu_time(line) {
            pending_cpu_time = Some(t);
            continue;
        }

        // header before calculation
        if line.starts_with("N         xq(1)         xq(2)         xq(3)   N irreps") {
            in_qpoint_list = true;
            pm.num_qpoints = 0;
            pm.num_representations.clear();
            continue;
        }

        if in_qpoint_list {
            if line.starts_with("Calculation of q =") {
                in_qpoint_list = false;
                continue;
            }
            // Q-point/irreps data lines look like:
            //   1   0.000000000   0.000000000   0.000000000       4
            // They start with a digit and contain decimal points (the xq coords).
            // Degeneracy lines like "1   2   1   2" are all integers (no dots).
            if line.bytes().next().is_some_and(|b| b.is_ascii_digit())
                && line.contains('.')
                && let Some(n_irreps) = line
                    .split_whitespace()
                    .last()
                    .and_then(|s| s.parse::<u32>().ok())
            {
                pm.num_qpoints += 1;
                pm.num_representations.push(n_irreps);
            }
            continue;
        }

        if line.starts_with("Number of q in the star") {
            pm.num_qpoints_completed += 1;
            continue;
        }

        // representation-level block parsing
        if line.contains("Self-consistent Calculation") {
            // close previous block
            if let Some(b) = open_repr_block.take()
                && (!b.accuracy.is_empty()
                    || b.conv_iters.is_some()
                    || b.cpu_time_first.is_some()
                    || b.cpu_time_last.is_some())
            {
                pm.representation_blocks.push(b);
            }

            // open new block
            let b = RepresentationBlock::default();
            open_repr_block = Some(b);
            cur_iter = None;
            continue;
        }

        if line.contains("iter #") && line.contains("av.it.") {
            if let Some((iter, t)) = parse_ph_iteration_and_cpu(line) {
                cur_iter = Some(iter);
                pending_cpu_time = Some(t);
                if let Some(b) = open_repr_block.as_mut() {
                    b.cpu_time_first.get_or_insert(t);
                    b.cpu_time_last = Some(t);
                }
            }
            continue;
        }

        if line.contains("thresh") && line.contains("|ddv_scf|^2") {
            if let Some(b) = open_repr_block.as_mut()
                && let Some(ddv_scf2) = parse_ph_ddv_scf2(line)
            {
                let iter = cur_iter.unwrap_or((b.accuracy.len() as u32) + 1);
                b.iteration.push(iter);
                b.accuracy.push(ddv_scf2);
            }
            continue;
        }

        if line.contains("End of self-consistent calculation") {
            pm.num_representations_completed += 1;

            if let Some(mut b) = open_repr_block.take() {
                if b.conv_iters.is_none() {
                    b.conv_iters = cur_iter.or_else(|| b.iteration.iter().copied().max());
                }

                if !b.accuracy.is_empty()
                    || b.conv_iters.is_some()
                    || b.cpu_time_first.is_some()
                    || b.cpu_time_last.is_some()
                {
                    pm.representation_blocks.push(b);
                }
            }

            cur_iter = None;
            continue;
        }
    }

    if let Some(mut b) = open_repr_block.take() {
        if b.cpu_time_first.is_none() {
            b.cpu_time_first = pending_cpu_time;
        }
        if b.cpu_time_last.is_none() {
            b.cpu_time_last = pending_cpu_time;
        }
        if b.conv_iters.is_none() {
            b.conv_iters = cur_iter.or_else(|| b.iteration.iter().copied().max());
        }

        if !b.accuracy.is_empty()
            || b.conv_iters.is_some()
            || b.cpu_time_first.is_some()
            || b.cpu_time_last.is_some()
        {
            pm.representation_blocks.push(b);
        }
    }

    pm
}

fn parse_ph_iteration_and_cpu(line: &str) -> Option<(u32, f64)> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"iter\s*#\s*(\d+)\s+total\s+cpu\s+time\s*:\s*([0-9]+(?:\.[0-9]+)?)\s*secs")
            .unwrap()
    });
    let iter = cap_u32(&RE, line, 1)?;
    let t = cap_f64(&RE, line, 2)?;
    Some((iter, t))
}

fn parse_ph_ddv_scf2(line: &str) -> Option<f64> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\|ddv_scf\|\^2\s*=\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[EeDd][+-]?\d+)?)").unwrap()
    });
    cap_f64(&RE, line, 1)
}

fn parse_phonon_cpu_time(line: &str) -> Option<f64> {
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"PHONON\s+:\s+([0-9]+(?:\.[0-9]+)?)s\s+CPU").unwrap());
    cap_f64(&RE, line, 1)
}
