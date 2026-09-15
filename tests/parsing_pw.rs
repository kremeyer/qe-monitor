use qe_monitor::pw;

mod common;
use common::fixture;

#[test]
fn detects_calculation_type() {
    // we currently only distinguish SCF and NSCF
    let name = "pw/scf.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.calc_type, pw::PwCalcType::Scf, "{name}");

    let name = "pw/relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.calc_type, pw::PwCalcType::Scf, "{name}");

    let name = "pw/vc_relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.calc_type, pw::PwCalcType::Scf, "{name}");

    let name = "pw/metal_smearing.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.calc_type, pw::PwCalcType::Scf, "{name}");

    let name = "pw/nscf_bands.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.calc_type, pw::PwCalcType::Nscf, "{name}");
}

#[test]
fn detects_total_energy() {
    // one "!    total energy" line per completed SCF cycle
    let name = "pw/scf.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_energy, vec![-15.79449449], "{name}");

    let name = "pw/metal_smearing.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_energy, vec![-4.1854697], "{name}");

    // a relax drives the energy downhill as the geometry settles
    let name = "pw/relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_energy.len(), 5, "{name}");
    assert_eq!(metrics.total_energy.first(), Some(&-43.09625758), "{name}");
    assert_eq!(metrics.total_energy.last(), Some(&-43.109768), "{name}");

    let name = "pw/vc_relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_energy.len(), 11, "{name}");
    assert_eq!(
        metrics.total_energy.len(),
        metrics.scf_blocks.len(),
        "{name}"
    );

    // a band structure run never prints a total energy
    let name = "pw/nscf_bands.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_energy.len(), 0, "{name}");
}

#[test]
fn detects_total_force() {
    // forces are only reported when the run computes them
    let name = "pw/relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_force.len(), 5, "{name}");
    assert_eq!(metrics.total_force.first(), Some(&0.21597), "{name}");
    assert_eq!(metrics.total_force.last(), Some(&4.4e-5), "{name}");

    // converged, so the final force sits below the declared threshold
    let last = *metrics.total_force.last().expect("forces parsed");
    let threshold = metrics.forc_conv_thr.expect("threshold parsed");
    assert!(last < threshold, "{name}");

    let name = "pw/vc_relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_force.len(), 11, "{name}");

    // a plain scf or nscf does not compute forces
    let name = "pw/scf.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_force.len(), 0, "{name}");

    let name = "pw/nscf_bands.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.total_force.len(), 0, "{name}");
}

#[test]
fn detects_pressure() {
    // pressure comes from the stress tensor, so it needs stress to be calculated
    let name = "pw/scf.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.pressure, vec![-29.93], "{name}");

    let name = "pw/metal_smearing.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.pressure, vec![-14.53], "{name}");

    // vc-relax records a pressure per ionic step and declares a threshold
    let name = "pw/vc_relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.pressure.len(), 11, "{name}");
    assert_eq!(metrics.pressure.len(), metrics.total_energy.len(), "{name}");
    assert_eq!(metrics.pressure.first(), Some(&217.54), "{name}");
    assert_eq!(metrics.pressure.last(), Some(&501.95), "{name}");
    assert!(metrics.press_conv_thr.is_some(), "{name}");

    // a plain relax does not compute stress at all
    let name = "pw/relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.pressure.len(), 0, "{name}");

    let name = "pw/nscf_bands.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.pressure.len(), 0, "{name}");
}

#[test]
fn detects_convergence_thresholds() {
    // a plain scf only declares the scf threshold
    let name = "pw/scf.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.scf_conv_thr, Some(1.0e-6), "{name}");
    assert_eq!(metrics.etot_conv_thr, None, "{name}");
    assert_eq!(metrics.forc_conv_thr, None, "{name}");
    assert_eq!(metrics.press_conv_thr, None, "{name}");

    let name = "pw/metal_smearing.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.scf_conv_thr, Some(1.0e-6), "{name}");
    assert_eq!(metrics.etot_conv_thr, None, "{name}");

    // relax adds energy and force targets, but computes no stress
    let name = "pw/relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.scf_conv_thr, Some(1.0e-7), "{name}");
    assert_eq!(metrics.etot_conv_thr, Some(1.0e-4), "{name}");
    assert_eq!(metrics.forc_conv_thr, Some(1.0e-3), "{name}");
    assert_eq!(metrics.press_conv_thr, None, "{name}");

    // vc-relax is the only one with a pressure target, and it restarts with a
    // tighter scf threshold once the cell settles, so the later value wins
    let name = "pw/vc_relax.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.scf_conv_thr, Some(1.0e-9), "{name}");
    assert_eq!(metrics.etot_conv_thr, Some(1.0e-4), "{name}");
    assert_eq!(metrics.forc_conv_thr, Some(1.0e-3), "{name}");
    assert_eq!(metrics.press_conv_thr, Some(0.5), "{name}");

    // nscf declares no thresholds at all
    let name = "pw/nscf_bands.out";
    let out = fixture(name);
    let metrics = pw::parse_metrics(&out);
    assert_eq!(metrics.scf_conv_thr, None, "{name}");
    assert_eq!(metrics.etot_conv_thr, None, "{name}");
    assert_eq!(metrics.forc_conv_thr, None, "{name}");
    assert_eq!(metrics.press_conv_thr, None, "{name}");
}
