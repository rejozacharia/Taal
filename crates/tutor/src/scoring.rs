use serde::{Deserialize, Serialize};
use taal_domain::{DrumEvent, LessonDescriptor};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceReport {
    pub accuracy: f32,
    pub early_hits: usize,
    pub late_hits: usize,
}

impl PerformanceReport {
    pub fn empty() -> Self {
        Self {
            accuracy: 0.0,
            early_hits: 0,
            late_hits: 0,
        }
    }
}

pub struct ScoringEngine;

impl ScoringEngine {
    /// Score using a provided seconds-per-beat value (derived from current practice BPM).
    /// Uses timing thresholds in milliseconds.
    pub fn score_with_spb(&self, lesson: &LessonDescriptor, hits: &[DrumEvent], seconds_per_beat: f64) -> PerformanceReport {
        if lesson.notation.is_empty() {
            return PerformanceReport::empty();
        }
        let mut matched = 0usize;
        let mut early = 0usize;
        let mut late = 0usize;
        // simple pairwise comparison for now
        for (expected, actual) in lesson.notation.iter().zip(hits.iter()) {
            let delta_beats = actual.beat - expected.event.beat;
            let delta_ms = (delta_beats * seconds_per_beat * 1000.0).abs();
            if delta_ms < 50.0 {
                matched += 1;
                if delta_beats < 0.0 {
                    early += 1;
                } else if delta_beats > 0.0 {
                    late += 1;
                }
            }
        }
        let accuracy = matched as f32 / lesson.notation.len() as f32;
        PerformanceReport { accuracy, early_hits: early, late_hits: late }
    }

    pub fn score(&self, lesson: &LessonDescriptor, hits: &[DrumEvent]) -> PerformanceReport {
        if lesson.notation.is_empty() {
            return PerformanceReport::empty();
        }
        let mut matched = 0usize;
        let mut early = 0usize;
        let mut late = 0usize;
        for (expected, actual) in lesson.notation.iter().zip(hits.iter()) {
            let delta = actual.beat - expected.event.beat;
            if delta.abs() < 0.25 {
                matched += 1;
                if delta < 0.0 {
                    early += 1;
                } else if delta > 0.0 {
                    late += 1;
                }
            }
        }
        let accuracy = matched as f32 / lesson.notation.len() as f32;
        PerformanceReport {
            accuracy,
            early_hits: early,
            late_hits: late,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    #[test]
    fn scoring_matches_hits() {
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
        let hits = vec![taal_domain::DrumEvent::new(
            0.1,
            taal_domain::DrumPiece::Snare,
            90,
            taal_domain::DrumArticulation::Normal,
        )];
        let engine = ScoringEngine;
        let report = engine.score(&lesson, &hits);
        assert!(report.accuracy > 0.0);
    }
}
