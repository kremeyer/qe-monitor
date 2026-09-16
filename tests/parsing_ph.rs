use qe_monitor::ph;

mod common;
use common::fixture;

#[test]
fn detects_num_total_qpoints() {
    // different ph runs have different number of q-points
    let name = "ph/base_si.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 1, "{name}");

    let name = "ph/metal_multiq.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 8, "{name}");

    let name = "ph/restart1.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 1, "{name}");

    let name = "ph/restart2.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 1, "{name}");

    let name = "own/ph_q40.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 1, "{name}");

    let name = "own/ph_qsplit.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 1, "{name}");
}

#[test]
fn detects_num_completed_qpoints() {
    let name = "ph/metal_multiq.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints_completed, 8, "{name}");
}

#[test]
fn detects_num_representations() {
    let name = "ph/base_si.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations, vec![3]);

    let name = "ph/metal_multiq.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations, vec![1, 2, 2, 2, 3, 3, 2, 2]);

    let name = "ph/restart1.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations, vec![2]);

    let name = "own/ph_q40.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations, vec![12]);

    let name = "own/ph_qsplit.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations, vec![18]);
}

#[test]
fn detects_num_completed_representations() {
    let name = "ph/base_si.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations_completed, 3, "{name}");

    let name = "ph/metal_multiq.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations_completed, 17, "{name}");

    let name = "ph/restart1.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations_completed, 0, "{name}");

    let name = "own/ph_q40.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations_completed, 6, "{name}");

    let name = "own/ph_qsplit.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_representations_completed, 4, "{name}");
}

#[test]
fn detects_convergence_threshold() {
    let name = "ph/base_si.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.conv_threshold, Some(1e-14), "{name}");

    let name = "ph/metal_multiq.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.conv_threshold, Some(1e-10), "{name}");

    let name = "ph/restart1.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.conv_threshold, Some(1e-18), "{name}");

    let name = "own/ph_q40.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.conv_threshold, Some(1e-14), "{name}");

    let name = "own/ph_qsplit.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.conv_threshold, Some(3e-15), "{name}");
}
