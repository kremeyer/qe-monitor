/// PWSCF-specific metrics

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PwCalcType {
    #[default]
    Scf, // SCF will handle scf, relax, vc-relax, md, vc-md
    Nscf, // NSCF will handle nscf, bands
}

#[derive(Debug, Default)]
pub struct ScfBlock {
    pub iteration: Vec<u32>,
    pub accuracy: Vec<f64>,

    pub conv_iters: Option<u32>, // number of iterations until convergence
    pub cpu_time_first: Option<f64>, // CPU time at first iteration
    pub cpu_time_last: Option<f64>, // CPU time at last iteration
}

impl ScfBlock {
    pub fn iterations_to_converge(&self) -> Option<u32> {
        self.conv_iters
            .or_else(|| self.iteration.iter().copied().max())
    }

    pub fn time_per_calculation(&self) -> Option<f64> {
        match (self.cpu_time_first, self.cpu_time_last) {
            (Some(a), Some(b)) => Some((b - a).max(0.0)),
            _ => None,
        }
    }

    pub fn time_per_iteration(&self) -> Option<f64> {
        let dt = self.time_per_calculation()?;
        let iters = self.iterations_to_converge()? as f64;
        if iters > 0.0 { Some(dt / iters) } else { None }
    }
}

/// Band structure calculation block
#[derive(Debug, Default)]
pub struct BandBlock {
    pub kpt_number: Vec<u32>,        // k-point indices
    pub cpu_time: Vec<f64>,          // CPU time at each k-point
    pub num_kpts: Option<u32>,       // Total k-points
    pub cpu_time_first: Option<f64>, // First timing
    pub cpu_time_last: Option<f64>,  // Last timing
}

impl BandBlock {
    pub fn time_per_calculation(&self) -> Option<f64> {
        match (self.cpu_time_first, self.cpu_time_last) {
            (Some(t1), Some(t2)) => Some((t2 - t1).max(0.0)),
            _ => None,
        }
    }

    pub fn time_per_iteration(&self) -> Option<f64> {
        let dt = self.time_per_calculation()?;
        let kpts = self.kpt_number.len() as f64;
        if kpts > 0.0 { Some(dt / kpts) } else { None }
    }
}

#[derive(Debug, Default)]
pub struct PwMetrics {
    pub calc_type: PwCalcType,
    pub total_force: Vec<f64>,
    pub total_energy: Vec<f64>,
    pub pressure: Vec<f64>,
    pub scf_blocks: Vec<ScfBlock>,
    pub band_blocks: Vec<BandBlock>,
    pub scf_conv_thr: Option<f64>,
    pub etot_conv_thr: Option<f64>,
    pub forc_conv_thr: Option<f64>,
    pub press_conv_thr: Option<f64>,
    // per ion_dynamics step errors (one entry per completed step)
    pub ion_dyn_etot_err: Vec<f64>,  // "Energy error" [Ry]
    pub ion_dyn_forc_err: Vec<f64>,  // "Gradient error" [Ry/Bohr] — max force component
    pub ion_dyn_press_err: Vec<f64>, // "Cell gradient error" [kbar]
}
