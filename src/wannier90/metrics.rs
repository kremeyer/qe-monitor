#[derive(Debug, Default)]

pub struct DisentanglementBlock {
    pub cpu_time: Vec<f64>,      // CPU time at each iteration
    pub omega_i: Vec<f64>,       // absolute Omega_I(i) at each iteration
    pub delta_omega_i: Vec<f64>, // fractional change in Omega_I at each iteration
}

#[derive(Debug, Default)]
pub struct SpreadBlock {
    pub cpu_time: Vec<f64>,        // CPU time at each iteration
    pub spread: Vec<f64>,          // spread at each iteration
    pub delta_spread: Vec<f64>,    // change in spread at each iteration
    pub wf_spreads_last: Vec<f64>, // per-Wannier-function spread of the last complete block
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
