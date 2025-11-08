use egui::{Color32, Pos2, Rect, Response, Sense, Ui};
use taal_domain::{LessonDescriptor, NotatedEvent};

pub struct NotationEditor {
    lesson: LessonDescriptor,
}

impl NotationEditor {
    pub fn new(lesson: LessonDescriptor) -> Self {
        Self { lesson }
    }

    pub fn set_lesson(&mut self, lesson: LessonDescriptor) {
        self.lesson = lesson;
    }

    pub fn draw(&mut self, ui: &mut Ui) -> Response {
        let (rect, response) =
            ui.allocate_at_least(egui::vec2(ui.available_width(), 200.0), Sense::hover());
        let painter = ui.painter_at(Rect::from_min_size(rect.min, rect.size()));
        let total_events = self.lesson.notation.len().max(1) as f32;
        for (index, event) in self.lesson.notation.iter().enumerate() {
            let x = rect.left() + rect.width() * index as f32 / total_events;
            let y = rect.center().y;
            painter.circle_filled(Pos2 { x, y }, 6.0, piece_color(event));
        }
        response
    }

    pub fn event_count(&self) -> usize {
        self.lesson.notation.len()
    }
}

fn piece_color(event: &NotatedEvent) -> Color32 {
    use taal_domain::DrumPiece::*;
    match event.event.piece {
        Bass => Color32::from_rgb(200, 80, 80),
        Snare => Color32::from_rgb(80, 120, 220),
        HiHatClosed | HiHatOpen => Color32::from_rgb(240, 220, 120),
        Crash | Ride => Color32::from_rgb(180, 220, 255),
        _ => Color32::from_rgb(200, 200, 200),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    #[test]
    fn editor_tracks_event_count() {
        let tempo = taal_domain::TempoMap::constant(120.0).unwrap();
        let lesson = taal_domain::LessonDescriptor::new(
            "id",
            "Lesson",
            "desc",
            1,
            tempo,
            vec![NotatedEvent::new(
                taal_domain::DrumEvent::new(
                    0.0,
                    taal_domain::DrumPiece::Snare,
                    96,
                    taal_domain::DrumArticulation::Normal,
                ),
                Duration::milliseconds(500),
            )],
        );
        let editor = NotationEditor::new(lesson);
        assert_eq!(editor.event_count(), 1);
    }
}
