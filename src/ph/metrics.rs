/// Phonon-specific metrics
use crate::pw::ScfBlock as RepresentationBlock;

#[derive(Debug, Default)]
pub struct PhMetrics {
    pub num_qpoints: u32,
    pub num_qpoints_completed: u32,
    pub num_representations: Vec<u32>,
    pub num_representations_completed: u32,
    pub representation_blocks: Vec<RepresentationBlock>,
    pub conv_threshold: Option<f64>,
}
