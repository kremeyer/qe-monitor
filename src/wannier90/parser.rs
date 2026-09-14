use crate::app::RunInfo;
use crate::wannier90::{DisentanglementBlock, WannierMetrics};
use crate::{WANN_MARKER, WANN_RESUME_MARKER};

/// The banner title line, printed a few lines above [`WANN_MARKER`] in the same
/// header box. Kept separate so the slice can start at the top of the banner.
const WANN_BANNER_TITLE: &str = "WANNIER90";

/// How far above the welcome line the banner title may sit for us to still count
/// it as part of the same header box (the box is ~4 lines of ~65 columns).
const BANNER_LOOKBEHIND: usize = 512;

/// Restrict `output` to the most recent wannier90 run.
///
/// EPW drives wannier90 in library mode and *appends* to an existing `.wout`, so
/// one file can hold several runs back to back — including aborted ones. Parsing
/// the whole file concatenates their iteration series into a single nonsensical
/// curve and mixes up thresholds and run info. Each invocation reopens with the
/// WANNIER90 banner, so everything from the last banner onwards is the current
/// run. A mid-run `Resuming Wannier90` line is *not* a new run and is kept.
///
/// When no banner is present at all - EPW appended to a `.wout` that begins
/// directly with `Resuming Wannier90` - the resume line is the only run marker
/// left, and every iteration table follows it, so use the last one instead.
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

#[cfg(test)]
mod tests {
    use super::{WANN_MARKER, last_run, parse_metrics, parse_run_info};

    #[test]
    fn parses_omega_i_spread_and_last_wf_spreads() {
        // Minimal excerpt: a DIS table (absolute Omega_I(i) + fractional delta),
        // a CONV block, and two per-WF spread blocks. The last complete block (the
        // "Final State" one) must win for wf_spreads_last.
        let output = r#"
 Extraction of optimally-connected subspace
 +---------------------------------------------------------------------+<-- DIS
 |  Iter     Omega_I(i-1)      Omega_I(i)      Delta (frac.)    Time   |<-- DIS
 +---------------------------------------------------------------------+<-- DIS
       1      22.82053478      21.35148593       6.880E-02      0.00    <-- DIS
       2      21.99618771      20.66625678       6.435E-02      0.05    <-- DIS
 Time to disentangle bands
 +--------------------------------------------------------------------+<-- CONV
 | Iter  Delta Spread     RMS Gradient      Spread (Ang^2)      Time  |<-- CONV
 +--------------------------------------------------------------------+<-- CONV
  WF centre and spread    1  (  0.0, 0.0, 0.0 )     2.98428934
  WF centre and spread    2  (  0.0, 0.0, 0.0 )     1.95250110
  Sum of centres and spreads (  0.0, 0.0, 0.0 )    4.93679044
      0     0.599E+02     0.0000000000       59.9115099639     103.95  <-- CONV
 Final State
  WF centre and spread    1  (  0.0, 0.0, 0.0 )     1.24982419
  WF centre and spread    2  (  0.0, 0.0, 0.0 )     0.78838218
  Sum of centres and spreads (  0.0, 0.0, 0.0 )    2.03820637
 Time for wannierise
"#;

        let wm = parse_metrics(output);

        let db = wm
            .disentanglement_block
            .as_ref()
            .expect("disentanglement block");
        assert_eq!(db.omega_i, vec![21.35148593, 20.66625678]);
        assert_eq!(db.omega_i.len(), db.delta_omega_i.len());
        assert!((db.delta_omega_i[0] - 6.880e-02).abs() < 1e-9);

        assert_eq!(wm.spread_block.spread, vec![59.9115099639]);

        // The Final State block is the last one, so it wins.
        assert_eq!(
            wm.spread_block.wf_spreads_last,
            vec![1.24982419, 0.78838218]
        );
    }

    /// A wannier90 banner header, as printed at the top of every invocation.
    fn banner(release: &str, started: &str) -> String {
        format!(
            "             +---------------------------------------------------+\n\
             \x20            |                                                   |\n\
             \x20            |                   WANNIER90                       |\n\
             \x20            |                                                   |\n\
             \x20            +---------------------------------------------------+\n\
             \x20            |        Welcome to the Maximally-Localized         |\n\
             \x20            |        Generalized Wannier Functions code         |\n\
             \x20            |  Release: {release}   Execution started on {started}  |\n\
             \x20Running in serial (with parallel executable)\n"
        )
    }

    /// One complete run body with a single DIS row and a single CONV row.
    fn run_body(omega: f64, spread: f64) -> String {
        format!(
            " Extraction of optimally-connected subspace\n\
             \x20      1      99.00000000      {omega:.8}       1.000E-02      0.00    <-- DIS\n\
             \x20Time to disentangle bands\n\
             \x20+--------------------------------------------------------------------+<-- CONV\n\
             \x20| Iter  Delta Spread     RMS Gradient      Spread (Ang^2)      Time  |<-- CONV\n\
             \x20+--------------------------------------------------------------------+<-- CONV\n\
             \x20     0     0.100E+01     0.0000000000       {spread:.8}     1.00  <-- CONV\n\
             \x20Time for wannierise\n\
             \x20All done: wannier90 exiting\n"
        )
    }

    /// EPW appends each wannier90 invocation to the same `.wout`. Only the last
    /// run may contribute data - otherwise the iteration series of several runs
    /// get concatenated into one meaningless curve.
    #[test]
    fn only_the_last_run_is_parsed() {
        let output = format!(
            "{}{}{}{}",
            banner("3.0.0", "11Jan2026 at 21:54:37"),
            run_body(11.0, 11.5),
            banner("3.1.0", "11Jan2026 at 22:51:51"),
            run_body(22.0, 22.5),
        );

        let wm = parse_metrics(&output);
        let db = wm.disentanglement_block.as_ref().expect("dis block");
        assert_eq!(db.omega_i, vec![22.0], "earlier run leaked into Omega_I");
        assert_eq!(
            wm.spread_block.spread,
            vec![22.5],
            "earlier run leaked into spread"
        );

        // Run info must describe the current run, not the first one.
        let ri = parse_run_info(&output);
        assert_eq!(ri.executable.as_deref(), Some("Wannier90.x"));
        assert_eq!(ri.version.as_deref(), Some("3.1.0"));
        assert_eq!(ri.start_time.as_deref(), Some("11Jan2026 22:51:51"));
    }

    /// EPW's library mode prints "Resuming Wannier90" partway through a run,
    /// after wannier_setup returns. That is a continuation, not a new run.
    #[test]
    fn resuming_line_does_not_split_a_run() {
        let output = format!(
            "{} Exiting wannier_setup in wannier90 21:52:20\n Resuming Wannier90 at 21:54:37\n{}",
            banner("3.1.0", "11Jan2026 at 21:54:37"),
            run_body(33.0, 33.5),
        );

        let wm = parse_metrics(&output);
        assert_eq!(
            wm.disentanglement_block
                .as_ref()
                .expect("dis block")
                .omega_i,
            vec![33.0],
        );
        assert_eq!(wm.spread_block.spread, vec![33.5]);
    }

    /// An aborted run still counts: it is the newest one, so the panels should
    /// show it as empty rather than resurrecting the previous run's curves.
    #[test]
    fn aborted_last_run_yields_no_data() {
        let output = format!(
            "{}{}{} Exiting.......\n Unrecognised keyword(s) in input file\n",
            banner("3.1.0", "11Jan2026 at 21:54:37"),
            run_body(44.0, 44.5),
            banner("3.1.0", "11Jan2026 at 22:00:00"),
        );

        let wm = parse_metrics(&output);
        assert!(wm.disentanglement_block.is_none());
        assert!(wm.spread_block.spread.is_empty());
    }

    /// A `.wout` that EPW appended to without re-printing the banner starts
    /// straight at "Resuming Wannier90". It has no banner header at all, so the
    /// resume line is both the run marker and the only run info available.
    #[test]
    fn banner_less_output_parses_from_the_resume_line() {
        let output = format!(" Resuming Wannier90 at 01:11:28\n{}", run_body(55.0, 55.5));

        let wm = parse_metrics(&output);
        assert_eq!(wm.spread_block.spread, vec![55.5]);

        let ri = parse_run_info(&output);
        assert_eq!(ri.executable.as_deref(), Some("Wannier90.x"));
        assert_eq!(ri.start_time.as_deref(), Some("01:11:28"));
        assert_eq!(ri.version, None);
    }

    /// Without a banner to split on, the last resume line opens the current run.
    #[test]
    fn banner_less_output_uses_the_last_resume_line() {
        let output = format!(
            " Resuming Wannier90 at 01:11:28\n{} Resuming Wannier90 at 02:22:33\n{}",
            run_body(66.0, 66.5),
            run_body(77.0, 77.5),
        );

        let wm = parse_metrics(&output);
        assert_eq!(wm.spread_block.spread, vec![77.5], "earlier run leaked in");
        assert_eq!(
            parse_run_info(&output).start_time.as_deref(),
            Some("02:22:33"),
        );
    }

    /// Text with neither marker is handed back untouched.
    #[test]
    fn output_without_any_marker_is_kept_whole() {
        let output = run_body(88.0, 88.5);
        assert_eq!(last_run(&output), output);
    }

    /// A banner run keeps reporting the banner's own timestamp, not the resume
    /// line that follows it mid-run.
    #[test]
    fn banner_timestamp_wins_over_resume_line() {
        let output = format!(
            "{} Resuming Wannier90 at 23:59:59\n{}",
            banner("3.1.0", "11Jan2026 at 21:54:37"),
            run_body(99.0, 99.5),
        );
        let ri = parse_run_info(&output);
        assert_eq!(ri.start_time.as_deref(), Some("11Jan2026 21:54:37"));
    }

    /// The slice starts at the banner title line rather than at the welcome line,
    /// so parse_run_info still sees the "WANNIER90" that names the executable.
    #[test]
    fn slice_starts_at_the_banner_title() {
        let output = format!(
            "{}{}",
            banner("3.1.0", "11Jan2026 at 21:54:37"),
            run_body(1.0, 2.0)
        );
        let sliced = last_run(&output);
        let first = sliced.lines().next().unwrap();
        assert!(first.contains("WANNIER90"), "first line: {first:?}");
        assert!(!first.contains(WANN_MARKER));
    }
}
