use serde::{Deserialize, Serialize};
use time::Duration;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DrumPiece {
    Crash,
    Ride,
    HiHatClosed,
    HiHatOpen,
    HiHatFoot,
    HighTom,
    LowTom,
    FloorTom,
    Snare,
    CrossStick,
    Bass,
    Splash,
    China,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DrumArticulation {
    Normal,
    Flam,
    Drag,
    Rimshot,
    Ghost,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum DrumDynamic {
    Pianissimo,
    Piano,
    MezzoPiano,
    MezzoForte,
    Forte,
    Fortissimo,
}

impl DrumDynamic {
    pub fn from_velocity(velocity: u8) -> Self {
        match velocity {
            0..=20 => DrumDynamic::Pianissimo,
            21..=50 => DrumDynamic::Piano,
            51..=80 => DrumDynamic::MezzoPiano,
            81..=100 => DrumDynamic::MezzoForte,
            101..=115 => DrumDynamic::Forte,
            _ => DrumDynamic::Fortissimo,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct TimingOffset {
    /// Offset from the quantized grid in milliseconds.
    pub millis: f32,
}

impl TimingOffset {
    pub fn zero() -> Self {
        Self { millis: 0.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DrumEvent {
    /// Beat position (quarter note = 1.0)
    pub beat: f64,
    pub piece: DrumPiece,
    pub articulation: DrumArticulation,
    pub dynamic: DrumDynamic,
    pub velocity: u8,
    pub timing_offset: TimingOffset,
}

impl DrumEvent {
    pub fn new(beat: f64, piece: DrumPiece, velocity: u8, articulation: DrumArticulation) -> Self {
        Self {
            beat,
            piece,
            articulation,
            dynamic: DrumDynamic::from_velocity(velocity),
            velocity,
            timing_offset: TimingOffset::zero(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NotatedEvent {
    pub event: DrumEvent,
    pub duration: Duration,
    pub tuplet: Option<(u8, u8)>,
}

impl NotatedEvent {
    pub fn new(event: DrumEvent, duration: Duration) -> Self {
        Self {
            event,
            duration,
            tuplet: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_from_velocity() {
        assert_eq!(DrumDynamic::from_velocity(10), DrumDynamic::Pianissimo);
        assert_eq!(DrumDynamic::from_velocity(55), DrumDynamic::MezzoPiano);
        assert_eq!(DrumDynamic::from_velocity(120), DrumDynamic::Fortissimo);
    }

    #[test]
    fn drum_event_new_sets_dynamic() {
        let event = DrumEvent::new(1.0, DrumPiece::Snare, 96, DrumArticulation::Normal);
        assert_eq!(event.dynamic, DrumDynamic::MezzoForte);
        assert_eq!(event.timing_offset.millis, 0.0);
    }
}
