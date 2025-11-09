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
        let mut notation: Vec<NotatedEvent> = Vec::new();

        // Per-voice beat positions to support layered notes
        let mut voice_pos: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

        // State for current note
        let mut in_note = false;
        let mut is_rest = false;
        let mut note_duration_beats: Option<f64> = None;
        let mut current_instrument: Option<String> = None;
        let mut current_voice: Option<String> = None;
        let mut chord_flag = false;
        // Heuristic helpers captured per note
        let mut notehead: Option<String> = None;
        let mut display_step: Option<String> = None;
        let mut display_octave: Option<i32> = None;
        let mut hh_open_artic: bool = false;

        // Persist last known instrument per voice as a weak hint
        let mut last_voice_instr: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        fn type_to_beats(t: &str) -> Option<f64> {
            match t {
                "whole" => Some(4.0),
                "half" => Some(2.0),
                "quarter" => Some(1.0),
                "eighth" => Some(0.5),
                "16th" => Some(0.25),
                "32nd" => Some(0.125),
                "64th" => Some(0.0625),
                _ => None,
            }
        }

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Empty(e)) => {
                    if e.name().as_ref() == b"chord" { chord_flag = true; }
                }
                Ok(Event::Start(e)) => {
                    match e.name().as_ref() {
                        b"divisions" => { if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { if let Ok(v) = t.unescape().unwrap_or_default().parse::<f64>() { divisions = v.max(1.0); } } }
                        b"sound" => {
                            for a in e.attributes().flatten() {
                                if a.key.as_ref() == b"tempo" { if let Ok(s) = a.unescape_value() { if let Ok(v) = s.parse::<f32>() { bpm = v.max(1.0); } } }
                            }
                        }
                        // Tempo fallback: <direction><direction-type><metronome><per-minute>
                        b"per-minute" => { if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { if let Ok(v) = t.unescape().unwrap_or_default().parse::<f32>() { bpm = v.max(1.0); } } }
                        b"note" => { in_note = true; is_rest = false; note_duration_beats = None; current_instrument = None; current_voice = None; chord_flag = false; notehead = None; display_step = None; display_octave = None; hh_open_artic = false; }
                        b"rest" => { if in_note { is_rest = true; } }
                        b"duration" => { if in_note { if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { if let Ok(v) = t.unescape().unwrap_or_default().parse::<f64>() { note_duration_beats = Some(v / divisions); } } } }
                        b"type" => { if in_note { if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { let s = t.unescape().unwrap_or_default().to_string(); if let Some(b) = type_to_beats(&s) { note_duration_beats = Some(b); } } } }
                        b"voice" => { if in_note { if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { current_voice = Some(t.unescape().unwrap_or_default().to_string()); } } }
                        b"notehead" => { if in_note { if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { notehead = Some(t.unescape().unwrap_or_default().to_string()); } } }
                        b"display-step" => { if in_note { if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { display_step = Some(t.unescape().unwrap_or_default().to_string()); } } }
                        b"display-octave" => { if in_note { if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { if let Ok(v) = t.unescape().unwrap_or_default().parse::<i32>() { display_octave = Some(v); } } } }
                        b"instrument" => {
                            // instrument can be attribute id or inner text under <notations><technical>
                            let mut got_text = false;
                            for a in e.attributes().flatten() { if a.key.as_ref() == b"id" { if let Ok(s) = a.unescape_value() { current_instrument = Some(s.to_string()); got_text = true; } } }
                            if !got_text {
                                if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { current_instrument = Some(t.unescape().unwrap_or_default().to_string()); }
                            }
                        }
                        // Treat <open/> articulation as hi-hat open hint when present
                        b"open" => { if in_note { hh_open_artic = true; } }
                        _ => {}
                    }
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"note" {
                        let voice = current_voice.clone().unwrap_or_else(|| "1".to_string());
                        let pos = *voice_pos.entry(voice.clone()).or_insert(0.0);
                        let dur = note_duration_beats.unwrap_or(1.0);
                        if !is_rest {
                            // Resolve instrument priority: explicit instrument -> heuristic by unpitched/notehead -> last voice instrument -> default
                            let mut piece = current_instrument
                                .as_ref()
                                .and_then(|s| map_instr_to_piece(s));
                            if piece.is_none() {
                                piece = map_by_unpitched(&notehead, &display_step, display_octave, hh_open_artic);
                            }
                            if piece.is_none() {
                                if let Some(last) = last_voice_instr.get(&voice) {
                                    piece = map_instr_to_piece(last);
                                }
                            }
                            let piece = piece.unwrap_or(DrumPiece::Snare);
                            let ev = crate::events::DrumEvent::new(pos, piece, 96, crate::events::DrumArticulation::Normal);
                            let spb = 60.0 / bpm as f64;
                            let dur_ms = (dur * spb * 1000.0) as i64;
                            notation.push(NotatedEvent::new(ev, time::Duration::milliseconds(dur_ms)));
                        }
                        if let Some(instr) = &current_instrument { last_voice_instr.insert(voice.clone(), instr.clone()); }
                        if !chord_flag { *voice_pos.entry(voice).or_insert(0.0) += dur; }
                        // reset
                        in_note = false; is_rest = false; note_duration_beats = None; current_instrument = None; current_voice = None; chord_flag = false; notehead = None; display_step = None; display_octave = None; hh_open_artic = false;
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
        if l.contains("mid") { return Some(DrumPiece::LowTom); }
        if l.contains("high") || l.contains("hi ") { return Some(DrumPiece::HighTom); }
        if l.contains("low") { return Some(DrumPiece::LowTom); }
        return Some(DrumPiece::HighTom);
    }
    None
}

// Heuristic mapping when explicit instrument is absent
fn map_by_unpitched(
    notehead: &Option<String>,
    step: &Option<String>,
    octave: Option<i32>,
    hh_open_artic: bool,
) -> Option<DrumPiece> {
    let nh = notehead.as_ref().map(|s| s.to_ascii_lowercase());
    let st = step.as_ref().map(|s| s.to_ascii_uppercase());
    let oct = octave.unwrap_or(0);

    // Cymbal-ish x heads
    if let Some(nh) = &nh {
        if nh.contains('x') {
            // Use display-step heuristics to split hats/crash/ride
            match (st.as_deref(), oct) {
                (Some("G"), 4..=6) | (Some("F"), 4..=6) => {
                    return Some(if hh_open_artic { DrumPiece::HiHatOpen } else { DrumPiece::HiHatClosed });
                }
                (Some("A"), 4..=6) => { return Some(DrumPiece::Crash); }
                (Some("B"), 4..=6) | (Some("C"), 4..=6) => { return Some(DrumPiece::Ride); }
                _ => {
                    // Default x-head to hi-hat closed
                    return Some(if hh_open_artic { DrumPiece::HiHatOpen } else { DrumPiece::HiHatClosed });
                }
            }
        }
    }

    // Non-cymbal heads: use common drum staff positions
    match (st.as_deref(), oct) {
        (Some("F"), 3..=5) => Some(DrumPiece::Bass),
        (Some("C"), 4..=6) => Some(DrumPiece::Snare),
        (Some("E"), 4..=6) => Some(DrumPiece::HighTom),
        (Some("D"), 4..=6) => Some(DrumPiece::LowTom), // treat as mid/low tom
        _ => None,
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

    #[test]
    fn imports_musicxml_multi_instruments() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 3.1 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="3.1">
  <part-list>
    <score-part id="P1"><part-name>Drum Set</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>4</divisions>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>percussion</sign><line>2</line></clef>
      </attributes>
      <direction placement="above"><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>100</per-minute></metronome></direction-type></direction>
      <!-- Hi-hat 16th -->
      <note>
        <unpitched><display-step>G</display-step><display-octave>5</display-octave></unpitched>
        <voice>1</voice><type>16th</type><notehead>x</notehead>
        <notations><technical><instrument>Hi-Hat Closed</instrument></technical></notations>
      </note>
      <!-- Kick on 1 -->
      <note>
        <unpitched><display-step>F</display-step><display-octave>4</display-octave></unpitched>
        <voice>2</voice><type>quarter</type>
        <notations><technical><instrument>Bass Drum</instrument></technical></notations>
      </note>
      <!-- Snare on 2 -->
      <note>
        <unpitched><display-step>C</display-step><display-octave>5</display-octave></unpitched>
        <voice>2</voice><type>quarter</type>
        <notations><technical><instrument>Snare Drum</instrument></technical></notations>
      </note>
      <!-- Crash on next beat -->
      <note>
        <unpitched><display-step>A</display-step><display-octave>5</display-octave></unpitched>
        <voice>1</voice><type>quarter</type><notehead>x</notehead>
        <notations><technical><instrument>Crash Cymbal</instrument></technical></notations>
      </note>
      <!-- Toms fill -->
      <note>
        <unpitched><display-step>E</display-step><display-octave>5</display-octave></unpitched>
        <voice>2</voice><type>16th</type>
        <notations><technical><instrument>High Tom</instrument></technical></notations>
      </note>
      <note>
        <unpitched><display-step>D</display-step><display-octave>5</display-octave></unpitched>
        <voice>2</voice><type>16th</type>
        <notations><technical><instrument>Mid Tom</instrument></technical></notations>
      </note>
    </measure>
  </part>
</score-partwise>"#;

        let lesson = MusicXmlImporter::import_str(xml).expect("import");
        assert!(!lesson.notation.is_empty(), "should import events");
        let mut has_hat = false;
        let mut has_kick = false;
        let mut has_snare = false;
        let mut has_crash = false;
        let mut has_tom = false;
        for e in &lesson.notation {
            match e.event.piece {
                DrumPiece::HiHatClosed | DrumPiece::HiHatOpen => has_hat = true,
                DrumPiece::Bass => has_kick = true,
                DrumPiece::Snare => has_snare = true,
                DrumPiece::Crash => has_crash = true,
                DrumPiece::HighTom | DrumPiece::LowTom | DrumPiece::FloorTom => has_tom = true,
                _ => {}
            }
        }
        assert!(has_hat && has_kick && has_snare && has_crash && has_tom, "expected multiple instruments parsed");
    }
}
