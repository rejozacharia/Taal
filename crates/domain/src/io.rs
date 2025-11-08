use serde::{Deserialize, Serialize};

use crate::{error::DomainError, lesson::LessonDescriptor};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExportFormat {
    MusicXml,
    Midi,
    Json,
}

pub trait NotationExporter {
    fn export(
        &self,
        lesson: &LessonDescriptor,
        format: ExportFormat,
    ) -> Result<Vec<u8>, DomainError>;
}

pub struct JsonExporter;

impl NotationExporter for JsonExporter {
    fn export(
        &self,
        lesson: &LessonDescriptor,
        format: ExportFormat,
    ) -> Result<Vec<u8>, DomainError> {
        match format {
            ExportFormat::Json => serde_json::to_vec_pretty(lesson)
                .map_err(|err| DomainError::Serialization(err.to_string())),
            other => Err(DomainError::validation(format!(
                "JsonExporter cannot handle {:?}",
                other
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{events::NotatedEvent, tempo::TempoMap};
    use time::Duration;

    #[test]
    fn exports_json() {
        let tempo = TempoMap::constant(120.0).unwrap();
        let lesson = LessonDescriptor::new(
            "id",
            "title",
            "desc",
            2,
            tempo,
            vec![NotatedEvent::new(
                crate::events::DrumEvent::new(
                    0.0,
                    crate::events::DrumPiece::Bass,
                    100,
                    crate::events::DrumArticulation::Normal,
                ),
                Duration::milliseconds(500),
            )],
        );

        let exporter = JsonExporter;
        let bytes = exporter.export(&lesson, ExportFormat::Json).unwrap();
        let output = String::from_utf8(bytes).unwrap();
        assert!(output.contains("\"title\": \"title\""));
    }
}
