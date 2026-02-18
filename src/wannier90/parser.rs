use crate::app::RunInfo;
use crate::wannier90::{DisentanglementBlock, WannierMetrics};

pub fn parse_run_info(wannier90_output: &str) -> RunInfo {
    let mut run_info = RunInfo::default();

    for raw in wannier90_output.lines().take(100) {
        let line = raw.trim_start();

        if line.contains("WANNIER90") {
            // executable
            run_info.executable = Some("Wannier90.x".to_string());
        }

        if line.contains("Release:") {
            // version
            if let Some(pos) = line.find("Release:") {
                let after = line[(pos + "Release:".len())..]
                    .trim()
                    .split_whitespace()
                    .next()
                    .map(|s| s.to_string());
                run_info.version = after;
            }
        }

        if line.contains("Execution started on") {
            // start time
            if let Some(pos) = line.find("Execution started on") {
            let after = line[(pos + "Execution started on".len())..]
                .trim()
                .trim_matches('|')
                .trim()
                .replace(" at ", " ");
            run_info.start_time = Some(after);
            }
        }

        if line.contains("Running in serial") {
            run_info.mpi_ranks = Some("1".to_string());
        }

        if line.contains("Running in parallel on") {
            if let Some(pos) = line.find("Running in parallel on") {
                let after = line[(pos + "Running in parallel on".len())..]
                    .trim_start()
                    .split_whitespace()
                    .next()
                    .map(|s| s.to_string());
                run_info.mpi_ranks = after;
            }
        }

    }

    // Wannier90 only implicitly uses OpenMP for some low-level calls (e.g. BLAS)
    run_info.omp_threads = Some("-".to_string());

    run_info
}

pub fn parse_metrics(wannier90_output: &str) -> WannierMetrics {
    let mut wm = WannierMetrics::default();
    let mut in_disentanglement_block: bool = false;
    let mut in_wannierization_block: bool = false;
    wm.wannierize_conv_threshold = -1.0;
    let mut conv_buffer_value = 0.0;
    
    for raw in wannier90_output.lines() {
        let line = raw.trim_start();

        if line.contains("Extraction of optimally-connected subspace") { in_disentanglement_block = true; }
        if line.contains("Time to disentangle bands") { in_disentanglement_block = false; }
        if line.contains("| Iter  Delta Spread     RMS Gradient      Spread (Ang^2)      Time  |<-- CONV") { in_wannierization_block = true; }
        if line.contains("Time for wannierise") { in_wannierization_block = false; }

        if !in_disentanglement_block && !in_wannierization_block {

            if line.contains("Using band disentanglement") {
                if line.contains("T") && line.contains(":") {
                    wm.has_disentanglement = true;
                } else if line.contains("F") && line.contains(":") {
                    wm.has_disentanglement = false;
                }
            }

            if line.contains("|  Convergence tolerence                     :") {
                if let Some(pos) = line.find(':') {
                    let after = line[(pos + 1)..]
                        .trim()
                        .trim_end_matches('|')
                        .trim()
                        .parse::<f64>()
                        .ok();
                    conv_buffer_value = after.unwrap();
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
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    if let (Ok(delta), Ok(time)) = (parts[3].parse::<f64>(), parts[4].parse::<f64>()) {
                        let block = wm.disentanglement_block.get_or_insert_with(DisentanglementBlock::default);
                        block.delta_omega_i.push(delta);
                        block.cpu_time.push(time);
                    }
                }
            } else if line.contains("<<< Disentanglement convergence criteria satisfied >>>") {
                wm.disentanglement_converged = Some(true);
            } else if line.contains("<<< Disentanglement convergence criteria not satisfied >>>") {
                wm.disentanglement_converged = Some(false);
            }
        }

        if in_wannierization_block {
            if line.contains("<-- CONV") && !line.contains("Iter") && !line.contains("+---") && !line.contains("+----") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    if let (Ok(delta), Ok(spread), Ok(time)) = (
                        parts[1].parse::<f64>(),
                        parts[3].parse::<f64>(),
                        parts[4].parse::<f64>(),
                    ) {
                        wm.spread_block.delta_spread.push(delta);
                        wm.spread_block.spread.push(spread);
                        wm.spread_block.cpu_time.push(time);
                    }
                }
            }
        }

    }

    wm
}