use qe_monitor::{CalcType, detect_calc_type};
use qe_monitor::{ph, pw, wannier90};

mod common;
use common::{fixture, fixtures_in};

#[test]
fn detects_wannier90_output() {
    let out = fixture("wannier90/h3s_standalone.wout");
    assert_eq!(detect_calc_type(&out), Some(CalcType::Wannier90));
}

#[test]
fn aborted_run_is_not_detected() {
    for name in ["abort_nnkpt4.wout", "abort_nnkpt5.wout"] {
        let out = fixture(&format!("wannier90/{name}"));
        assert_eq!(
            detect_calc_type(&out),
            None,
            "{name} should not be recognized"
        );
    }
}

#[test]
fn detects_executable() {
    for name in fixtures_in("pw") {
        let out = fixture(&name);
        assert_eq!(pw::parse_run_info(&out).executable.as_deref(), Some("pw.x"));
    }
    for name in fixtures_in("ph") {
        let out = fixture(&name);
        assert_eq!(ph::parse_run_info(&out).executable.as_deref(), Some("ph.x"));
    }
    let out = fixture("wannier90/h3s_standalone.wout");
    assert_eq!(
        wannier90::parse_run_info(&out).executable.as_deref(),
        Some("Wannier90.x")
    );
}

/// everything ran with QE 7.5 for now...
#[test]
fn detects_qe_version() {
    for name in fixtures_in("pw") {
        let out = fixture(&name);
        assert_eq!(pw::parse_run_info(&out).version.as_deref(), Some("7.5"));
    }
    for name in fixtures_in("ph") {
        let out = fixture(&name);
        assert_eq!(ph::parse_run_info(&out).version.as_deref(), Some("7.5"));
    }
}

/// everything ran with Wannier 3.1.0 for now...
#[test]
fn detects_wannier90_version() {
    let out = fixture("wannier90/h3s_standalone.wout");
    assert_eq!(
        wannier90::parse_run_info(&out).version.as_deref(),
        Some("3.1.0")
    );
}

/// Check for MPI and OpenMP detection in QE output
#[test]
fn detects_parallelization() {
    // pw was run with 8/1
    for name in fixtures_in("pw") {
        let out = fixture(&name);
        let run_info = pw::parse_run_info(&out);
        assert_eq!(run_info.mpi_ranks.as_deref(), Some("8"));
        assert_eq!(run_info.omp_threads.as_deref(), Some("1"));
    }
    // ph was run with 12/1
    for name in fixtures_in("ph") {
        let out = fixture(&name);
        let run_info = ph::parse_run_info(&out);
        assert_eq!(run_info.mpi_ranks.as_deref(), Some("12"));
        assert_eq!(run_info.omp_threads.as_deref(), Some("1"));
    }
    // one was modified to 14/2
    let out = fixture("own/ph_q40.out");
    let run_info = ph::parse_run_info(&out);
    assert_eq!(run_info.mpi_ranks.as_deref(), Some("14"));
    assert_eq!(run_info.omp_threads.as_deref(), Some("2"));
}
