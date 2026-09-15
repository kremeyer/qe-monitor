use qe_monitor::wannier90;

mod common;
use common::fixture;

#[test]
fn only_the_last_run_is_parsed() {
    // EPW appends each wannier90 invocation to the same .wout, so bas_multirun
    // holds two runs back to back while bas_bannerless is the second one alone.
    // Both must parse to the same numbers: the earlier run must not leak in.
    let name = "epw/bas_multirun.wout";
    let out = fixture(name);
    let multirun = wannier90::parse_metrics(&out);

    let name = "derived/bas_bannerless.wout";
    let out = fixture(name);
    let single = wannier90::parse_metrics(&out);

    assert_eq!(
        multirun.spread_block.spread, single.spread_block.spread,
        "{name}"
    );
    assert_eq!(
        multirun.spread_block.wf_spreads_last, single.spread_block.wf_spreads_last,
        "{name}"
    );

    // pin the values too, so this cannot pass by both parsing to nothing
    assert_eq!(multirun.spread_block.spread.len(), 16, "{name}");
    assert_eq!(multirun.spread_block.wf_spreads_last.len(), 8, "{name}");
}

#[test]
fn detects_has_disentanglement() {
    let name = "epw/si_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert!(metrics.has_disentanglement, "{name}");

    // no disentanglement, so it goes straight to wannierisation
    let name = "epw/bn_no_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert!(!metrics.has_disentanglement, "{name}");
    assert!(metrics.disentanglement_block.is_none(), "{name}");
}

#[test]
fn detects_disentanglement_converged() {
    let name = "epw/si_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.disentanglement_converged, Some(true), "{name}");

    // ran out of iterations instead of meeting the criteria
    let name = "own/pt_no_conv.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.disentanglement_converged, Some(false), "{name}");
    assert_eq!(metrics.dis_max_iterations, Some(500), "{name}");

    // never disentangled at all, so there is nothing to have converged
    let name = "epw/bn_no_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.disentanglement_converged, None, "{name}");
}

#[test]
fn detects_max_iterations() {
    let name = "epw/si_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.dis_max_iterations, Some(500), "{name}");
    assert_eq!(metrics.wannierize_max_iterations, Some(1500), "{name}");

    // no DISENTANGLE section, so only the wannierise limit is reported
    let name = "epw/bn_no_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.dis_max_iterations, None, "{name}");
    assert_eq!(metrics.wannierize_max_iterations, Some(50000), "{name}");
}

#[test]
fn detects_convergence_thresholds() {
    let name = "epw/si_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(
        metrics.disentanglement_conv_threshold,
        Some(1.0e-10),
        "{name}"
    );
    assert_eq!(metrics.wannierize_conv_threshold, 1.0e-9, "{name}");

    let name = "epw/bn_no_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.disentanglement_conv_threshold, None, "{name}");
    assert_eq!(metrics.wannierize_conv_threshold, 1.0e-12, "{name}");

    // nothing was parsed, so the threshold keeps its unset sentinel - the plots
    // treat any value <= 0 as "no threshold line to draw"
    let name = "wannier90/abort_nnkpt4.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.wannierize_conv_threshold, -1.0, "{name}");
}

#[test]
fn detects_wf_spreads() {
    // one spread per Wannier function, taken from the last complete block
    let name = "epw/si_disentangle.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.spread_block.wf_spreads_last.len(), 16, "{name}");

    let name = "wannier90/h3s_standalone.wout";
    let out = fixture(name);
    let metrics = wannier90::parse_metrics(&out);
    assert_eq!(metrics.spread_block.wf_spreads_last.len(), 12, "{name}");
}

#[test]
fn aborted_run_yields_empty_metrics() {
    // three lines of failure text: nothing to parse, and nothing should panic
    for file in ["abort_nnkpt4.wout", "abort_nnkpt5.wout"] {
        let name = format!("wannier90/{file}");
        let out = fixture(&name);
        let metrics = wannier90::parse_metrics(&out);
        assert!(!metrics.has_disentanglement, "{name}");
        assert!(metrics.disentanglement_block.is_none(), "{name}");
        assert!(metrics.spread_block.spread.is_empty(), "{name}");
        assert!(metrics.spread_block.wf_spreads_last.is_empty(), "{name}");
    }
}
