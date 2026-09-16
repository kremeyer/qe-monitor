use crate::ui::mean_std;

fn iter_times(cpu_time: &[f64]) -> Vec<f64> {
    cpu_time
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&dt| dt.is_finite() && dt >= 0.0)
        .collect()
}

/// Projected seconds still to run before `max_iters` is reached, paired with an
/// uncertainty from the spread of the iterations seen so far.
pub fn eta_seconds(iter_times: &[f64], done: usize, max_iters: u32) -> Option<(f64, f64)> {
    if iter_times.is_empty() || max_iters == 0 {
        return None;
    }
    let left = f64::from(max_iters.saturating_sub(done as u32));
    let (mean, std) = mean_std(iter_times);
    Some((mean * left, std * left))
}

#[derive(Debug, Default)]

pub struct DisentanglementBlock {
    pub cpu_time: Vec<f64>,      // CPU time at each iteration
    pub omega_i: Vec<f64>,       // absolute Omega_I(i) at each iteration
    pub delta_omega_i: Vec<f64>, // fractional change in Omega_I at each iteration
}

impl DisentanglementBlock {
    /// Per-iteration wall times of the disentanglement phase.
    pub fn iter_times(&self) -> Vec<f64> {
        iter_times(&self.cpu_time)
    }
}

#[derive(Debug, Default)]
pub struct SpreadBlock {
    pub cpu_time: Vec<f64>,        // CPU time at each iteration
    pub spread: Vec<f64>,          // spread at each iteration
    pub delta_spread: Vec<f64>,    // change in spread at each iteration
    pub wf_spreads_last: Vec<f64>, // per-Wannier-function spread of the last complete block
}

impl SpreadBlock {
    /// Per-iteration wall times of the wannierisation phase.
    pub fn iter_times(&self) -> Vec<f64> {
        iter_times(&self.cpu_time)
    }
}

#[derive(Debug, Default)]
pub struct WannierMetrics {
    pub has_disentanglement: bool,
    pub disentanglement_converged: Option<bool>,
    pub disentanglement_conv_threshold: Option<f64>,
    pub wannierize_conv_threshold: f64,
    pub wannierize_max_iterations: Option<u32>,
    pub dis_max_iterations: Option<u32>,
    pub disentanglement_block: Option<DisentanglementBlock>,
    pub spread_block: SpreadBlock,
}
