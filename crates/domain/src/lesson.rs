use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{events::NotatedEvent, tempo::TempoMap};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PracticeGoal {
    pub target_tempo_bpm: f32,
    pub accuracy_threshold: f32,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PracticeStatistics {
    pub average_accuracy: f32,
    pub highest_streak: u32,
    pub last_practiced: Option<OffsetDateTime>,
}

impl PracticeStatistics {
    pub fn new() -> Self {
        Self {
            average_accuracy: 0.0,
            highest_streak: 0,
            last_practiced: None,
        }
    }
}

impl Default for PracticeStatistics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LessonDescriptor {
    pub id: String,
    pub title: String,
    pub description: String,
    pub difficulty: u8,
    pub default_tempo: TempoMap,
    pub notation: Vec<NotatedEvent>,
    pub goals: Vec<PracticeGoal>,
    pub stats: PracticeStatistics,
}

impl LessonDescriptor {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        difficulty: u8,
        default_tempo: TempoMap,
        notation: Vec<NotatedEvent>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            difficulty,
            default_tempo,
            notation,
            goals: Vec::new(),
            stats: PracticeStatistics::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    #[test]
    fn lesson_constructor() {
        let tempo = TempoMap::constant(120.0).unwrap();
        let event = NotatedEvent::new(
            crate::events::DrumEvent::new(
                0.0,
                crate::events::DrumPiece::Snare,
                96,
                crate::events::DrumArticulation::Normal,
            ),
            Duration::seconds(1),
        );
        let lesson =
            LessonDescriptor::new("id", "title", "desc", 3, tempo.clone(), vec![event.clone()]);
        assert_eq!(lesson.notation.len(), 1);
        assert_eq!(lesson.default_tempo, tempo);
        assert_eq!(lesson.difficulty, 3);
        assert_eq!(lesson.stats.highest_streak, 0);
    }
}
