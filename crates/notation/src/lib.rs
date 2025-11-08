use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};
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

    pub fn lesson(&self) -> &LessonDescriptor {
        &self.lesson
    }

    pub fn lesson_mut(&mut self) -> &mut LessonDescriptor {
        &mut self.lesson
    }

    pub fn push_event(&mut self, event: NotatedEvent) {
        self.lesson.notation.push(event);
    }

    pub fn draw(&mut self, ui: &mut Ui) -> Response {
        self.draw_with_timeline(ui, 0.0, self.estimate_total_beats(), None, None, None)
    }

    pub fn draw_with_timeline(
        &mut self,
        ui: &mut Ui,
        start_beat: f64,
        total_beats: f64,
        waveform: Option<&[f32]>,
        playhead_beat: Option<f64>,
        loop_region: Option<(f64, f64)>,
    ) -> Response {
        let (rect, response) =
            ui.allocate_at_least(egui::vec2(ui.available_width(), 220.0), Sense::click());
        let painter = ui.painter_at(Rect::from_min_size(rect.min, rect.size()));

        // Draw background waveform if provided.
        if let Some(wf) = waveform {
            let mid = rect.center().y;
            let half_h = rect.height() * 0.35;
            let mut points = Vec::with_capacity(wf.len());
            for (i, s) in wf.iter().enumerate() {
                let t = i as f32 / (wf.len().max(1) as f32);
                let x = rect.left() + rect.width() * t;
                let y = mid - s.clamp(-1.0, 1.0) * half_h;
                points.push(Pos2 { x, y });
            }
            if points.len() >= 2 {
                painter.add(Shape::line(points, Stroke::new(1.0, Color32::from_gray(120))));
            }
        }

        // Draw loop region first (under everything)
        if let Some((a, b)) = loop_region {
            let tb = total_beats.max(1.0) as f32;
            let la = ((a - start_beat) as f32 / tb).clamp(0.0, 1.0);
            let lb = ((b - start_beat) as f32 / tb).clamp(0.0, 1.0);
            let x0 = rect.left() + rect.width() * la.min(lb);
            let x1 = rect.left() + rect.width() * la.max(lb);
            let r = Rect::from_min_max(Pos2 { x: x0, y: rect.top() }, Pos2 { x: x1, y: rect.bottom() });
            painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(255, 255, 0, 24));
        }

        // Draw events using beat positions proportional to timeline length.
        let tb = total_beats.max(1.0) as f32;
        for ev in &self.lesson.notation {
            let t = ((ev.event.beat as f32 - start_beat as f32) / tb).clamp(0.0, 1.0);
            let x = rect.left() + rect.width() * t;
            let y = rect.center().y;
            painter.circle_filled(Pos2 { x, y }, 6.0, piece_color(ev));
        }

        // Playhead
        if let Some(ph) = playhead_beat {
            let t = ((ph as f32 - start_beat as f32) / tb).clamp(0.0, 1.0);
            let x = rect.left() + rect.width() * t;
            painter.line_segment(
                [Pos2 { x, y: rect.top() }, Pos2 { x, y: rect.bottom() }],
                Stroke::new(2.0, Color32::from_rgb(255, 210, 0)),
            );
        }
        response
    }

    fn estimate_total_beats(&self) -> f64 {
        self.lesson
            .notation
            .iter()
            .map(|e| e.event.beat)
            .fold(16.0, |acc, b| acc.max(b + 1.0))
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
