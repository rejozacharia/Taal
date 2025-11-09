use serde::{Deserialize, Serialize};

use crate::{error::DomainError, lesson::LessonDescriptor, DrumPiece, NotatedEvent, TempoMap};

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

pub struct MidiExporter;

impl MidiExporter {
    fn gm_note(piece: DrumPiece) -> u8 {
        match piece {
            DrumPiece::Bass => 36,
            DrumPiece::Snare | DrumPiece::CrossStick => 38,
            DrumPiece::HiHatClosed => 42,
            DrumPiece::HiHatOpen => 46,
            DrumPiece::Ride => 51,
            DrumPiece::Crash => 49,
            DrumPiece::HighTom => 50,
            DrumPiece::LowTom => 47,
            DrumPiece::FloorTom => 41,
            _ => 37,
        }
    }
}

impl NotationExporter for MidiExporter {
    fn export(
        &self,
        lesson: &LessonDescriptor,
        format: ExportFormat,
    ) -> Result<Vec<u8>, DomainError> {
        if !matches!(format, ExportFormat::Midi) {
            return Err(DomainError::validation("MidiExporter can only export MIDI"));
        }
        // Simple SMF type-0, PPQ=480
        let ppq: u16 = 480;
        let bpm = lesson.default_tempo.events()[0].bpm.max(1.0);
        let spb = 60.0 / bpm as f64; // seconds per beat
        let mut events: Vec<(u32, bool, u8, u8)> = Vec::new(); // (tick, on, note, vel)
        for NotatedEvent { event, duration, .. } in &lesson.notation {
            let start_tick = (event.beat * ppq as f64) as u32;
            let dur_ticks = ((duration.as_seconds_f64() / spb) * ppq as f64) as u32;
            let end_tick = start_tick + dur_ticks.max(60);
            let note = Self::gm_note(event.piece);
            let vel = event.velocity.max(1);
            events.push((start_tick, true, note, vel));
            events.push((end_tick, false, note, 64));
        }
        events.sort_by_key(|e| e.0);

        // Helpers
        fn write_u32_be(buf: &mut Vec<u8>, v: u32) { buf.extend_from_slice(&v.to_be_bytes()); }
        fn write_u16_be(buf: &mut Vec<u8>, v: u16) { buf.extend_from_slice(&v.to_be_bytes()); }
        fn write_varlen(buf: &mut Vec<u8>, mut v: u32) {
            let mut tmp = [0u8; 5];
            let mut i = 4;
            tmp[i] = (v & 0x7F) as u8; v >>= 7;
            while v > 0 { i -= 1; tmp[i] = ((v & 0x7F) as u8) | 0x80; v >>= 7; }
            buf.extend_from_slice(&tmp[i..=4]);
        }

        // Header chunk
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"MThd");
        write_u32_be(&mut out, 6);
        write_u16_be(&mut out, 0); // format 0
        write_u16_be(&mut out, 1); // one track
        write_u16_be(&mut out, ppq);

        // Track chunk buffer
        let mut trk: Vec<u8> = Vec::new();
        // Tempo meta (ff 51 03 tttttt)
        let us_per_qn: u32 = (60_000_000f64 / bpm as f64) as u32;
        // Delta 0
        trk.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03,
            ((us_per_qn >> 16) & 0xFF) as u8,
            ((us_per_qn >> 8) & 0xFF) as u8,
            (us_per_qn & 0xFF) as u8]);
        // Time signature (optional) default 4/4
        trk.extend_from_slice(&[0x00, 0xFF, 0x58, 0x04, 0x04, 0x02, 0x18, 0x08]);

        // Events
        let mut last_tick = 0u32;
        for (tick, on, note, vel) in events {
            let delta = tick.saturating_sub(last_tick);
            write_varlen(&mut trk, delta);
            last_tick = tick;
            if on {
                trk.extend_from_slice(&[0x99, note, vel]); // ch 10 note on
            } else {
                trk.extend_from_slice(&[0x89, note, 0]); // note off
            }
        }
        // End of track
        trk.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);

        out.extend_from_slice(b"MTrk");
        write_u32_be(&mut out, trk.len() as u32);
        out.extend_from_slice(&trk);
        Ok(out)
    }
}

pub struct SimpleMusicXmlExporter;

impl NotationExporter for SimpleMusicXmlExporter {
    fn export(
        &self,
        lesson: &LessonDescriptor,
        format: ExportFormat,
    ) -> Result<Vec<u8>, DomainError> {
        if !matches!(format, ExportFormat::MusicXml) {
            return Err(DomainError::validation("SimpleMusicXmlExporter can only export MusicXML"));
        }
        let bpm = lesson.default_tempo.events()[0].bpm;
        let mut s = String::new();
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        s.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 3.1 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
        s.push_str("<score-partwise version=\"3.1\">\n  <part-list>\n    <score-part id=\"P1\"><part-name>Drumset</part-name></score-part>\n  </part-list>\n  <part id=\"P1\">\n");
        s.push_str(&format!("    <!-- tempo {} bpm; simple export -->\n", bpm));
        for e in &lesson.notation {
            s.push_str(&format!("    <measure>\n      <note><unpitched/><duration>{:.0}</duration><voice>1</voice></note>\n    </measure>\n", e.event.beat * 1.0));
        }
        s.push_str("  </part>\n</score-partwise>\n");
        Ok(s.into_bytes())
    }
}

// Importers

pub enum ImportFormat {
    MusicXml,
}

pub struct MusicXmlImporter;

impl MusicXmlImporter {
    pub fn import_str(xml: &str) -> Result<LessonDescriptor, DomainError> {
        use quick_xml::events::Event;
        use quick_xml::Reader;
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut divisions: f64 = 1.0; // divisions per quarter
        let mut bpm: f32 = 120.0;
        let mut beats_accum: f64 = 0.0; // in quarter-note beats
        let mut notation: Vec<NotatedEvent> = Vec::new();
        let mut current_instrument: Option<String> = None;
        let mut in_note = false;
        let mut is_rest = false;
        let mut note_duration_divs: Option<f64> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(e)) => {
                    match e.name().as_ref() {
                        b"divisions" => {
                            if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                                if let Ok(v) = t.unescape().unwrap_or_default().parse::<f64>() { divisions = v.max(1.0); }
                            }
                        }
                        b"sound" => {
                            for a in e.attributes().flatten() {
                                if a.key.as_ref() == b"tempo" {
                                    if let Ok(s) = a.unescape_value() { if let Ok(v) = s.parse::<f32>() { bpm = v.max(1.0); } }
                                }
                            }
                        }
                        b"note" => { in_note = true; is_rest = false; note_duration_divs = None; current_instrument = None; }
                        b"rest" => { if in_note { is_rest = true; } }
                        b"duration" => {
                            if in_note {
                                if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                                    if let Ok(v) = t.unescape().unwrap_or_default().parse::<f64>() { note_duration_divs = Some(v); }
                                }
                            }
                        }
                        b"instrument" => {
                            if in_note {
                                for a in e.attributes().flatten() {
                                    if a.key.as_ref() == b"id" { if let Ok(s) = a.unescape_value() { current_instrument = Some(s.to_string()); } }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(e)) => {
                    match e.name().as_ref() {
                        b"note" => {
                            if !is_rest {
                                let divs = note_duration_divs.unwrap_or(divisions);
                                let dur_beats = divs / divisions; // in quarter beats
                                let piece = current_instrument
                                    .as_ref()
                                    .and_then(|s| map_instr_to_piece(s))
                                    .unwrap_or(DrumPiece::Snare);
                                let ev = crate::events::DrumEvent::new(beats_accum, piece, 96, crate::events::DrumArticulation::Normal);
                                let spb = 60.0 / bpm as f64;
                                let dur_ms = (dur_beats * spb * 1000.0) as i64;
                                notation.push(NotatedEvent::new(ev, time::Duration::milliseconds(dur_ms)));
                                beats_accum += dur_beats;
                            } else {
                                let divs = note_duration_divs.unwrap_or(divisions);
                                let dur_beats = divs / divisions; beats_accum += dur_beats;
                            }
                            in_note = false; is_rest = false; note_duration_divs = None; current_instrument = None;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            buf.clear();
        }

        let tempo = TempoMap::constant(bpm).map_err(|e| DomainError::validation(e.to_string()))?;
        Ok(LessonDescriptor::new("imported-musicxml", "Imported MusicXML", "", 1, tempo, notation))
    }
}

fn map_instr_to_piece(id: &str) -> Option<DrumPiece> {
    let l = id.to_ascii_lowercase();
    if l.contains("snare") { return Some(DrumPiece::Snare); }
    if l.contains("kick") || l.contains("bass") { return Some(DrumPiece::Bass); }
    if l.contains("hihat") || l.contains("hi-hat") {
        if l.contains("open") { return Some(DrumPiece::HiHatOpen); }
        return Some(DrumPiece::HiHatClosed);
    }
    if l.contains("ride") { return Some(DrumPiece::Ride); }
    if l.contains("crash") { return Some(DrumPiece::Crash); }
    if l.contains("floor") { return Some(DrumPiece::FloorTom); }
    if l.contains("tom") {
        if l.contains("high") || l.contains("hi ") { return Some(DrumPiece::HighTom); }
        if l.contains("low") { return Some(DrumPiece::LowTom); }
        return Some(DrumPiece::HighTom);
    }
    None
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
