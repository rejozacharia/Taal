use taal_domain::{DrumArticulation, DrumEvent, DrumPiece, NotatedEvent, TempoMap};
use time::Duration;

#[derive(Default)]
pub struct SimpleQuantizer;

impl SimpleQuantizer {
    pub fn quantize(&self, samples: &[f32], tempo: &TempoMap) -> Vec<NotatedEvent> {
        if samples.is_empty() {
            return Vec::new();
        }
        let beat_interval = 0.5; // eighth notes
        let mut events = Vec::new();
        for (index, chunk) in samples.chunks(tempo.events().len().max(1)).enumerate() {
            let velocity = (chunk.iter().map(|s| s.abs()).sum::<f32>() / chunk.len().max(1) as f32
                * 127.0)
                .clamp(1.0, 127.0) as u8;
            let event = DrumEvent::new(
                index as f64 * beat_interval,
                if index % 4 == 0 {
                    DrumPiece::Bass
                } else {
                    DrumPiece::Snare
                },
                velocity,
                DrumArticulation::Normal,
            );
            events.push(NotatedEvent::new(
                event,
                Duration::milliseconds((beat_interval * 500.0) as i64),
            ));
        }
        events
    }
}
