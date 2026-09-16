//! PWSCF-specific metrics

fn iter_times(cpu_time: &[f64]) -> Vec<f64> {
    cpu_time
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&dt| dt.is_finite() && dt >= 0.0)
        .collect()
}

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
    pub cpu_time: Vec<f64>, // cumulative CPU time at each iteration

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

    /// Mean wall time of one SCF iteration.
    ///
    /// Averages the gaps between consecutive timings. The first timing lands
    /// *after* the first iteration, so the span covers one interval fewer than
    /// there are iterations - dividing by the iteration count would
    /// under-report what an iteration costs.
    pub fn time_per_iteration(&self) -> Option<f64> {
        let gaps = self.iter_times();
        if gaps.is_empty() {
            return None;
        }
        Some(gaps.iter().sum::<f64>() / gaps.len() as f64)
    }

    /// Per-iteration wall times, from the cumulative series.
    pub fn iter_times(&self) -> Vec<f64> {
        iter_times(&self.cpu_time)
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

    /// Mean wall time of one k-point.
    ///
    /// Averages the gaps between consecutive timings rather than dividing by
    /// the k-point count: the first timing lands after k-point 1, and the
    /// k-point currently being worked on has no timing yet.
    pub fn time_per_iteration(&self) -> Option<f64> {
        let gaps = self.iter_times();
        if gaps.is_empty() {
            return None;
        }
        Some(gaps.iter().sum::<f64>() / gaps.len() as f64)
    }

    /// Per-k-point wall times, from the cumulative series.
    pub fn iter_times(&self) -> Vec<f64> {
        iter_times(&self.cpu_time)
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
    /// Number of k-point pools (`pw.x -nk`). `None` or 1 means no pooling.
    pub npool: Option<u32>,
    pub num_kpts_total: Option<u32>,
    pub scf_conv_thr: Option<f64>,
    pub etot_conv_thr: Option<f64>,
    pub forc_conv_thr: Option<f64>,
    pub press_conv_thr: Option<f64>,

    pub ion_dyn_etot_err: Vec<f64>,  // "Energy error" [Ry]
    pub ion_dyn_forc_err: Vec<f64>,  // "Gradient error" [Ry/Bohr] - max force component
    pub ion_dyn_press_err: Vec<f64>, // "Cell gradient error" [kbar]
}
