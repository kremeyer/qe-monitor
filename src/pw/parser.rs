use crate::app::RunInfo;
use crate::pw::metrics::{BandBlock, PwCalcType, PwMetrics, ScfBlock};

use once_cell::sync::Lazy;
use regex::Regex;

/// Detect calculation type by scanning for key markers
fn detect_calc_type(qe_output: &str) -> PwCalcType {
    // nscf and bands will have
    if qe_output.contains("Band Structure Calculation") {
        return PwCalcType::Nscf;
    }

    // Default to SCF for everything else
    PwCalcType::Scf
}

// ==================================================
// Run info parsing (common to all calculation types)
// ==================================================

pub fn parse_run_info(qe_output: &str) -> RunInfo {
    let mut run_info = RunInfo::default();

    for raw in qe_output.lines().take(50) {
        let line = raw.trim_start();

        if line.starts_with("Program ") && line.contains(" starts on ") {
            // executable
            if let Some(rest) = line.strip_prefix("Program ") {
                match rest.split_whitespace().next().map(|s| s.to_string()) {
                    Some(s) if s.contains("PWSCF") => {
                        run_info.executable = Some("pw.x".to_string())
                    }
                    Some(s) if s.contains("PHONON") => {
                        run_info.executable = Some("ph.x".to_string())
                    }
                    _ => {}
                }
            }

            // version
            if let Some(pos) = line.find("v.") {
                let after_v = &line[(pos + 2)..];
                run_info.qe_version = after_v.split_whitespace().next().map(|s| s.to_string());
            }

            // start time
            if let Some(pos) = line.find(" starts on ") {
                let after = line[(pos + " starts on ".len())..]
                    .trim()
                    .replace(": ", ":0")
                    .replace(" at ", " ");
                run_info.start_time = Some(after);
            }
        }

        if line.contains("running on") && line.contains("processors") {
            // only compiled with MPI, no openMP
            if let Some(pos) = line.find("running on") {
                let after = line[(pos + "running on".len())..].trim();
                run_info.mpi_ranks = after.split_whitespace().next().map(|s| s.to_string());
                run_info.omp_threads = Some("1".to_string());
            }
        }
        if line.contains("Number of MPI processes:")
            && let Some(pos) = line.find("Number of MPI processes:")
        {
            let after = line[(pos + "Number of MPI processes:".len())..].trim();
            run_info.mpi_ranks = after.split_whitespace().next().map(|s| s.to_string());
        }
        if line.contains("Threads/MPI process:")
            && let Some(pos) = line.find("Threads/MPI process:")
        {
            let after = line[(pos + "Threads/MPI process:".len())..].trim();
            run_info.omp_threads = after.split_whitespace().next().map(|s| s.to_string());
        }
    }

    run_info
}

pub fn parse_metrics(qe_output: &str) -> PwMetrics {
    let calc_type = detect_calc_type(qe_output);

    match calc_type {
        PwCalcType::Scf => parse_metrics_scf(qe_output),
        PwCalcType::Nscf => parse_metrics_nscf(qe_output),
    }
}

// =======================
// SCF calculation parsing
// =======================

fn parse_metrics_scf(qe_output: &str) -> PwMetrics {
    let mut pm = PwMetrics {
        calc_type: PwCalcType::Scf,
        ..Default::default()
    };

    let mut open: Option<ScfBlock> = None;
    let mut cur_iter: Option<u32> = None;

    for raw in qe_output.lines() {
        let line = raw.trim_start();

        // parse convergence threshold from header
        if line.contains("scf convergence threshold =")
            && let Some(pos) = line.find("scf convergence threshold =")
        {
            let after = line[(pos + "scf convergence threshold =".len())..].trim();
            pm.conv_threshold = after
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<f64>().ok());
        }

        // cpu time line: update open block if present
        if let Some(t) = parse_total_cpu_secs(line) {
            if let Some(b) = open.as_mut() {
                if b.cpu_time_first.is_none() {
                    b.cpu_time_first = Some(t);
                }
                b.cpu_time_last = Some(t);
            }
            continue;
        }

        // start a new block
        if line.contains("Self-consistent Calculation") {
            // close previous block
            if let Some(b) = open.take()
                && (!b.accuracy.is_empty()
                    || b.conv_iters.is_some()
                    || b.cpu_time_first.is_some()
                    || b.cpu_time_last.is_some())
            {
                pm.scf_blocks.push(b);
            }

            // open new block, seeded from pending cpu time
            let b = ScfBlock::default();
            open = Some(b);
            cur_iter = None;
            continue;
        }

        // explicit block end (if present)
        if line.contains("End of self-consistent calculation") {
            if let Some(mut b) = open.take() {
                // The last converged iteration has no "estimated scf accuracy"
                // line, so cur_iter may be ahead of what's recorded in b.iteration.
                if b.conv_iters.is_none() {
                    b.conv_iters = cur_iter;
                }
                if !b.accuracy.is_empty()
                    || b.conv_iters.is_some()
                    || b.cpu_time_first.is_some()
                    || b.cpu_time_last.is_some()
                {
                    pm.scf_blocks.push(b);
                }
            }
            cur_iter = None;
            continue;
        }

        // "convergence has been achieved in N iterations" appears after
        // "End of self-consistent calculation" in pw.x, so handle it
        // even when no block is open by updating the last pushed block.
        if let Some(n) = parse_scf_convergence_iterations(line) {
            if let Some(b) = open.as_mut() {
                b.conv_iters = Some(n);
            } else if let Some(b) = pm.scf_blocks.last_mut() {
                b.conv_iters = Some(n);
            }
            continue;
        }

        // only parse SCF-specific lines if a block is open
        if let Some(b) = open.as_mut() {
            if let Some(iter) = parse_scf_iteration_index(line) {
                cur_iter = Some(iter);
                continue;
            }
            if let Some(acc) = parse_scf_accuracy(line) {
                let iter = cur_iter.unwrap_or((b.accuracy.len() as u32) + 1);
                b.iteration.push(iter);
                b.accuracy.push(acc);
                continue;
            }
        }

        // other metrics
        if let Some(v) = parse_total_force(line) {
            pm.total_force.push(v);
            continue;
        }
        if let Some(v) = parse_total_energy(line) {
            pm.total_energy.push(v);
            continue;
        }
        if let Some(v) = parse_pressure_kbar(line) {
            pm.pressure.push(v);
            continue;
        }
    }

    // EOF closes any open block
    if let Some(b) = open.take()
        && (!b.accuracy.is_empty()
            || b.conv_iters.is_some()
            || b.cpu_time_first.is_some()
            || b.cpu_time_last.is_some())
    {
        pm.scf_blocks.push(b);
    }

    pm
}

// ========================
// NSCF calculation parsing
// ========================

fn parse_metrics_nscf(qe_output: &str) -> PwMetrics {
    let mut pm = PwMetrics {
        calc_type: PwCalcType::Nscf,
        ..Default::default()
    };

    let mut band_block: Option<BandBlock> = None;

    for raw in qe_output.lines() {
        let line = raw.trim_start();

        // Detect band structure calculation start
        if line.contains("Band Structure Calculation") {
            band_block = Some(BandBlock::default());
            continue;
        }

        // Parse k-point progress (only when band_block exists)
        if let Some(b) = band_block.as_mut()
            && let Some((current_kpt, total_kpts)) = parse_band_kpt_progress(line)
        {
            b.kpt_number.push(current_kpt);
            b.num_kpts = Some(total_kpts);
            continue;
        }

        // CPU time tracking for band calculations
        if let Some(t) = parse_total_cpu_secs(line) {
            if let Some(b) = band_block.as_mut() {
                if b.cpu_time_first.is_none() {
                    b.cpu_time_first = Some(t);
                }
                b.cpu_time_last = Some(t);
                b.cpu_time.push(t);
            }
            continue;
        }
    }

    // EOF closes any open band block
    if let Some(b) = band_block.take()
        && !b.kpt_number.is_empty()
    {
        pm.band_blocks.push(b);
    }

    pm
}

// ============================
// Helper functions for parsing
// ============================

pub fn cap_f64(re: &Regex, line: &str, idx: usize) -> Option<f64> {
    let caps = re.captures(line)?;
    let s = caps.get(idx)?.as_str().replace(['D', 'd'], "E");
    s.parse::<f64>().ok()
}

pub fn cap_u32(re: &Regex, line: &str, idx: usize) -> Option<u32> {
    let caps = re.captures(line)?;
    caps.get(idx)?.as_str().trim().parse::<u32>().ok()
}

fn parse_total_force(line: &str) -> Option<f64> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"Total force\s*=\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[EeDd][+-]?\d+)?)").unwrap()
    });
    cap_f64(&RE, line, 1)
}

fn parse_total_energy(line: &str) -> Option<f64> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"!\s+total energy\s*=\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[EeDd][+-]?\d+)?)").unwrap()
    });
    cap_f64(&RE, line, 1)
}

fn parse_pressure_kbar(line: &str) -> Option<f64> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\(kbar\).*\bP=\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[EeDd][+-]?\d+)?)").unwrap()
    });
    cap_f64(&RE, line, 1)
}

fn parse_scf_accuracy(line: &str) -> Option<f64> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"estimated scf accuracy\s*<\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[EeDd][+-]?\d+)?)")
            .unwrap()
    });
    cap_f64(&RE, line, 1)
}

fn parse_scf_iteration_index(line: &str) -> Option<u32> {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"iteration\s*#\s*(\d+)").unwrap());
    let caps = RE.captures(line)?;
    caps.get(1)?.as_str().parse::<u32>().ok()
}

fn parse_scf_convergence_iterations(line: &str) -> Option<u32> {
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"convergence has been achieved in\s+(\d+)\s+iterations").unwrap());
    cap_u32(&RE, line, 1)
}

fn parse_total_cpu_secs(line: &str) -> Option<f64> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"total cpu time spent up to now is\s*([0-9]+(?:\.[0-9]+)?)\s*secs").unwrap()
    });
    cap_f64(&RE, line, 1)
}

fn parse_band_kpt_progress(line: &str) -> Option<(u32, u32)> {
    // Returns (current_kpt, total_kpts) from "Computing kpt #: X of Y"
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"Computing kpt #:\s*(\d+)\s*of\s*(\d+)").unwrap());
    let caps = RE.captures(line)?;
    let current = caps.get(1)?.as_str().parse::<u32>().ok()?;
    let total = caps.get(2)?.as_str().parse::<u32>().ok()?;
    Some((current, total))
}
