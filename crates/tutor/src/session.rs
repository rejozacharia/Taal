use serde::{Deserialize, Serialize};
use taal_domain::{DrumEvent, LessonDescriptor};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PracticeMode {
    Learn,
    Practice,
    Perform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub lesson: LessonDescriptor,
    pub mode: PracticeMode,
    pub current_index: usize,
    pub completed: bool,
}

impl SessionState {
    pub fn new(lesson: LessonDescriptor, mode: PracticeMode) -> Self {
        Self {
            lesson,
            mode,
            current_index: 0,
            completed: false,
        }
    }

    pub fn expect_next(&self) -> Option<&DrumEvent> {
        self.lesson
            .notation
            .get(self.current_index)
            .map(|event| &event.event)
    }

    pub fn register_hit(&mut self, event: &DrumEvent) {
        if let Some(expected) = self.expect_next() {
            if expected.piece == event.piece {
                self.current_index += 1;
            }
        }
        if self.current_index >= self.lesson.notation.len() {
            self.completed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    #[test]
    fn session_tracks_completion() {
        let tempo = taal_domain::TempoMap::constant(120.0).unwrap();
        let lesson = taal_domain::LessonDescriptor::new(
            "id",
            "Lesson",
            "desc",
            1,
            tempo,
            vec![taal_domain::NotatedEvent::new(
                taal_domain::DrumEvent::new(
                    0.0,
                    taal_domain::DrumPiece::Snare,
                    96,
                    taal_domain::DrumArticulation::Normal,
                ),
                Duration::milliseconds(500),
            )],
        );
        let mut session = SessionState::new(lesson, PracticeMode::Learn);
        assert!(!session.completed);
        let hit = taal_domain::DrumEvent::new(
            0.0,
            taal_domain::DrumPiece::Snare,
            96,
            taal_domain::DrumArticulation::Normal,
        );
        session.register_hit(&hit);
        assert!(session.completed);
    }
}
