use anyhow::Result;
use tracing::debug;

use taal_domain::{DomainError, TempoEvent, TempoMap};

#[derive(Default)]
pub struct TempoEstimator;

impl TempoEstimator {
    pub fn estimate(&self, samples: &[f32], sample_rate: u32) -> Result<TempoMap> {
        debug!(
            "estimating tempo",
            sample_rate,
            sample_count = samples.len()
        );
        let bpm = if samples.is_empty() {
            120.0
        } else {
            100.0 + (samples.len() as f32 % 40.0)
        };
        let event = TempoEvent::new(0.0, bpm.max(60.0), (4, 4))?;
        Ok(TempoMap::new(vec![event])?)
    }
}
