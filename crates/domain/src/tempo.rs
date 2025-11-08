use serde::{Deserialize, Serialize};
use time::Duration;

use crate::DomainError;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct TempoEvent {
    /// Seconds from the start of the piece.
    pub time: f64,
    /// Beats per minute.
    pub bpm: f32,
    /// Time signature represented as (numerator, denominator).
    pub signature: (u8, u8),
}

impl TempoEvent {
    pub fn new(time: f64, bpm: f32, signature: (u8, u8)) -> Result<Self, DomainError> {
        if time < 0.0 {
            return Err(DomainError::validation(
                "tempo events cannot have negative time",
            ));
        }
        if !(10.0..=400.0).contains(&bpm) {
            return Err(DomainError::validation(
                "tempo bpm must be between 10 and 400",
            ));
        }
        if signature.0 == 0 || !signature.1.is_power_of_two() {
            return Err(DomainError::validation(
                "time signature denominator must be power of two",
            ));
        }
        Ok(Self {
            time,
            bpm,
            signature,
        })
    }

    pub fn seconds_per_beat(&self) -> f64 {
        60.0 / self.bpm as f64
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TempoMap {
    pub(crate) events: Vec<TempoEvent>,
}

impl TempoMap {
    pub fn new(events: Vec<TempoEvent>) -> Result<Self, DomainError> {
        let mut sorted = events;
        sorted.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        if let Some(first) = sorted.first() {
            if first.time != 0.0 {
                return Err(DomainError::validation("tempo map must start at time 0"));
            }
        } else {
            return Err(DomainError::validation(
                "tempo map requires at least one event",
            ));
        }
        Ok(Self { events: sorted })
    }

    pub fn constant(bpm: f32) -> Result<Self, DomainError> {
        Ok(Self {
            events: vec![TempoEvent::new(0.0, bpm, (4, 4))?],
        })
    }

    pub fn events(&self) -> &[TempoEvent] {
        &self.events
    }

    pub fn bpm_at(&self, time: f64) -> f32 {
        let mut current = *self.events.first().expect("tempo map not empty");
        for event in &self.events {
            if event.time <= time {
                current = *event;
            } else {
                break;
            }
        }
        current.bpm
    }

    pub fn time_signature_at(&self, time: f64) -> (u8, u8) {
        let mut current = *self.events.first().expect("tempo map not empty");
        for event in &self.events {
            if event.time <= time {
                current = *event;
            } else {
                break;
            }
        }
        current.signature
    }

    pub fn beat_at_time(&self, time: f64) -> f64 {
        let mut prev_time = 0.0;
        let mut beat_accum = 0.0;
        let mut current = self.events[0];
        for event in &self.events[1..] {
            if event.time > time {
                break;
            }
            let segment_duration = event.time - prev_time;
            beat_accum += segment_duration / current.seconds_per_beat();
            prev_time = event.time;
            current = *event;
        }
        beat_accum + (time - prev_time) / current.seconds_per_beat()
    }

    pub fn duration_between_beats(&self, start_beat: f64, end_beat: f64) -> Duration {
        let mut seconds = 0.0;
        let mut current = self.events[0];
        let mut beat_cursor = start_beat;
        while beat_cursor < end_beat {
            let seconds_per_beat = current.seconds_per_beat();
            let remaining_beats = end_beat - beat_cursor;
            seconds += remaining_beats.min(1.0) * seconds_per_beat;
            beat_cursor += 1.0;
        }
        Duration::seconds_f64(seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_event_validation() {
        assert!(TempoEvent::new(-1.0, 120.0, (4, 4)).is_err());
        assert!(TempoEvent::new(0.0, 5.0, (4, 4)).is_err());
        assert!(TempoEvent::new(0.0, 120.0, (3, 5)).is_err());
        assert!(TempoEvent::new(0.0, 120.0, (4, 4)).is_ok());
    }

    #[test]
    fn tempo_map_queries() {
        let map = TempoMap::new(vec![
            TempoEvent::new(0.0, 120.0, (4, 4)).unwrap(),
            TempoEvent::new(10.0, 90.0, (3, 4)).unwrap(),
        ])
        .unwrap();
        assert_eq!(map.bpm_at(5.0), 120.0);
        assert_eq!(map.bpm_at(12.0), 90.0);
        assert_eq!(map.time_signature_at(12.0), (3, 4));
    }
}
