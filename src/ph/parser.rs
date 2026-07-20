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
    let mut in_degeneracy_table = false;

    // (xq, n_irreps) for every q-point of the grid, from the up-front
    // "Number and degeneracy of irreps per q-point" table. This covers the whole
    // grid, which is more q-points than a run restricted by start_q/last_q will
    // touch, so it is only used as a lookup keyed on the q actually being computed.
    let mut qpoint_table: Vec<([f64; 3], u32)> = Vec::new();
    // Irreps of the q-points this run actually computes, in order.
    let mut computed_reps: Vec<u32> = Vec::new();
    // Fallback per-q irreps, from the "There are N irreducible representations"
    // lines that appear only as each q-point is reached (used when no table exists,
    // e.g. split runs).
    let mut there_are_reps: Vec<u32> = Vec::new();
    // Number of q-points this run has started, and the explicit run range when
    // QE reports one (it only does so with recover = .true.).
    let mut num_qpoints_started: u32 = 0;
    let mut range_total: Option<u32> = None;

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

        // Split run header:  "1 / 12 q-points for this run, from 2 to 2:"
        // The size of the [from, to] range is the only explicit statement of this
        // run's scope that QE ever prints, so prefer it when present.
        if let Some(total_qpoints) = parse_ph_qpoint_run_total(line) {
            range_total = Some(total_qpoints);
            in_degeneracy_table = false;
            continue;
        }

        // The "Number and degeneracy of irreps per q-point" table lists the
        // irrep count for *every* q-point of the run in its last column, so it
        // gives both the true total number of representations and the number of
        // q-points. Its header is the only q-point table header with an "N irreps"
        // column, which distinguishes it from the plain "uniform grid" listing
        // that shares the same "N   xq(1) xq(2) xq(3)" prefix.
        if line.starts_with("Number and degeneracy of irreps per q-point")
            || (line.starts_with("N ") && line.contains("xq(1)") && line.contains("N irreps"))
        {
            in_degeneracy_table = true;
            continue;
        }

        // Each q-point this run computes announces itself here. QE does not report
        // start_q/last_q (except via the recover range line above), so the q-points
        // that actually appear are the only reliable measure of the run's scope.
        // The irrep count is taken from the grid table rather than the later
        // "There are ..." line, so it is known as soon as the q-point starts.
        if line.starts_with("Calculation of q =") {
            in_degeneracy_table = false;
            num_qpoints_started += 1;
            if let Some(xq) = parse_calculation_of_q(line)
                && let Some(n_irreps) = lookup_irreps(&qpoint_table, xq)
            {
                computed_reps.push(n_irreps);
            }
            continue;
        }

        // A q-point is finished once its dynamical matrix / star is written.
        if line.starts_with("Number of q in the star") {
            pm.num_qpoints_completed += 1;
            continue;
        }

        if in_degeneracy_table {
            //   3   0.000000000   0.128300060   0.000000000      12
            // Trailing integer is the irrep count. Degeneracy sub-lines
            // ("1  1  2  2") are all integers and "No degeneracy" is text, so
            // neither parses as a row.
            if let Some(row) = parse_degeneracy_row(line) {
                qpoint_table.push(row);
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
            there_are_reps.push(n_irreps);
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

    // Scope the totals to the q-points this run actually touches. Irreps looked up
    // in the grid table are preferred (they are known as soon as a q-point starts);
    // the per-q "There are ..." lines are the fallback when no table was printed,
    // as in a recover run.
    pm.num_representations = if !computed_reps.is_empty() {
        computed_reps
    } else {
        there_are_reps
    };
    pm.num_qpoints = range_total.unwrap_or(0).max(num_qpoints_started);

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

/// The q of a "Calculation of q =    0.2213199  -0.3854415   0.1245717" line.
fn parse_calculation_of_q(line: &str) -> Option<[f64; 3]> {
    let mut fields = line.strip_prefix("Calculation of q =")?.split_whitespace();
    let mut xq = [0.0f64; 3];
    for c in xq.iter_mut() {
        *c = fields.next()?.parse().ok()?;
    }
    Some(xq)
}

/// Irrep count of the grid q-point matching `xq`. "Calculation of q" prints 7
/// decimals against the table's 9, so the comparison needs rounding slack.
fn lookup_irreps(table: &[([f64; 3], u32)], xq: [f64; 3]) -> Option<u32> {
    const TOL: f64 = 1e-6;
    table
        .iter()
        .find(|(q, _)| q.iter().zip(xq.iter()).all(|(a, b)| (a - b).abs() < TOL))
        .map(|&(_, n_irreps)| n_irreps)
}

/// A row of the "Number and degeneracy of irreps per q-point" table:
///   3   0.000000000   0.128300060   0.000000000      12
/// Returns the q coordinates and the trailing irrep count (12 here). Degeneracy
/// sub-lines (all integers) and "No degeneracy" text lines yield None.
fn parse_degeneracy_row(line: &str) -> Option<([f64; 3], u32)> {
    let mut fields = line.split_whitespace();
    let _n: u32 = fields.next()?.parse().ok()?;
    let mut xq = [0.0f64; 3];
    for c in xq.iter_mut() {
        let field = fields.next()?;
        if !field.contains('.') {
            return None;
        }
        *c = field.parse().ok()?;
    }
    let n_irreps = fields.next()?.parse::<u32>().ok()?;
    Some((xq, n_irreps))
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

    /// A uniform-grid run prints two tables sharing the same
    /// "N   xq(1) xq(2) xq(3)" header prefix: the plain grid listing and the
    /// "Number and degeneracy of irreps" table. Only the latter must be read, and
    /// only for the q-points the run actually reaches.
    const GRID_HEADER: &str = r#"
     Dynamical matrices for ( 2, 2, 1)  uniform grid of q-points
     (   3 q-points):
       N         xq(1)         xq(2)         xq(3)
       1   0.000000000   0.000000000   0.000000000
       2   0.000000000   0.500000000   0.000000000
       3   0.500000000   0.500000000   0.000000000

     Number and degeneracy of irreps per q-point
       N         xq(1)         xq(2)         xq(3)   N irreps
       1   0.000000000   0.000000000   0.000000000       8
        1   1   2   2   1   1   2   2
       2   0.000000000   0.500000000   0.000000000      12
     No degeneracy
       3   0.500000000   0.500000000   0.000000000       6
     No degeneracy

     Saving dvscf to file. Distribute only q points, not irreducible representations.
"#;

    #[test]
    fn scopes_totals_to_the_qpoints_the_run_reaches() {
        let output = format!(
            "{GRID_HEADER}
     Calculation of q =    0.0000000   0.0000000   0.0000000
     There are    8 irreducible representations
     End of self-consistent calculation
     Number of q in the star =    1

     Calculation of q =    0.0000000   0.5000000   0.0000000
     There are   12 irreducible representations
"
        );

        let metrics = parse_metrics(&output);
        // Two q-points started; q3 is not counted since the run hasn't reached it.
        assert_eq!(metrics.num_qpoints, 2);
        // Only the first has finished (one "Number of q in the star").
        assert_eq!(metrics.num_qpoints_completed, 1);
        assert_eq!(metrics.num_representations, vec![8, 12]);
    }

    #[test]
    fn start_q_run_reads_only_its_own_qpoint() {
        // With start_q = last_q = 2 and recover = .false., QE still prints the
        // whole grid but computes a single q-point, and reports no range line.
        // The irrep count comes from the grid table, so it is known before the
        // "There are ..." line is reached.
        let output = format!(
            "{GRID_HEADER}
     Calculation of q =    0.0000000   0.5000000   0.0000000
"
        );

        let metrics = parse_metrics(&output);
        assert_eq!(metrics.num_qpoints, 1);
        assert_eq!(metrics.num_qpoints_completed, 0);
        assert_eq!(metrics.num_representations, vec![12]);
    }

    #[test]
    fn parses_split_qpoint_header_and_irreps() {
        // A split run has no irreps-degeneracy table; the total is the size of
        // the [from, to] range and reps come from the "There are ..." lines.
        let output = r#"
     Saving dvscf to file. Distribute only q points, not irreducible representations.
        1 /  12 q-points for this run, from  2 to  2:
       N       xq(1)         xq(2)         xq(3)
       1   0.000000000   0.000000000   0.000000000
       2   0.000000000   0.000000000  -0.128314539

     Calculation of q =    0.0000000   0.0000000  -0.1283145

     There are   18 irreducible representations
"#;

        let metrics = parse_metrics(output);
        assert_eq!(metrics.num_qpoints, 1);
        assert_eq!(metrics.num_qpoints_completed, 0);
        assert_eq!(metrics.num_representations, vec![18]);
    }

    #[test]
    fn single_qpoint_run_reads_irreps_from_table() {
        // A single-q run has only the irreps table (no grid or split header).
        let output = r#"
     Number and degeneracy of irreps per q-point
       N         xq(1)         xq(2)         xq(3)   N irreps
       1   0.000000000   0.000000000   0.000000000      36
     No degeneracy

     Saving dvscf to file. Distribute only q points, not irreducible representations.

     Calculation of q =    0.0000000   0.0000000   0.0000000

     There are   36 irreducible representations
"#;

        let metrics = parse_metrics(output);
        assert_eq!(metrics.num_qpoints, 1);
        assert_eq!(metrics.num_qpoints_completed, 0);
        assert_eq!(metrics.num_representations, vec![36]);
    }
}
