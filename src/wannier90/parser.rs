use crate::app::RunInfo;
use crate::wannier90::{DisentanglementBlock, WannierMetrics};
use crate::{WANN_MARKER, WANN_RESUME_MARKER};

const WANN_BANNER_TITLE: &str = "WANNIER90";

/// how far above the welcome line the banner title may sit for us to still count
const BANNER_LOOKBEHIND: usize = 512;

/// Restrict `output` to the most recent wannier90 run.
///
/// EPW drives wannier90 in library mode and appends to an existing `.wout`, so
/// one file can hold several runs back to back.
fn last_run(output: &str) -> &str {
    let Some(welcome) = output.rfind(WANN_MARKER) else {
        return match output.rfind(WANN_RESUME_MARKER) {
            Some(resume) => &output[line_start(output, resume)..],
            None => output,
        };
    };
    let welcome_line = line_start(output, welcome);

    // Back up to the banner title so parse_run_info still sees it.
    let start = output[..welcome_line]
        .rfind(WANN_BANNER_TITLE)
        .filter(|&title| welcome_line - title <= BANNER_LOOKBEHIND)
        .map(|title| line_start(output, title))
        .unwrap_or(welcome_line);

    &output[start..]
}

/// Byte offset of the start of the line containing `pos`.
fn line_start(s: &str, pos: usize) -> usize {
    s[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

pub fn parse_run_info(wannier90_output: &str) -> RunInfo {
    let mut run_info = RunInfo::default();
    let wannier90_output = last_run(wannier90_output);

    for raw in wannier90_output.lines().take(100) {
        let line = raw.trim_start();

        if run_info.executable.is_none()
            && (line.contains(WANN_BANNER_TITLE) || line.contains(WANN_RESUME_MARKER))
        {
            run_info.executable = Some("Wannier90.x".to_string());
        }

        if run_info.version.is_none()
            && let Some(pos) = line.find("Release:")
        {
            run_info.version = line[(pos + "Release:".len())..]
                .split_whitespace()
                .next()
                .map(|s| s.to_string());
        }

        if run_info.start_time.is_none()
            && let Some(pos) = line.find("Execution started on")
        {
            let after = line[(pos + "Execution started on".len())..]
                .trim()
                .trim_matches('|')
                .trim()
                .replace(" at ", " ");
            run_info.start_time = Some(after);
        }

        // A banner-less run has no "Execution started on" line; its resume line
        // carries the only timestamp. Checked second so the banner still wins.
        if run_info.start_time.is_none()
            && let Some(pos) = line.find(WANN_RESUME_MARKER)
        {
            let after = line[(pos + WANN_RESUME_MARKER.len())..]
                .trim()
                .trim_start_matches("at")
                .trim();
            if !after.is_empty() {
                run_info.start_time = Some(after.to_string());
            }
        }

        if run_info.mpi_ranks.is_none() {
            if line.contains("Running in serial") {
                run_info.mpi_ranks = Some("1".to_string());
            } else if let Some(pos) = line.find("Running in parallel on") {
                run_info.mpi_ranks = line[(pos + "Running in parallel on".len())..]
                    .split_whitespace()
                    .next()
                    .map(|s| s.to_string());
            }
        }

        // All header fields filled - no need to scan further
        if run_info.executable.is_some()
            && run_info.version.is_some()
            && run_info.start_time.is_some()
            && run_info.mpi_ranks.is_some()
        {
            break;
        }
    }

    // Wannier90 only implicitly uses OpenMP for some low-level calls (e.g. BLAS)
    run_info.omp_threads = Some("-".to_string());

    run_info
}

pub fn parse_metrics(wannier90_output: &str) -> WannierMetrics {
    let wannier90_output = last_run(wannier90_output);
    let mut wm = WannierMetrics::default();
    let mut in_disentanglement_block: bool = false;
    let mut in_wannierization_block: bool = false;
    let mut in_wannierise_section: bool = false;
    let mut in_disentangle_section: bool = false;
    wm.wannierize_conv_threshold = -1.0;
    let mut conv_buffer_value = 0.0;
    let mut wf_accum: Vec<f64> = Vec::new();

    for raw in wannier90_output.lines() {
        let line = raw.trim_start();

        // Per-Wannier-function spreads. Blocks appear each iteration and in the
        // final state; markers are unambiguous, so scan unconditionally and keep
        // the last complete block (the final state naturally wins when finished).
        if line.contains("WF centre and spread") {
            let mut fields = line.split_whitespace();
            // "WF centre and spread   <N>   ( x, y, z )   <spread>"
            let index = fields.nth(4).and_then(|s| s.parse::<u32>().ok());
            let spread = line
                .split_whitespace()
                .last()
                .and_then(|s| s.parse::<f64>().ok());
            if let Some(s) = spread {
                if index == Some(1) {
                    wf_accum.clear();
                }
                wf_accum.push(s);
            }
        } else if line.contains("Sum of centres and spreads") && !wf_accum.is_empty() {
            wm.spread_block.wf_spreads_last = wf_accum.clone();
        }

        // Track WANNIERISE / DISENTANGLE header sections to parse max iterations
        if line.starts_with('*') && line.ends_with('*') {
            if line.contains("WANNIERISE") {
                in_wannierise_section = true;
                in_disentangle_section = false;
            } else if line.contains("DISENTANGLE") {
                in_disentangle_section = true;
                in_wannierise_section = false;
            } else if in_wannierise_section || in_disentangle_section {
                in_wannierise_section = false;
                in_disentangle_section = false;
            }
        }

        if (in_wannierise_section || in_disentangle_section)
            && line.contains("Total number of iterations")
            && let Some(pos) = line.rfind(':')
            && let Ok(n) = line[pos + 1..]
                .trim()
                .trim_end_matches('|')
                .trim()
                .parse::<u32>()
        {
            if in_wannierise_section {
                wm.wannierize_max_iterations = Some(n);
            } else {
                wm.dis_max_iterations = Some(n);
            }
        }

        if line.contains("Extraction of optimally-connected subspace") {
            in_disentanglement_block = true;
        }
        if line.contains("Time to disentangle bands") {
            in_disentanglement_block = false;
        }
        if line.contains(
            "| Iter  Delta Spread     RMS Gradient      Spread (Ang^2)      Time  |<-- CONV",
        ) {
            in_wannierization_block = true;
        }
        if line.contains("Time for wannierise") {
            in_wannierization_block = false;
        }

        if !in_disentanglement_block && !in_wannierization_block {
            if line.contains("Using band disentanglement") {
                if line.contains("T") && line.contains(":") {
                    wm.has_disentanglement = true;
                } else if line.contains("F") && line.contains(":") {
                    wm.has_disentanglement = false;
                }
            }

            if let Some(pos) = line.find("|  Convergence tolerence                     :") {
                let after_colon = pos + "|  Convergence tolerence                     :".len();
                if let Ok(val) = line[after_colon..]
                    .trim()
                    .trim_end_matches('|')
                    .trim()
                    .parse::<f64>()
                {
                    conv_buffer_value = val;
                }
                if wm.wannierize_conv_threshold < 0.0 {
                    wm.wannierize_conv_threshold = conv_buffer_value
                } else if wm.has_disentanglement {
                    wm.disentanglement_conv_threshold = Some(conv_buffer_value)
                }
            }
        }

        if in_disentanglement_block {
            if line.contains("<-- DIS") && !line.contains("Iter") && !line.contains("+---") {
                // Row: Iter  Omega_I(i-1)  Omega_I(i)  Delta(frac.)  Time  <-- DIS
                let fields: Vec<&str> = line.split_whitespace().collect();
                if let (Some(omega_i), Some(delta), Some(time)) = (
                    fields.get(2).and_then(|s| s.parse::<f64>().ok()),
                    fields.get(3).and_then(|s| s.parse::<f64>().ok()),
                    fields.get(4).and_then(|s| s.parse::<f64>().ok()),
                ) {
                    let block = wm
                        .disentanglement_block
                        .get_or_insert_with(DisentanglementBlock::default);
                    block.omega_i.push(omega_i);
                    block.delta_omega_i.push(delta);
                    block.cpu_time.push(time);
                }
            } else if line.contains("<<< Disentanglement convergence criteria satisfied >>>") {
                wm.disentanglement_converged = Some(true);
            } else if line.contains("<<< Disentanglement convergence criteria not satisfied >>>") {
                wm.disentanglement_converged = Some(false);
            }
        }

        if in_wannierization_block
            && line.contains("<-- CONV")
            && !line.contains("Iter")
            && !line.contains("+---")
            && !line.contains("+----")
        {
            let mut parts = line.split_whitespace();
            if let (Some(delta), Some(spread), Some(time)) = (
                parts.nth(1).and_then(|s| s.parse::<f64>().ok()),
                parts.nth(1).and_then(|s| s.parse::<f64>().ok()),
                parts.next().and_then(|s| s.parse::<f64>().ok()),
            ) {
                wm.spread_block.delta_spread.push(delta);
                wm.spread_block.spread.push(spread);
                wm.spread_block.cpu_time.push(time);
            }
        }
    }

    wm
}
