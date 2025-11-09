use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use taal_audio::io::AudioDecoder;
use taal_domain::{LessonDescriptor, NotatedEvent};

use crate::notation::SimpleQuantizer;
use crate::tempo::TempoEstimator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionJob {
    pub audio_path: String,
    pub title: String,
}

pub struct TranscriptionPipeline {
    tempo: TempoEstimator,
    quantizer: SimpleQuantizer,
}

impl TranscriptionPipeline {
    pub fn new() -> Self {
        Self {
            tempo: TempoEstimator,
            quantizer: SimpleQuantizer,
        }
    }

    #[instrument(skip(self))]
    pub fn transcribe(&self, job: &TranscriptionJob) -> Result<LessonDescriptor> {
        info!("loading audio path={}", job.audio_path);
        let audio = AudioDecoder::open(&job.audio_path)?;
        let tempo = self.tempo.estimate(&audio.samples, audio.sample_rate)?;
        let events: Vec<NotatedEvent> = self.quantizer.quantize(&audio.samples, &tempo);
        Ok(LessonDescriptor::new(
            job.audio_path.clone(),
            job.title.clone(),
            "Auto-generated transcription",
            1,
            tempo,
            events,
        ))
    }
}

impl Default for TranscriptionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_handles_missing_audio() {
        let pipeline = TranscriptionPipeline::new();
        let job = TranscriptionJob {
            audio_path: "missing.wav".to_string(),
            title: "Test".to_string(),
        };
        let result = pipeline.transcribe(&job);
        assert!(result.is_err());
    }
}
