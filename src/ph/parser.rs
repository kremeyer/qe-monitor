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
    let mut split_qpoint_run = false;

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
        if line.starts_with("N         xq(1)         xq(2)         xq(3)") {
            in_qpoint_list = true;
            continue;
        }

        if let Some(total_qpoints) = parse_ph_qpoint_run_total(line) {
            split_qpoint_run = true;
            pm.num_qpoints = pm.num_qpoints.max(total_qpoints);
            continue;
        }

        if in_qpoint_list {
            if line.starts_with("Calculation of q =") {
                if !split_qpoint_run {
                    pm.num_qpoints_completed += 1;
                }
                in_qpoint_list = false;
                continue;
            }
            // Q-point/irreps data lines look like:
            //   1   0.000000000   0.000000000   0.000000000       4
            // They start with a digit and contain decimal points (the xq coords).
            // Degeneracy lines like "1   2   1   2" are all integers (no dots).
            if !split_qpoint_run
                && line.bytes().next().is_some_and(|b| b.is_ascii_digit())
                && line.contains('.')
            {
                pm.num_qpoints += 1;
            }
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

        if let Some(n_irreps) = parse_ph_irreps_count(line) {
            pm.num_representations.push(n_irreps);
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

fn parse_ph_qpoint_run_total(line: &str) -> Option<u32> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?i)(\d+)\s*/\s*(\d+)\s+q-points\s+for\s+this\s+run,\s+from\s+(\d+)\s+to\s+(\d+)",
        )
        .unwrap()
    });

    let from = cap_u32(&RE, line, 3)?;
    let to = cap_u32(&RE, line, 4)?;
    Some(to.saturating_sub(from).saturating_add(1))
}

fn parse_ph_irreps_count(line: &str) -> Option<u32> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)There\s+are\s+(\d+)\s+irreducible\s+representations").unwrap()
    });

    cap_u32(&RE, line, 1)
}

fn parse_phonon_cpu_time(line: &str) -> Option<f64> {
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"PHONON\s+:\s+([0-9]+(?:\.[0-9]+)?)s\s+CPU").unwrap());
    cap_f64(&RE, line, 1)
}

#[cfg(test)]
mod tests {
    use super::parse_metrics;

    #[test]
    fn parses_split_qpoint_header_and_irreps() {
        let output = r#"
     Saving dvscf to file. Distribute only q points, not irreducible representations.
        1 /  12 q-points for this run, from  2 to  2:
       N       xq(1)         xq(2)         xq(3)
       1   0.000000000   0.000000000   0.000000000
       2   0.000000000   0.000000000  -0.128314539

     Calculation of q =    0.0000000   0.0000000  -0.1283145

     Number of q in the star: 1

     There are   18 irreducible representations
"#;

        let metrics = parse_metrics(output);
        assert_eq!(metrics.num_qpoints, 1);
        assert_eq!(metrics.num_qpoints_completed, 0);
        assert_eq!(metrics.num_representations, vec![18]);
    }

    #[test]
    fn parses_unsplit_qpoint_table_rows() {
        let output = r#"
     N         xq(1)         xq(2)         xq(3)   N irreps
       1   0.000000000   0.000000000   0.000000000       4
       2   0.000000000   0.000000000  -0.128314539       4

     Calculation of q =    0.0000000   0.0000000  -0.1283145

     Number of q in the star: 2

     There are    4 irreducible representations
"#;

        let metrics = parse_metrics(output);
        assert_eq!(metrics.num_qpoints, 2);
        assert_eq!(metrics.num_qpoints_completed, 1);
        assert_eq!(metrics.num_representations, vec![4]);
    }
}
