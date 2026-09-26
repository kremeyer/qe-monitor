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
    assert_eq!(metrics.num_qpoints, 82, "{name}");

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
    assert_eq!(metrics.num_representations.len(), 82);
    assert!(metrics.num_representations.iter().all(|&n| n == 12));

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

#[test]
fn detects_grid_totals_for_a_full_run() {
    let name = "own/ph_midrun.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 5, "{name}");
    assert_eq!(metrics.num_qpoints_completed, 0, "{name}");
    assert_eq!(metrics.num_representations, vec![6, 9, 9, 9, 9], "{name}");
    assert_eq!(
        metrics.num_representations.iter().sum::<u32>(),
        42,
        "{name}"
    );

    let name = "ph/metal_multiq.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 8, "{name}");
    assert_eq!(
        metrics.num_representations,
        vec![1, 2, 2, 2, 3, 3, 2, 2],
        "{name}"
    );
}

#[test]
fn recover_range_wins_over_the_grid() {
    let name = "own/ph_qsplit.out";
    let out = fixture(name);
    let metrics = ph::parse_metrics(&out);
    assert_eq!(metrics.num_qpoints, 1, "{name}");
    assert_eq!(metrics.num_representations, vec![18], "{name}");

    for file in ["restart2.out", "restart3.out", "restart4.out"] {
        let name = format!("ph/{file}");
        let metrics = ph::parse_metrics(&fixture(&name));
        assert_eq!(metrics.num_qpoints, 1, "{name}");
    }
}

#[test]
fn single_iteration_representation_has_a_nonzero_duration() {
    // A representation's own timings start only after its first iteration
    let out = "\
     PHONON       :     10.0s CPU     10.5s WALL

     Self-consistent Calculation

      iter #   1 total cpu time :     22.5 secs   av.it.:   5.4
      thresh= 1.000E-02 alpha_mix =  0.700 |ddv_scf|^2 =  4.4E-07

     End of self-consistent calculation
";
    let metrics = ph::parse_metrics(out);
    let block = metrics
        .representation_blocks
        .first()
        .expect("one representation");
    assert_eq!(block.iterations_to_converge(), Some(1));
    assert_eq!(block.time_per_calculation(), Some(12.5));
    assert_eq!(block.time_per_iteration(), Some(12.5));
}
