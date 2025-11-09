use std::sync::Arc;

use eframe::{egui, egui::Ui};
use rfd::FileDialog;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use time::Duration;
use taal_domain::{DrumArticulation, DrumEvent, DrumPiece, LessonDescriptor, NotatedEvent, TempoMap, NotationExporter};
use taal_notation::NotationEditor;
use taal_services::MarketplaceClient;
use taal_transcriber::{TranscriptionJob, TranscriptionPipeline};
use taal_tutor::{PracticeMode, ScoringEngine, SessionAnalytics, SessionState};
use tokio::runtime::Runtime;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver};
use midir::{MidiInput, MidiInputConnection};
use dirs;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let rt = Arc::new(Runtime::new()?);
    let options = eframe::NativeOptions::default();
    let rt_clone = rt.clone();
    eframe::run_native(
        "Taal Desktop",
        options,
        Box::new(move |_cc| Box::new(DesktopApp::new(rt_clone.clone()))),
    )
    .map_err(|e| anyhow::anyhow!(format!("{e:?}")))?;
    Ok(())
}

struct DesktopApp {
    active_tab: ActiveTab,
    extractor: ExtractorPane,
    tutor: TutorPane,
    marketplace: MarketplacePane,
    settings: SettingsPane,
}

impl DesktopApp {
    fn new(rt: Arc<Runtime>) -> Self {
        Self {
            active_tab: ActiveTab::Extractor,
            extractor: ExtractorPane::new(),
            tutor: TutorPane::new(),
            marketplace: MarketplacePane::new(rt),
            settings: SettingsPane::new(),
        }
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme preferences (e.g., high-contrast) each frame.
        self.settings.apply_style(ctx);
        // Keep Tutor in sync with current settings
        self.tutor.sync_settings(&self.settings);
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Extractor, "Studio");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Tutor, "Tutor");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Marketplace, "Marketplace");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Settings, "Settings");
            });
        });

        match self.active_tab {
            ActiveTab::Extractor => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.extractor.ui(ui, &mut self.tutor, &mut self.settings);
                });
            }
            ActiveTab::Tutor => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.tutor.ui(ui, &mut self.settings);
                });
            }
            ActiveTab::Marketplace => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.marketplace.ui(ui);
                });
            }
            ActiveTab::Settings => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.settings.ui(ui);
                });
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Extractor,
    Tutor,
    Marketplace,
    Settings,
}

struct ExtractorPane {
    pipeline: TranscriptionPipeline,
    input_path: String,
    status_message: Option<String>,
    editor: Option<NotationEditor>,
    // Creation tools
    selected_piece: DrumPiece,
    selected_velocity: u8,
    grid_total_beats: f64,
    snap_den: u32,
    waveform: Option<Vec<f32>>,
    // Selection and editing
    selected_event: Option<usize>,
    // Transport
    playing: bool,
    bpm: f32,
    playhead: f64,
    loop_enabled: bool,
    loop_start: f64,
    loop_end: f64,
    last_tick: Option<std::time::Instant>,
    next_click_beat: f64,
    // Live MIDI record
    record_enabled: bool,
    midi_rx: Option<Receiver<(u8, u8, u8)>>,
    midi_conn: Option<MidiInputConnection<()>>,
    last_device: Option<String>,
    mapping: HashMap<DrumPiece, u8>,
    // Viewport
    view_start: f64,
    view_span: f64,
    dragging_loop: bool,
    loop_drag_start: f64,
    record_latency_ms: f32,
    lane_mode: bool,
    // Drag state
    drag_on_selected: bool,
    // Multi-select
    selected_set: std::collections::HashSet<usize>,
    marquee_start: Option<egui::Pos2>,
    marquee_active: bool,
    // Click timing for double-click
    last_click_time: Option<std::time::Instant>,
    last_click_idx: Option<usize>,
    // Lane mute/solo sets
    lane_mute: HashSet<DrumPiece>,
    lane_solo: HashSet<DrumPiece>,
    // Undo/redo stacks (notation snapshots)
    undo_stack: Vec<Vec<taal_domain::NotatedEvent>>,
    redo_stack: Vec<Vec<taal_domain::NotatedEvent>>,
}

impl ExtractorPane {
    fn new() -> Self {
        Self {
            pipeline: TranscriptionPipeline::new(),
            input_path: String::new(),
            status_message: None,
            editor: None,
            selected_piece: DrumPiece::Snare,
            selected_velocity: 96,
            grid_total_beats: 16.0,
            snap_den: 8,
            waveform: None,
            selected_event: None,
            playing: false,
            bpm: 120.0,
            playhead: 0.0,
            loop_enabled: false,
            loop_start: 0.0,
            loop_end: 16.0,
            last_tick: None,
            next_click_beat: 0.0,
            record_enabled: false,
            midi_rx: None,
            midi_conn: None,
            last_device: None,
            mapping: default_mapping(),
            view_start: 0.0,
            view_span: 16.0,
            dragging_loop: false,
            loop_drag_start: 0.0,
            record_latency_ms: 0.0,
            lane_mode: true,
            drag_on_selected: false,
            selected_set: std::collections::HashSet::new(),
            marquee_start: None,
            marquee_active: false,
            last_click_time: None,
            last_click_idx: None,
            lane_mute: HashSet::new(),
            lane_solo: HashSet::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tutor: &mut TutorPane, settings: &mut SettingsPane) {
        ui.heading("Chart Studio");
        ui.separator();
        ui.label("Import audio and transcribe, or start a new chart.");
        ui.horizontal(|ui| {
            ui.label("Audio file:");
            ui.text_edit_singleline(&mut self.input_path);
            if ui.button("Browse...").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("Audio", &["wav", "mp3", "flac", "ogg"])
                    .pick_file()
                {
                    self.input_path = path.display().to_string();
                    self.waveform = build_waveform(&self.input_path).map_err(|e| {
                        error!(?e, "waveform build failed");
                        e
                    }).ok();
                }
            }
            if ui.button("Transcribe").clicked() {
                match self.transcribe() {
                    Ok(lesson) => {
                        tutor.load_lesson(lesson.clone());
                        self.editor = Some(NotationEditor::new(lesson.clone()));
                        self.status_message =
                            Some(format!("Transcribed {} events", lesson.notation.len()));
                    }
                    Err(err) => {
                        error!(?err, "failed to transcribe");
                        self.status_message = Some(format!("Error: {}", err));
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("New Chart").clicked() {
                let tempo = TempoMap::constant(120.0).unwrap();
                let lesson = LessonDescriptor::new(
                    "new",
                    "Untitled Chart",
                    "",
                    1,
                    tempo,
                    vec![],
                );
                self.editor = Some(NotationEditor::new(lesson));
                self.status_message = Some("Created new empty chart".to_string());
                self.selected_event = None;
                self.playhead = 0.0;
            }
            if ui.button("Load Sample").clicked() {
                let tempo = TempoMap::constant(100.0).unwrap();
                let mut events = Vec::new();
                for i in 0..8 {
                    let beat = i as f64;
                    events.push(NotatedEvent::new(
                        DrumEvent::new(beat, DrumPiece::Bass, 110, DrumArticulation::Normal),
                        Duration::milliseconds(500),
                    ));
                    events.push(NotatedEvent::new(
                        DrumEvent::new(beat + 0.5, DrumPiece::Snare, 100, DrumArticulation::Normal),
                        Duration::milliseconds(500),
                    ));
                }
                let lesson = LessonDescriptor::new(
                    "sample",
                    "Sample Groove",
                    "Bass on beats, snare on offbeats",
                    1,
                    tempo,
                    events,
                );
                self.editor = Some(NotationEditor::new(lesson));
                self.status_message = Some("Loaded sample transcription".to_string());
            }
            // Save / Open chart (JSON)
            if ui.button("Open Chart").on_hover_text("Load a saved chart (.json)").clicked() {
                if let Some(path) = FileDialog::new().add_filter("Chart", &["json"]).pick_file() {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        match serde_json::from_str::<LessonDescriptor>(&text) {
                            Ok(lesson) => {
                                self.editor = Some(NotationEditor::new(lesson));
                                self.status_message = Some(format!("Loaded chart: {}", path.display()));
                            }
                            Err(err) => { self.status_message = Some(format!("Failed to load: {err}")); }
                        }
                    }
                }
            }
            if ui.button("Save Chart").on_hover_text("Save current chart to JSON").clicked() {
                if let Some(editor) = &self.editor {
                    if let Some(path) = FileDialog::new().set_file_name("chart.json").save_file() {
                        match serde_json::to_string_pretty(editor.lesson()) {
                            Ok(s) => { let _ = std::fs::write(&path, s); self.status_message = Some(format!("Saved to {}", path.display())); }
                            Err(err) => { self.status_message = Some(format!("Failed to save: {err}")); }
                        }
                    }
                } else {
                    self.status_message = Some("Nothing to save".to_string());
                }
            }
            egui::ComboBox::from_id_source("export_menu")
                .selected_text("Export…")
                .show_ui(ui, |ui| {
                    if ui.selectable_label(false, "Export JSON").on_hover_text("Export chart as JSON").clicked() {
                        if let Some(editor) = &self.editor {
                            if let Some(path) = FileDialog::new().set_file_name("chart.json").save_file() {
                                if let Ok(s) = serde_json::to_string_pretty(editor.lesson()) { let _ = std::fs::write(&path, s); }
                            }
                        }
                    }
                    if ui.selectable_label(false, "Export MIDI").on_hover_text("Standard MIDI File (type 0)").clicked() {
                        if let Some(editor) = &self.editor {
                            if let Some(path) = FileDialog::new().set_file_name("chart.mid").save_file() {
                                let exp = taal_domain::io::MidiExporter;
                                match exp.export(editor.lesson(), taal_domain::io::ExportFormat::Midi) {
                                    Ok(bytes) => { let _ = std::fs::write(&path, bytes); }
                                    Err(err) => { self.status_message = Some(format!("Export failed: {}", err)); }
                                }
                            }
                        }
                    }
                    if ui.selectable_label(false, "Export MusicXML").on_hover_text("Simple MusicXML").clicked() {
                        if let Some(editor) = &self.editor {
                            if let Some(path) = FileDialog::new().set_file_name("chart.musicxml").save_file() {
                                let exp = taal_domain::io::SimpleMusicXmlExporter;
                                match exp.export(editor.lesson(), taal_domain::io::ExportFormat::MusicXml) {
                                    Ok(bytes) => { let _ = std::fs::write(&path, bytes); }
                                    Err(err) => { self.status_message = Some(format!("Export failed: {}", err)); }
                                }
                            }
                        }
                    }
                });
        });

        if let Some(message) = &self.status_message {
            ui.label(message);
        }
        ui.separator();
        // Creation tools
        ui.collapsing("Editor Tools", |ui| {
            ui.horizontal(|ui| {
                ui.label("Piece:").on_hover_text("Select drum piece for new notes");
                egui::ComboBox::from_id_source("piece_select")
                    .selected_text(format!("{:?}", self.selected_piece))
                    .show_ui(ui, |ui| {
                        for piece in [
                            DrumPiece::Bass,
                            DrumPiece::Snare,
                            DrumPiece::HiHatClosed,
                            DrumPiece::HiHatOpen,
                            DrumPiece::Ride,
                            DrumPiece::Crash,
                            DrumPiece::HighTom,
                            DrumPiece::LowTom,
                            DrumPiece::FloorTom,
                            DrumPiece::CrossStick,
                        ] {
                            ui.selectable_value(&mut self.selected_piece, piece, format!("{:?}", piece));
                        }
                    });
                ui.label("Velocity:").on_hover_text("MIDI velocity (1-127) for new notes");
                ui.add(egui::Slider::new(&mut self.selected_velocity, 1..=127));
                ui.label("Grid beats:").on_hover_text("Total beats in chart timeline");
                ui.add(egui::Slider::new(&mut self.grid_total_beats, 4.0..=256.0).logarithmic(true));
                ui.label("Snap:").on_hover_text("Note placement grid resolution");
                egui::ComboBox::from_id_source("snap_select")
                    .selected_text(format!("1/{}", self.snap_den))
                    .show_ui(ui, |ui| {
                        for d in [4_u32, 8, 16, 32] {
                            ui.selectable_value(&mut self.snap_den, d, format!("1/{}", d));
                        }
                    });
            });
            ui.separator();
            // Lane controls: Mute/Solo per lane
            if self.lane_mode {
                ui.label("Lane controls (Mute / Solo)");
                let lanes = studio_lanes();
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Clear solo").clicked() { self.lane_solo.clear(); }
                    if ui.small_button("Clear mute").clicked() { self.lane_mute.clear(); }
                    for piece in lanes {
                        ui.group(|ui| {
                            ui.label(format!("{:?}", piece));
                            let is_mute = self.lane_mute.contains(&piece);
                            let is_solo = self.lane_solo.contains(&piece);
                            if ui.small_button(if is_mute {"M*"} else {"M"}).on_hover_text("Toggle mute").clicked() {
                                if is_mute { self.lane_mute.remove(&piece); } else { self.lane_mute.insert(piece); }
                            }
                            ui.add_space(4.0);
                            if ui.small_button(if is_solo {"S*"} else {"S"}).on_hover_text("Toggle solo").clicked() {
                                if is_solo { self.lane_solo.remove(&piece); } else { self.lane_solo.insert(piece); }
                            }
                        });
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(if self.playing {"Pause"} else {"Play"}).on_hover_text("Play/Stop chart preview").clicked() { self.playing = !self.playing; if self.playing { self.last_tick = Some(std::time::Instant::now()); self.next_click_beat = self.playhead.ceil(); } }
                ui.label("BPM").on_hover_text("Preview tempo"); ui.add(egui::Slider::new(&mut self.bpm, 40.0..=220.0));
                ui.checkbox(&mut self.loop_enabled, "Loop").on_hover_text("Loop between start and end beats");
                ui.label("Start"); ui.add(egui::DragValue::new(&mut self.loop_start).speed(0.1));
                ui.label("End"); ui.add(egui::DragValue::new(&mut self.loop_end).speed(0.1));
                if ui.button("<< Reset").on_hover_text("Reset playhead to start").clicked() { self.playhead = 0.0; }
                ui.separator();
                ui.checkbox(&mut self.record_enabled, "Record MIDI").on_hover_text("Arm to capture MIDI into chart while playing");
                ui.separator();
                ui.checkbox(&mut settings.metronome_enabled, "Metronome");
                ui.add(egui::Slider::new(&mut settings.metronome_gain, 0.0..=1.0).text("Click vol"));
                ui.separator();
                if ui.button("Quantize sel").on_hover_text("Quantize selected note to snap").clicked() { self.quantize_selected(); }
                if ui.button("Quantize all").on_hover_text("Quantize all notes to snap").clicked() {
                    // avoid borrow conflict by taking editor mutably after this UI block
                    self.status_message = Some("__DO_QUANTIZE_ALL__".into());
                }
                ui.separator();
                if ui.button("Undo").on_hover_text("Undo last change (Ctrl+Z)").clicked() { self.undo(); }
                if ui.button("Redo").on_hover_text("Redo (Ctrl+Shift+Z)").clicked() { self.redo(); }
                ui.separator();
                ui.checkbox(&mut self.lane_mode, "Lane editor").on_hover_text("Compose per instrument in lanes (Snare first)");
            });
        });

        // Live MIDI record: pre-collect any hits to insert to avoid borrow conflicts
        let mut recorded: Vec<NotatedEvent> = Vec::new();
        if self.record_enabled { self.sync_midi(settings); self.poll_midi_collect(&mut recorded); }

        if self.status_message.as_deref() == Some("__DO_QUANTIZE_ALL__") {
            // take editor out briefly to avoid nested borrow
            let mut tmp = None;
            std::mem::swap(&mut self.editor, &mut tmp);
            if let Some(mut ed) = tmp {
                let snapshot = ed.lesson().notation.clone();
                self.undo_stack.push(snapshot); self.redo_stack.clear();
                self.quantize_all(&mut ed);
                self.editor = Some(ed);
            }
            self.status_message = Some("Quantized all".into());
        }
        if let Some(editor) = &mut self.editor {
            // apply recorded notes (snapshot for undo if needed)
            if !recorded.is_empty() {
                let snapshot = editor.lesson().notation.clone();
                self.undo_stack.push(snapshot); self.redo_stack.clear();
                for ev in recorded.drain(..) { editor.push_event(ev); }
            }

            // advance transport
            if self.playing {
                let now = std::time::Instant::now();
                if let Some(last) = self.last_tick {
                    let dt = now.duration_since(last).as_secs_f64();
                    self.playhead += dt * (self.bpm as f64) / 60.0;
                    if self.loop_enabled && self.playhead >= self.loop_end { self.playhead = self.loop_start; }
                    // Trigger preview sounds for events crossed since last frame
                    let prev = self.last_tick.map(|_| self.playhead - dt * (self.bpm as f64) / 60.0).unwrap_or(self.playhead);
                    let (a, b) = if self.loop_enabled && prev > self.playhead { (prev, self.loop_end) } else { (prev, self.playhead) };
                    for ev in editor.lesson().notation.iter() {
                        if ev.event.beat > a && ev.event.beat <= b {
                            let audible = if !self.lane_solo.is_empty() { self.lane_solo.contains(&ev.event.piece) } else { !self.lane_mute.contains(&ev.event.piece) };
                            if audible { settings.play_drum(ev.event.piece, ev.event.velocity, 80, settings.main_volume * 0.8); }
                        }
                    }
                    if self.loop_enabled && prev > self.playhead {
                        for ev in editor.lesson().notation.iter() {
                            if ev.event.beat > self.loop_start && ev.event.beat <= self.playhead {
                                let audible = if !self.lane_solo.is_empty() { self.lane_solo.contains(&ev.event.piece) } else { !self.lane_mute.contains(&ev.event.piece) };
                                if audible { settings.play_drum(ev.event.piece, ev.event.velocity, 80, settings.main_volume * 0.8); }
                            }
                        }
                    }
                }
                self.last_tick = Some(now);
                // Metronome clicks at integer beats
                if settings.metronome_enabled && settings.app_sounds {
                    while self.playhead >= self.next_click_beat {
                        let accent = (self.next_click_beat as i64) % 4 == 0;
                        let freq = if accent { 880.0 } else { 660.0 };
                        settings.play_tone(freq, 70, settings.main_volume * settings.metronome_gain);
                        self.next_click_beat += 1.0;
                    }
                }
                ui.ctx().request_repaint();
            }

            // Zoom/pan/loop interactions
            let mut response = if self.lane_mode {
                draw_studio_lanes(ui, editor.lesson(), self.view_start, self.view_span, Some(self.playhead), if self.loop_enabled { Some((self.loop_start, self.loop_end)) } else { None }, &self.lane_solo, &self.lane_mute)
            } else {
                editor.draw_with_timeline(ui, self.view_start, self.view_span, self.waveform.as_deref(), Some(self.playhead), if self.loop_enabled { Some((self.loop_start, self.loop_end)) } else { None })
            };
            // Mouse wheel to zoom
            let scroll = ui.input(|i| i.scroll_delta.y);
            if response.hovered() && scroll.abs() > 0.0 {
                let factor = (1.0 - scroll * 0.001).clamp(0.5, 1.5);
                self.view_span = (self.view_span * factor as f64).clamp(1.0, self.grid_total_beats);
            }
            // Middle button drag to pan
            if response.dragged_by(egui::PointerButton::Middle) {
                if let Some(pos) = response.interact_pointer_pos() {
                    let dx = ui.input(|i| i.pointer.delta().x);
                    let beats_per_px = self.view_span / response.rect.width() as f64;
                    self.view_start = (self.view_start - dx as f64 * beats_per_px).clamp(0.0, self.grid_total_beats - self.view_span);
                }
            }
            // Ctrl+drag to set loop
            let ctrl_down = ui.input(|i| i.modifiers.ctrl);
            if ctrl_down && response.drag_started() {
                if let Some(p) = response.interact_pointer_pos() {
                    let t = ((p.x - response.rect.left()) / response.rect.width()).clamp(0.0, 1.0) as f64;
                    self.loop_drag_start = self.view_start + t * self.view_span;
                    self.dragging_loop = true;
                }
            }
            if self.dragging_loop {
                if let Some(p) = response.interact_pointer_pos() {
                    let t = ((p.x - response.rect.left()) / response.rect.width()).clamp(0.0, 1.0) as f64;
                    let b = self.view_start + t * self.view_span;
                    self.loop_start = self.loop_drag_start.min(b);
                    self.loop_end = self.loop_drag_start.max(b);
                    self.loop_enabled = true;
                }
                if response.drag_released() { self.dragging_loop = false; }
            }
            // Apply recorded events (if any)
            for ev in recorded { editor.push_event(ev); }

            // Draw timing axis (beats + bars) aligned with content
            let (left, right) = if self.lane_mode {
                (response.rect.left() + 90.0, response.rect.right() - 8.0)
            } else {
                (response.rect.left(), response.rect.right())
            };
            draw_studio_axis_with_bounds(ui, response.rect, self.view_start, self.view_span, editor.lesson(), left, right);

            // Highlight selected notes (outline) when lane editor is on
            if self.lane_mode && (!self.selected_set.is_empty() || self.selected_event.is_some()) {
                let lane_h = 26.0f32; let margin = 8.0f32; let top = response.rect.top() + margin;
                let mut rings: Vec<usize> = self.selected_set.iter().copied().collect();
                if let Some(i) = self.selected_event { if !self.selected_set.contains(&i) { rings.push(i); } }
                for i in rings {
                    if let Some(ev) = editor.lesson().notation.get(i) {
                        if let Some(row) = studio_lanes().iter().position(|p| *p == ev.event.piece) {
                            let tt = ((ev.event.beat - self.view_start) / self.view_span).clamp(0.0, 1.0) as f32;
                            let x = left + (right - left) * tt; let y = top + row as f32 * lane_h + lane_h * 0.5;
                            ui.painter().circle_stroke(egui::pos2(x, y), 8.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                        }
                    }
                }
            }

            // Click to add/select/drag note
            if let Some(pos) = response.interact_pointer_pos() {
                // In lane mode, ignore clicks outside the lane band to avoid confusing sticky selection
                let lane_ok = if self.lane_mode { piece_from_lane_click(pos, response.rect).is_some() } else { true };
                if !lane_ok { self.selected_event = None; self.drag_on_selected = false; self.marquee_active = false; self.selected_set.clear(); } else {
                let rect = response.rect;
                // Map x->beat using content bounds (lanes have left/right inset)
                let (left, right) = if self.lane_mode { (rect.left() + 90.0, rect.right() - 8.0) } else { (rect.left(), rect.right()) };
                let width = (right - left).max(1.0);
                let t = ((pos.x - left) / width).clamp(0.0, 1.0);
                let mut beat = (t as f64) * self.view_span + self.view_start;
                // Snap to grid based on selection.
                let step = 4.0_f64 / (self.snap_den as f64); // beats per snap tick
                beat = (beat / step).round() * step;

                // Determine nearest note in current lane (if any)
                let lane_piece = if self.lane_mode { piece_from_lane_click(pos, response.rect) } else { None };
                let near_idx = nearest_event_index(editor, beat, lane_piece);
                let near_enough = near_idx
                    .map(|i| (editor.lesson().notation[i].event.beat - beat).abs() <= 0.3)
                    .unwrap_or(false);
                let now_click = std::time::Instant::now();

                // Start dragging only if drag starts on an existing note
                if response.drag_started() {
                    if let Some(i) = near_idx { if near_enough { self.selected_event = Some(i); self.drag_on_selected = true; } }
                    // Box-select without modifier when starting on empty area (or Shift+drag)
                    let mods = ui.input(|i| i.modifiers);
                    if !self.drag_on_selected && !near_enough {
                        self.marquee_start = Some(pos);
                        self.marquee_active = true;
                        if !mods.shift && !mods.ctrl && !mods.command { self.selected_set.clear(); }
                    } else if mods.shift && !self.drag_on_selected {
                        self.marquee_start = Some(pos);
                        self.marquee_active = true;
                        self.selected_set.clear();
                    }
                }

                // Right-click deletes nearest event (by beat) of same piece within threshold.
                if response.clicked_by(egui::PointerButton::Secondary) {
                    let snapshot = editor.lesson().notation.clone();
                    self.undo_stack.push(snapshot); self.redo_stack.clear();
                    let del_piece = lane_piece.or(Some(self.selected_piece));
                    let idx = nearest_event_index(editor, beat, del_piece);
                    if let Some(i) = idx { editor.lesson_mut().notation.remove(i); self.selected_event = None; }
                }

                // Left-click: select if near existing (in lane when lane_mode), else add
                if response.clicked_by(egui::PointerButton::Primary) {
                    let mods = ui.input(|i| i.modifiers);
                    // Ctrl/Cmd-click: toggle note selection only; do not add/clear
                    if mods.ctrl || mods.command {
                        if let Some(i) = near_idx { if near_enough {
                            if self.selected_set.contains(&i) { self.selected_set.remove(&i); } else { self.selected_set.insert(i); }
                            self.selected_event = Some(i);
                            self.last_click_idx = Some(i); self.last_click_time = Some(now_click);
                        }}
                    } else
                    if let Some(i) = near_idx {
                        if near_enough {
                            // Double-click to delete
                            let is_double = self.last_click_idx == Some(i)
                                && self.last_click_time.map(|t| now_click.duration_since(t).as_millis() < 350).unwrap_or(false);
                            if is_double {
                                let snapshot = editor.lesson().notation.clone();
                                self.undo_stack.push(snapshot); self.redo_stack.clear();
                                editor.lesson_mut().notation.remove(i);
                                self.selected_set.remove(&i);
                                self.selected_event = None;
                                self.drag_on_selected = false;
                                self.last_click_idx = None;
                                self.last_click_time = None;
                            } else {
                                // Toggle selection of this note
                                if self.selected_set.contains(&i) { self.selected_set.remove(&i); } else { self.selected_set.insert(i); }
                                self.selected_event = Some(i);
                                self.last_click_idx = Some(i);
                                self.last_click_time = Some(now_click);
                            }
                        }
                    } else {
                        // Clicked empty area in lane
                        // If something is selected and no shift: clear selection instead of adding
                        if !self.selected_set.is_empty() && !ui.input(|i| i.modifiers.shift) {
                            self.selected_set.clear();
                            self.selected_event = None;
                            self.last_click_idx = None;
                            self.last_click_time = None;
                        } else {
                            let piece = if self.lane_mode { lane_piece.unwrap_or(self.selected_piece) } else { self.selected_piece };
                            let snapshot = editor.lesson().notation.clone();
                            self.undo_stack.push(snapshot); self.redo_stack.clear();
                            editor.push_event(NotatedEvent::new(
                                DrumEvent::new(
                                    beat,
                                    piece,
                                    self.selected_velocity,
                                    DrumArticulation::Normal,
                                ),
                                Duration::milliseconds(500),
                            ));
                            self.last_click_idx = None;
                            self.last_click_time = Some(now_click);
                        }
                    }
                }

                // Drag to move selected
                if self.drag_on_selected && response.dragged() {
                    // on first drag, take snapshot
                    if self.redo_stack.is_empty() {
                        let snapshot = editor.lesson().notation.clone();
                        self.undo_stack.push(snapshot); self.redo_stack.clear();
                    }
                    if let Some(sel) = self.selected_event { if let Some(ev) = editor.lesson_mut().notation.get_mut(sel) { ev.event.beat = beat; } }
                }
                if response.drag_released() { self.drag_on_selected = false; }

                // Marquee update
                if self.marquee_active {
                    if let Some(start) = self.marquee_start {
                        let min = egui::pos2(start.x.min(pos.x), start.y.min(pos.y));
                        let max = egui::pos2(start.x.max(pos.x), start.y.max(pos.y));
                        let sel_rect = egui::Rect::from_min_max(min, max);
                        // Compute note positions similar to lane drawing
                        let lane_h = 26.0f32; let margin = 8.0f32; let top = rect.top() + margin; let left_c = left; let right_c = right;
                        self.selected_set.clear();
                        for (i, ev) in editor.lesson().notation.iter().enumerate() {
                            let tt = ((ev.event.beat - self.view_start) / self.view_span).clamp(0.0, 1.0) as f32;
                            let x = left_c + (right_c - left_c) * tt;
                            if let Some(row) = studio_lanes().iter().position(|p| *p == ev.event.piece) {
                                let y = top + row as f32 * lane_h + lane_h * 0.5;
                                let p2 = egui::pos2(x, y);
                                if sel_rect.contains(p2) { self.selected_set.insert(i); }
                            }
                        }
                        // Draw overlay rectangle
                        ui.painter().rect_stroke(sel_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(255,210,0)));
                    }
                    if response.drag_released() { self.marquee_active = false; self.marquee_start = None; }
                }
                }
            }
            // Keyboard delete: delete selected set or single selected
            if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                if !self.selected_set.is_empty() {
                    // Push undo and remove in descending index order to avoid reindex issues
                    let snapshot = editor.lesson().notation.clone();
                    self.undo_stack.push(snapshot); self.redo_stack.clear();
                    // Remove in descending index order
                    let mut idxs: Vec<_> = self.selected_set.iter().copied().collect();
                    idxs.sort_unstable_by(|a,b| b.cmp(a));
                    for i in idxs { if i < editor.lesson().notation.len() { editor.lesson_mut().notation.remove(i); } }
                    self.selected_set.clear();
                    self.selected_event = None;
                } else if let Some(sel) = self.selected_event {
                    let snapshot = editor.lesson().notation.clone();
                    self.undo_stack.push(snapshot); self.redo_stack.clear();
                    if sel < editor.lesson().notation.len() { editor.lesson_mut().notation.remove(sel); }
                    self.selected_event = None;
                }
            }
        } else {
            ui.label("No transcription yet. Provide an audio path and press Transcribe.");
        }
    }

    fn sync_midi(&mut self, settings: &SettingsPane) {
        let device = settings.selected_midi.and_then(|i| settings.midi_inputs.get(i)).map(|d| d.name.clone());
        if device != self.last_device {
            self.last_device = device.clone();
            self.midi_conn = None; self.midi_rx = None;
            if let Some(name) = device { if let Ok((conn, rx)) = open_midi_capture(&name) { self.midi_conn = Some(conn); self.midi_rx = Some(rx); } }
        }
        self.mapping = settings.mapping.clone();
        self.record_latency_ms = settings.latency_ms;
    }

    fn poll_midi_collect(&mut self, out: &mut Vec<NotatedEvent>) {
        if let Some(rx) = &self.midi_rx {
            while let Ok((status, note, vel)) = rx.try_recv() {
                let on = status & 0xF0 == 0x90; if !on { continue; }
                if let Some(piece) = self.mapping.iter().find_map(|(p, n)| if *n == note { Some(*p) } else { None }) {
                    let step = 4.0_f64 / (self.snap_den as f64);
                    // latency compensation in beats
                    let latency_beats = (self.record_latency_ms as f64) / 1000.0 * (self.bpm as f64) / 60.0;
                    let raw = (self.playhead - latency_beats).max(0.0);
                    let beat = (raw / step).round() * step;
                    out.push(NotatedEvent::new(DrumEvent::new(beat, piece, vel, DrumArticulation::Normal), Duration::milliseconds(500)));
                }
            }
        }
    }

    fn quantize_selected(&mut self) {
        if let Some(editor) = &mut self.editor {
            let step = 4.0_f64 / (self.snap_den as f64);
            if !self.selected_set.is_empty() {
                for i in self.selected_set.iter().copied().collect::<Vec<_>>() {
                    if let Some(ev) = editor.lesson_mut().notation.get_mut(i) { ev.event.beat = (ev.event.beat / step).round() * step; }
                }
            } else if let Some(sel) = self.selected_event {
                if let Some(ev) = editor.lesson_mut().notation.get_mut(sel) { ev.event.beat = (ev.event.beat / step).round() * step; }
            }
        }
    }

    fn push_undo(&mut self) {
        if let Some(ed) = &self.editor {
            self.undo_stack.push(ed.lesson().notation.clone());
            self.redo_stack.clear();
        }
    }

    fn undo(&mut self) {
        if let Some(ed) = &mut self.editor {
            if let Some(prev) = self.undo_stack.pop() {
                let current = std::mem::take(&mut ed.lesson_mut().notation);
                ed.lesson_mut().notation = prev;
                self.redo_stack.push(current);
                self.selected_set.clear(); self.selected_event = None;
            }
        }
    }

    fn redo(&mut self) {
        if let Some(ed) = &mut self.editor {
            if let Some(next) = self.redo_stack.pop() {
                let current = std::mem::take(&mut ed.lesson_mut().notation);
                ed.lesson_mut().notation = next;
                self.undo_stack.push(current);
                self.selected_set.clear(); self.selected_event = None;
            }
        }
    }

    fn quantize_all(&mut self, editor: &mut NotationEditor) {
        let step = 4.0_f64 / (self.snap_den as f64);
        for ev in &mut editor.lesson_mut().notation {
            ev.event.beat = (ev.event.beat / step).round() * step;
        }
    }

    fn transcribe(&self) -> anyhow::Result<LessonDescriptor> {
        if self.input_path.trim().is_empty() {
            anyhow::bail!("Please enter an audio file path");
        }
        let job = TranscriptionJob {
            audio_path: self.input_path.clone(),
            title: "Imported Track".to_string(),
        };
        self.pipeline.transcribe(&job)
    }
}

fn nearest_event_index(editor: &NotationEditor, beat: f64, piece_filter: Option<DrumPiece>) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, ev) in editor.lesson().notation.iter().enumerate() {
        if let Some(p) = piece_filter {
            if ev.event.piece != p {
                continue;
            }
        }
        let d = (ev.event.beat - beat).abs();
        let is_better = best.map(|(_, bd)| d < bd).unwrap_or(true);
        if is_better {
            best = Some((i, d));
        }
    }
    // threshold within half a snap step (approx based on 1/16): allow 0.3 beats
    best.and_then(|(i, d)| if d <= 0.3 { Some(i) } else { None })
}

fn build_waveform(path: &str) -> Result<Vec<f32>, SymphoniaError> {
    use std::fs::File;

    let file = File::open(path).map_err(SymphoniaError::from)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.sample_rate.is_some())
        .ok_or(SymphoniaError::Unsupported("no audio track"))?
        .clone();

    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    // Low-res envelope: collect peaks per small window (e.g., 1024 samples).
    let mut envelope: Vec<f32> = Vec::new();
    let mut window_peak = 0.0_f32;
    let window_len = 1024_usize;
    let mut count_in_window = 0_usize;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(e) => return Err(e),
        };
        if packet.track_id() != track.id { continue; }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(e) => return Err(e),
        };
        let spec = *decoded.spec();
        let chans = spec.channels.count();
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        let data = buf.samples();
        for frame in data.chunks(chans) {
            let s = frame[0].abs();
            if s > window_peak { window_peak = s; }
            count_in_window += 1;
            if count_in_window >= window_len {
                envelope.push(window_peak);
                window_peak = 0.0;
                count_in_window = 0;
            }
        }
    }

    if count_in_window > 0 { envelope.push(window_peak); }
    // Normalize to [-1, 1]
    let max = envelope.iter().cloned().fold(1e-6, f32::max);
    for v in &mut envelope { *v = (*v / max).clamp(0.0, 1.0); }
    Ok(envelope)
}

struct TutorPane {
    session: Option<SessionState>,
    hits: Vec<taal_domain::DrumEvent>,
    scoring: ScoringEngine,
    analytics: Option<SessionAnalytics>,
    // Live MIDI
    midi_rx: Option<Receiver<(u8, u8, u8)>>,
    midi_conn: Option<MidiInputConnection<()>>,
    last_device: Option<String>,
    mapping: HashMap<DrumPiece, u8>,
    latency_ms: f32,
    // Highway/transport
    playing: bool,
    bpm: f32,
    playhead: f64,
    last_tick: Option<std::time::Instant>,
    // hit window (ms)
    hit_window_ms: f64,
    // metronome
    metronome_enabled: bool,
    metronome_gain: f32,
    next_click_beat: f64,
    // pre-roll
    pre_roll_beats: u8,
    pre_roll_active: bool,
    pre_roll_remaining: f64,
    // variable tempo support
    elapsed_secs: f64,
    // freeze playhead vs scroll notes
    freeze_playhead: bool,
    statuses: Vec<Option<HitLabel>>,
}

impl TutorPane {
    fn new() -> Self {
        Self {
            session: None,
            hits: Vec::new(),
            scoring: ScoringEngine,
            analytics: None,
            midi_rx: None,
            midi_conn: None,
            last_device: None,
            mapping: default_mapping(),
            latency_ms: 0.0,
            playing: false,
            bpm: 120.0,
            playhead: 0.0,
            last_tick: None,
            hit_window_ms: 75.0,
            metronome_enabled: true,
            metronome_gain: 0.7,
            next_click_beat: 0.0,
            pre_roll_beats: 4,
            pre_roll_active: false,
            pre_roll_remaining: 0.0,
            elapsed_secs: 0.0,
            freeze_playhead: false,
            statuses: Vec::new(),
        }
    }

    fn load_lesson(&mut self, lesson: LessonDescriptor) {
        info!("loading lesson into tutor id={}", lesson.id);
        self.session = Some(SessionState::new(lesson, PracticeMode::Learn));
        self.hits.clear();
        self.analytics = None;
        self.playhead = 0.0;
        self.statuses = self
            .session
            .as_ref()
            .map(|s| vec![None; s.lesson.notation.len()])
            .unwrap_or_default();
    }

    fn ui(&mut self, ui: &mut Ui, settings: &mut SettingsPane) {
        ui.heading("Tutor Mode");
        // Selected MIDI device is configured in Settings
        self.poll_midi();
        if let Some(session) = &mut self.session {
            // Transport + options
            ui.horizontal(|ui| {
                if ui.button(if self.playing {"Pause"} else {"Play"}).clicked() {
                    self.playing = !self.playing;
                    if self.playing {
                        self.last_tick = Some(std::time::Instant::now());
                        self.pre_roll_active = true;
                        self.pre_roll_beats = settings.tutor_pre_roll_beats;
                        self.pre_roll_remaining = self.pre_roll_beats as f64;
                        self.next_click_beat = 0.0;
                    }
                }
                // Use lesson tempo toggle
                let r_use = ui.checkbox(&mut settings.tutor_use_lesson_tempo, "Use lesson tempo");
                if r_use.changed() { settings.mark_dirty(); }
                let mut target_bpm = self.bpm;
                if settings.tutor_use_lesson_tempo {
                    target_bpm = session.lesson.default_tempo.events()[0].bpm;
                    self.bpm = target_bpm;
                }
                ui.label("BPM");
                let r_bpm = ui.add_enabled(!settings.tutor_use_lesson_tempo, egui::Slider::new(&mut self.bpm, 40.0..=240.0));
                if r_bpm.changed() { settings.mark_dirty(); }
                if ui.button("Reset").clicked() { self.playhead = 0.0; }
                ui.separator();
                let r_m = ui.checkbox(&mut settings.metronome_enabled, "Metronome");
                if r_m.changed() { settings.mark_dirty(); }
                let r_mg = ui.add(egui::Slider::new(&mut settings.metronome_gain, 0.0..=1.0).text("Click vol"));
                if r_mg.changed() { settings.mark_dirty(); }
                ui.separator();
                ui.label("Hit window (ms)");
                let r_hw = ui.add(egui::Slider::new(&mut settings.tutor_hit_window_ms, 20.0..=150.0));
                if r_hw.changed() { settings.mark_dirty(); }
                ui.label("Pre-roll (beats)");
                let r_pr = ui.add(egui::Slider::new(&mut settings.tutor_pre_roll_beats, 0..=8));
                if r_pr.changed() { settings.mark_dirty(); }
                ui.separator();
                ui.checkbox(&mut self.freeze_playhead, "Freeze playhead (scroll notes)");
            });

            // Advance playhead
            if self.playing {
                let now = std::time::Instant::now();
                if let Some(last) = self.last_tick {
                    let dt = now.duration_since(last).as_secs_f64();
                    // compute beats advanced based on source (lesson tempo or fixed bpm)
                    let beats_advanced;
                    if settings.tutor_use_lesson_tempo {
                        // approximate using instantaneous bpm at current elapsed time
                        let bpm_now = session.lesson.default_tempo.bpm_at(self.elapsed_secs);
                        beats_advanced = dt * (bpm_now as f64) / 60.0;
                    } else {
                        beats_advanced = dt * (self.bpm as f64) / 60.0;
                    }
                    if self.pre_roll_active {
                        if settings.metronome_enabled && settings.app_sounds {
                            while self.next_click_beat < self.pre_roll_remaining {
                                settings.play_tone( if (self.next_click_beat as i64) % 4 == 0 { 1000.0 } else { 800.0 }, 70, settings.main_volume * settings.metronome_gain);
                                self.next_click_beat += 1.0;
                            }
                        }
                        self.pre_roll_remaining -= beats_advanced;
                        if self.pre_roll_remaining <= 0.0 { self.pre_roll_active = false; self.next_click_beat = 0.0; }
                    } else {
                        if settings.tutor_use_lesson_tempo {
                            self.elapsed_secs += dt;
                            self.playhead = session.lesson.default_tempo.beat_at_time(self.elapsed_secs);
                        } else {
                            self.playhead += beats_advanced;
                        }
                        if settings.metronome_enabled && settings.app_sounds {
                            while self.playhead >= self.next_click_beat {
                                settings.play_tone( if (self.next_click_beat as i64) % 4 == 0 { 880.0 } else { 660.0 }, 70, settings.main_volume * settings.metronome_gain);
                                self.next_click_beat += 1.0;
                            }
                        }
                        let beat_window = (settings.tutor_hit_window_ms / 1000.0) * (self.bpm as f64) / 60.0;
                        let head = self.playhead;
                        for (i, ev) in session.lesson.notation.iter().enumerate() {
                            if self.statuses.get(i).copied().flatten().is_none() && ev.event.beat + beat_window < head {
                                if let Some(s) = self.statuses.get_mut(i) { *s = Some(HitLabel::Missed); }
                            }
                        }
                    }
                }
                self.last_tick = Some(now);
                ui.ctx().request_repaint();
            }

            // Highway lanes
            let window_span = 8.0f64;
            let start = if self.freeze_playhead { self.playhead - window_span * 0.5 } else { self.playhead - 2.0 };
            draw_highway(ui, &session.lesson, self.playhead, &self.statuses, start, window_span, self.freeze_playhead);
            // Countdown overlay during pre-roll
            if self.pre_roll_active {
                let overlay = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("tutor_pre_roll")));
                let screen = ui.ctx().screen_rect();
                let center = screen.center();
                let remaining = self.pre_roll_remaining.ceil() as i32;
                let text = if remaining > 0 { format!("{}", remaining) } else { "Go!".to_string() };
                let font = egui::TextStyle::Heading.resolve(ui.style());
                overlay.text(center, egui::Align2::CENTER_CENTER, text, font, egui::Color32::from_rgb(255, 210, 0));
            }
            // Legend
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                legend_dot(ui, egui::Color32::from_rgb(80,200,120), "On time");
                legend_dot(ui, egui::Color32::from_rgb(160,80,200), "Late");
                legend_dot(ui, egui::Color32::from_rgb(240,200,80), "Early");
                legend_dot(ui, egui::Color32::from_rgb(220,80,80), "Missed");
                legend_dot(ui, egui::Color32::from_rgb(120,170,255), "Not yet played");
            });

            ui.label(format!("Mode: {:?}", session.mode));
            ui.label(format!(
                "Progress: {}/{}",
                session.current_index,
                session.lesson.notation.len()
            ));
            if ui.button("Simulate Hit").clicked() {
                if let Some(expected) = session.expect_next() {
                    let hit = expected.clone();
                    session.register_hit(&hit);
                    self.hits.push(hit.clone());
                }
            }
            if ui.button("Score Performance").clicked() {
                let spb = 60.0 / self.bpm as f64;
                let report = self.scoring.score_with_spb(&session.lesson, &self.hits, spb);
                let mut stats = session.lesson.stats.clone();
                let analytics = SessionAnalytics::new(report.clone());
                analytics.update_statistics(&mut stats);
                self.analytics = Some(analytics);
                ui.label(format!("Accuracy: {:.0}%", report.accuracy * 100.0));
            }
            // Debounced auto-save handled in SettingsPane.tick_autosave()
            if let Some(analytics) = &self.analytics {
                ui.separator();
                ui.label(format!(
                    "Last accuracy: {:.0}%",
                    analytics.report.accuracy * 100.0
                ));
            }
        } else {
            ui.label("Load a lesson from the extractor to begin practice.");
        }
    }

    fn sync_settings(&mut self, settings: &SettingsPane) {
        // Device
        let device = settings.selected_midi.and_then(|i| settings.midi_inputs.get(i)).map(|d| d.name.clone());
        if device != self.last_device {
            self.last_device = device.clone();
            self.midi_conn = None;
            self.midi_rx = None;
            if let Some(name) = device {
                if let Ok((conn, rx)) = open_midi_capture(&name) { self.midi_conn = Some(conn); self.midi_rx = Some(rx); }
            }
        }
        self.mapping = settings.mapping.clone();
        self.latency_ms = settings.latency_ms;
        self.hit_window_ms = settings.tutor_hit_window_ms;
        self.pre_roll_beats = settings.tutor_pre_roll_beats;
    }

    fn poll_midi(&mut self) {
        if let Some(rx) = &self.midi_rx {
            // drain into vec first to avoid immutable borrow during handling
            let mut buf: Vec<(u8, u8, u8)> = Vec::new();
            while let Ok(msg) = rx.try_recv() { buf.push(msg); }
            for (status, note, vel) in buf {
                let on = status & 0xF0 == 0x90; // Note On
                if !on { continue; }
                // Map note to piece
                if let Some(piece) = self.mapping.iter().find_map(|(p, n)| if *n == note { Some(*p) } else { None }) {
                    self.handle_live_hit(piece, vel);
                }
            }
        }
    }

    fn handle_live_hit(&mut self, piece: DrumPiece, vel: u8) {
        // Convert latency to beats
        let latency_beats = (self.latency_ms as f64) / 1000.0 * (self.bpm as f64) / 60.0;
        let hit_beat = (self.playhead - latency_beats).max(0.0);
        if let Some(session) = &mut self.session {
            // Find nearest unmatched expected for this piece
            let mut best: Option<(usize, f64)> = None;
            for (i, ev) in session.lesson.notation.iter().enumerate() {
                if ev.event.piece != piece { continue; }
                if self.statuses.get(i).copied().flatten().is_some() { continue; }
                let d = (ev.event.beat - hit_beat).abs();
                let beat_window = (self.hit_window_ms / 1000.0) * (self.bpm as f64) / 60.0;
                if d < beat_window { if best.map(|(_, bd)| d < bd).unwrap_or(true) { best = Some((i, d)); } }
            }
            if let Some((idx, _)) = best {
                let expected = session.lesson.notation[idx].event.beat;
                let delta = hit_beat - expected;
                let ontime_beats = (20.0 / 1000.0) * (self.bpm as f64) / 60.0; // 20ms band
                let label = if delta.abs() < ontime_beats { HitLabel::OnTime } else if delta > 0.0 { HitLabel::Late } else { HitLabel::Early };
                if let Some(s) = self.statuses.get_mut(idx) { *s = Some(label); }
                let ev = DrumEvent::new(hit_beat, piece, vel, DrumArticulation::Normal);
                session.register_hit(&ev);
                self.hits.push(ev);
            }
        }
    }

}

fn draw_highway(ui: &mut Ui, lesson: &LessonDescriptor, playhead: f64, statuses: &[Option<HitLabel>], start: f64, window_span: f64, freeze_playhead: bool) {
    let lanes = ordered_lanes();
        let lane_h = 28.0f32;
        let margin = 8.0f32;
        let width = ui.available_width();
        let height = lanes.len() as f32 * lane_h + margin * 2.0;
        let (rect, _resp) = ui.allocate_at_least(egui::vec2(width, height), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let left = rect.left() + 90.0; // space for lane labels
        let right = rect.right() - 10.0;
        let top = rect.top() + margin;

        // Draw lanes + labels
        for (row, piece) in lanes.iter().enumerate() {
            // alternating lane backgrounds for readability
            let lane_top = top + row as f32 * lane_h;
            let bg = if row % 2 == 0 { egui::Color32::from_rgba_unmultiplied(255,255,255,6) } else { egui::Color32::from_rgba_unmultiplied(255,255,255,0) };
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(left, lane_top), egui::vec2(right-left, lane_h)), 0.0, bg);
            let y = top + row as f32 * lane_h + lane_h * 0.5;
            painter.line_segment([egui::pos2(left, y), egui::pos2(right, y)], egui::Stroke::new(1.0, egui::Color32::from_gray(80)));
            painter.text(egui::pos2(rect.left() + 6.0, y), egui::Align2::LEFT_CENTER, format!("{:?}", piece), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::LIGHT_GRAY);
        }

        // Window mapping
        let end = start + window_span;
        let to_x = |beat: f64| {
            let t = ((beat - start) / window_span).clamp(0.0, 1.0) as f32;
            left + (right - left) * t
        };

        // Beat and bar markers
        let sig = lesson.default_tempo.time_signature_at(0.0);
        let denom = sig.1.max(1) as f64;
        let beats_per_bar = sig.0.max(1) as f64 * (4.0 / denom);
        let mut b = start.floor();
        let mut measure_idx = (start / beats_per_bar).floor() as i64 + 1;
        while b <= end {
            let x = to_x(b);
            let pos_in_bar = (b / beats_per_bar).fract();
            let strong = pos_in_bar < 1e-6;
            let col = if strong { egui::Color32::from_gray(120) } else { egui::Color32::from_gray(70) };
            let w = if strong { 2.0 } else { 1.0 };
            painter.line_segment([egui::pos2(x, top), egui::pos2(x, rect.bottom())], egui::Stroke::new(w, col));
            if strong {
                painter.text(egui::pos2(x + 2.0, top - 2.0), egui::Align2::LEFT_BOTTOM, format!("{}", measure_idx), egui::TextStyle::Small.resolve(ui.style()), egui::Color32::from_gray(160));
                measure_idx += 1;
            }
            b += 1.0;
        }

        // Draw expected notes and status colors
        for (i, ev) in lesson.notation.iter().enumerate() {
            if ev.event.beat < start || ev.event.beat > end { continue; }
            let lane = lanes.iter().position(|p| *p == ev.event.piece).unwrap_or(0);
            let y = top + lane as f32 * lane_h + lane_h * 0.5;
            let x = to_x(ev.event.beat);
            let color = match statuses.get(i).and_then(|s| *s) {
                Some(HitLabel::OnTime) => egui::Color32::from_rgb(80, 200, 120),
                Some(HitLabel::Late) => egui::Color32::from_rgb(160, 80, 200),
                Some(HitLabel::Early) => egui::Color32::from_rgb(240, 200, 80),
                Some(HitLabel::Missed) => egui::Color32::from_rgb(220, 80, 80),
                None => egui::Color32::from_rgb(120, 170, 255), // Not yet played
            };
            painter.circle_filled(egui::pos2(x, y), 7.5, color);
        }

        // Playhead line
        let x = to_x(playhead);
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 210, 0)));
}

fn legend_dot(ui: &mut Ui, color: egui::Color32, label: &str) {
    let (rect, _resp) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 6.0, color);
    ui.add_space(4.0);
    ui.label(label);
}

fn draw_studio_axis_with_bounds(ui: &mut Ui, rect: egui::Rect, start: f64, span: f64, lesson: &LessonDescriptor, left: f32, right: f32) {
    let painter = ui.painter_at(rect);
    let sig = lesson.default_tempo.time_signature_at(0.0);
    let denom = sig.1.max(1) as f64;
    let beats_per_bar = sig.0.max(1) as f64 * (4.0 / denom);
    let end = start + span;
    let to_x = |beat: f64| left + (right - left) * ((beat - start) / span).clamp(0.0, 1.0) as f32;
    let mut b = start.floor();
    let mut measure_idx = (start / beats_per_bar).floor() as i64 + 1;
    while b <= end {
        let x = to_x(b);
        let strong = ((b / beats_per_bar).fract()) < 1e-6;
        let col = if strong { egui::Color32::from_gray(100) } else { egui::Color32::from_gray(60) };
        let w = if strong { 2.0 } else { 1.0 };
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(w, col));
        if strong {
            painter.text(egui::pos2(x + 2.0, rect.top() - 2.0), egui::Align2::LEFT_BOTTOM, format!("{}", measure_idx), egui::TextStyle::Small.resolve(ui.style()), egui::Color32::from_gray(140));
            measure_idx += 1;
        }
        b += 1.0;
    }
}

fn studio_lanes() -> Vec<DrumPiece> {
    // Snare first, then others
    use DrumPiece::*;
    vec![Snare, Bass, HiHatClosed, HiHatOpen, HighTom, LowTom, FloorTom, Ride, Crash]
}

fn lane_color(piece: DrumPiece) -> egui::Color32 {
    match piece {
        DrumPiece::Snare => egui::Color32::from_rgb(70, 130, 255),     // vivid blue
        DrumPiece::Bass => egui::Color32::from_rgb(230, 70, 70),        // red
        DrumPiece::HiHatClosed => egui::Color32::from_rgb(245, 215, 80),// yellow
        DrumPiece::HiHatOpen => egui::Color32::from_rgb(255, 165, 60),  // orange
        DrumPiece::HighTom => egui::Color32::from_rgb(80, 220, 220),    // cyan
        DrumPiece::LowTom => egui::Color32::from_rgb(60, 200, 140),     // green
        DrumPiece::FloorTom => egui::Color32::from_rgb(150, 120, 230),  // purple
        DrumPiece::Ride => egui::Color32::from_rgb(130, 200, 255),      // light sky
        DrumPiece::Crash => egui::Color32::from_rgb(255, 160, 160),     // salmon
        DrumPiece::CrossStick => egui::Color32::from_rgb(200, 160, 120),// tan
        _ => egui::Color32::from_gray(180),
    }
}

fn draw_studio_lanes(
    ui: &mut Ui,
    lesson: &LessonDescriptor,
    start_beat: f64,
    span_beats: f64,
    playhead: Option<f64>,
    loop_region: Option<(f64, f64)>,
    solo: &HashSet<DrumPiece>,
    mute: &HashSet<DrumPiece>,
) -> egui::Response {
    let lanes = studio_lanes();
    let lane_h = 26.0f32;
    let margin = 8.0f32;
    let height = lanes.len() as f32 * lane_h + margin * 2.0;
    let (rect, response) = ui.allocate_at_least(egui::vec2(ui.available_width(), height), egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let left = rect.left() + 90.0;
    let right = rect.right() - 8.0;
    let top = rect.top() + margin;

    // lane backgrounds and labels
    for (row, piece) in lanes.iter().enumerate() {
        let lane_top = top + row as f32 * lane_h;
        let bg = if row % 2 == 0 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 6) } else { egui::Color32::TRANSPARENT };
        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(left, lane_top), egui::vec2(right - left, lane_h)), 0.0, bg);
        let y = lane_top + lane_h * 0.5;
        painter.text(egui::pos2(rect.left() + 6.0, y), egui::Align2::LEFT_CENTER, format!("{:?}", piece), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::LIGHT_GRAY);
    }

    // loop highlight
    if let Some((a, b)) = loop_region {
        let x0 = left + (right - left) * ((a - start_beat) / span_beats).clamp(0.0, 1.0) as f32;
        let x1 = left + (right - left) * ((b - start_beat) / span_beats).clamp(0.0, 1.0) as f32;
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x1, rect.bottom())), 0.0, egui::Color32::from_rgba_unmultiplied(255, 255, 0, 24));
    }

    // notes
    for ev in &lesson.notation {
        let t = ((ev.event.beat - start_beat) / span_beats).clamp(0.0, 1.0) as f32;
        let x = left + (right - left) * t;
        if let Some(row) = lanes.iter().position(|p| *p == ev.event.piece) {
            let y = top + row as f32 * lane_h + lane_h * 0.5;
            let mut c = lane_color(ev.event.piece);
            let dim = (!solo.is_empty() && !solo.contains(&ev.event.piece)) || mute.contains(&ev.event.piece);
            if dim { c = egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 120); }
            painter.circle_filled(egui::pos2(x, y), 6.0, c);
        }
    }

    // playhead
    if let Some(ph) = playhead {
        let t = ((ph - start_beat) / span_beats).clamp(0.0, 1.0) as f32;
        let x = left + (right - left) * t;
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 210, 0)));
    }

    response
}

fn piece_from_lane_click(pos: egui::Pos2, rect: egui::Rect) -> Option<DrumPiece> {
    // Match bounds used by draw_studio_lanes
    let lanes = studio_lanes();
    let lane_h = 26.0f32;
    let margin = 8.0f32;
    let left = rect.left() + 90.0;
    let right = rect.right() - 8.0;
    let top = rect.top() + margin;
    if pos.x < left || pos.x > right { return None; }
    let row = ((pos.y - top) / lane_h).floor() as isize;
    if row < 0 { return None; }
    lanes.get(row as usize).cloned()
}

#[derive(Clone, Copy, Debug)]
enum HitLabel { OnTime, Late, Early, Missed }

fn ordered_lanes() -> Vec<DrumPiece> {
    use DrumPiece::*;
    vec![Crash, Ride, HiHatOpen, HiHatClosed, Snare, HighTom, LowTom, FloorTom, Bass]
}

struct SettingsPane {
    // Audio
    audio_devices: Vec<String>,
    selected_audio: Option<usize>,
    exclusive_mode: bool,
    latency_ms: f32,
    main_volume: f32,
    // Options
    app_sounds: bool,
    auto_preview: bool,
    high_contrast: bool,
    play_streaks: bool,
    new_keys_exp: bool,
    // MIDI
    midi_inputs: Vec<taal_tutor::midi::MidiDevice>,
    selected_midi: Option<usize>,
    // Mapping wizard
    mapping: HashMap<DrumPiece, u8>,
    show_mapping_wizard: bool,
    wizard_selected_piece: Option<DrumPiece>,
    midi_rx: Option<Receiver<(u8, u8, u8)>>, // (status, note, velocity)
    midi_conn: Option<MidiInputConnection<()>>,
    // test tone lifetime
    test_stream: Option<cpal::Stream>,
    test_end: Option<Instant>,
    // Latency calibration
    calibrating: bool,
    calibration_trials_total: usize,
    calibration_trials_done: usize,
    calibration_offsets: Vec<f64>,
    next_beep_at: Option<Instant>,
    last_beep_time: Option<Instant>,
    awaiting_hit: bool,
    calibration_avg_ms: Option<f32>,
    // Metronome
    metronome_enabled: bool,
    metronome_gain: f32,
    // Tutor preferences
    tutor_hit_window_ms: f64,
    tutor_pre_roll_beats: u8,
    tutor_use_lesson_tempo: bool,
    // autosave debounce
    autosave_due: Option<Instant>,
}

impl SettingsPane {
    fn new() -> Self {
        let mut s = Self {
            audio_devices: vec!["OS Default".to_string()],
            selected_audio: Some(0),
            exclusive_mode: false,
            latency_ms: 10.0,
            main_volume: 0.8,
            app_sounds: true,
            auto_preview: true,
            high_contrast: false,
            play_streaks: true,
            new_keys_exp: true,
            midi_inputs: Vec::new(),
            selected_midi: None,
            mapping: default_mapping(),
            show_mapping_wizard: false,
            wizard_selected_piece: None,
            midi_rx: None,
            midi_conn: None,
            test_stream: None,
            test_end: None,
            calibrating: false,
            calibration_trials_total: 5,
            calibration_trials_done: 0,
            calibration_offsets: Vec::new(),
            next_beep_at: None,
            last_beep_time: None,
            awaiting_hit: false,
            calibration_avg_ms: None,
            metronome_enabled: true,
            metronome_gain: 0.6,
            tutor_hit_window_ms: 75.0,
            tutor_pre_roll_beats: 4,
            tutor_use_lesson_tempo: false,
            autosave_due: None,
        };
        s.refresh_audio_devices();
        s.refresh_midi();
        // Load persisted settings if available
        if let Ok(data) = load_settings() {
            s.apply_persisted(&data);
        }
        s
    }

    fn refresh_audio_devices(&mut self) {
        // Try cpal; if unavailable or no devices, keep OS Default only.
        #[allow(unused_mut)]
        let mut names: Vec<String> = Vec::new();
        #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
        {
            use cpal::traits::{DeviceTrait, HostTrait};
            for host_id in cpal::available_hosts() {
                if let Ok(host) = cpal::host_from_id(host_id) {
                    if let Ok(devices) = host.output_devices() {
                        for d in devices {
                            if let Ok(name) = d.name() {
                                names.push(name);
                            }
                        }
                    }
                }
            }
        }
        if names.is_empty() {
            self.audio_devices = vec!["OS Default".to_string()];
            self.selected_audio = Some(0);
        } else {
            self.audio_devices = names;
            self.selected_audio = Some(0);
        }
    }

    fn refresh_midi(&mut self) {
        match taal_tutor::midi::MidiManager::list_inputs() {
            Ok(list) => {
                self.midi_inputs = list;
                self.selected_midi = self.selected_midi.and_then(|i| if i < self.midi_inputs.len() { Some(i) } else { None });
            }
            Err(err) => {
                error!(?err, "failed to list MIDI inputs");
                self.midi_inputs.clear();
                self.selected_midi = None;
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.heading("Settings");
        ui.add_space(8.0);
        // Three columns similar to Melodics: Audio, Options, Connected Instruments
        egui::Grid::new("settings_grid").num_columns(3).striped(true).show(ui, |ui| {
            // Audio column
            ui.vertical(|ui| {
                ui.heading("Audio");
                ui.label("Selected audio device");
                egui::ComboBox::from_id_source("audio_device")
                    .selected_text(self.selected_audio.and_then(|i| self.audio_devices.get(i)).cloned().unwrap_or_else(|| "OS Default".into()))
                    .show_ui(ui, |ui| {
                        for (i, name) in self.audio_devices.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_audio, Some(i), name.clone());
                        }
                    });
                ui.checkbox(&mut self.exclusive_mode, "Use in exclusive mode");
                ui.label("Latency");
                ui.add(egui::Slider::new(&mut self.latency_ms, 1.0..=100.0).suffix(" ms"));
                ui.label("Main volume");
                ui.add(egui::Slider::new(&mut self.main_volume, 0.0..=1.0));
                if ui.button("Play test audio").clicked() { self.play_test_audio(); }
                if ui.button("Refresh audio devices").clicked() { self.refresh_audio_devices(); }
                if ui.button("Save settings").clicked() {
                    let _ = save_settings(&self.to_persisted());
                }
                ui.separator();
                ui.heading("Latency calibration");
                if ui.button(if self.calibrating {"Stop"} else {"Calibrate latency"}).clicked() { if self.calibrating { self.end_calibration(); } else { self.start_calibration(); } }
                if let Some(avg) = self.calibration_avg_ms { ui.label(format!("Estimated latency: {:.1} ms", avg)); }
            });
            ui.end_row();

            // Options column
            ui.vertical(|ui| {
                ui.heading("Options");
                toggle_row(ui, "App sounds", &mut self.app_sounds);
                toggle_row(ui, "Auto-preview", &mut self.auto_preview);
                toggle_row(ui, "High contrast mode", &mut self.high_contrast);
                toggle_row(ui, "Play screen note streaks", &mut self.play_streaks);
                toggle_row(ui, "New Keys Experience", &mut self.new_keys_exp);
            });
            ui.end_row();

            // Connected instruments column
            ui.vertical(|ui| {
                ui.heading("Connected instruments");
                if ui.button("Refresh MIDI Inputs").clicked() { self.refresh_midi(); }
                egui::ComboBox::from_id_source("midi_inputs_settings")
                    .selected_text(self.selected_midi.and_then(|i| self.midi_inputs.get(i)).map(|d| d.name.clone()).unwrap_or_else(|| "Select instrument".to_string()))
                    .show_ui(ui, |ui| {
                        for (i, dev) in self.midi_inputs.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_midi, Some(i), dev.name.clone());
                        }
                    });
                if ui.button("Map MIDI instrument").clicked() { self.open_mapping_wizard(); }
                if ui.button("Revert all mappings").clicked() { self.mapping = default_mapping(); }
            });
            ui.end_row();
        });

        if self.show_mapping_wizard {
            egui::Window::new("MIDI Mapping Wizard").collapsible(false).resizable(true).show(ui.ctx(), |ui| {
                self.mapping_wizard_ui(ui);
            });
        }

        // Stop test tone after deadline
        if let Some(end) = self.test_end {
            if Instant::now() >= end {
                self.test_stream = None;
                self.test_end = None;
            }
        }
        self.calibration_tick();
        self.tick_autosave();
    }

    fn to_persisted(&self) -> PersistedSettings {
        PersistedSettings {
            audio_device: self.selected_audio.and_then(|i| self.audio_devices.get(i)).cloned(),
            exclusive_mode: self.exclusive_mode,
            latency_ms: self.latency_ms,
            main_volume: self.main_volume,
            app_sounds: self.app_sounds,
            auto_preview: self.auto_preview,
            high_contrast: self.high_contrast,
            play_streaks: self.play_streaks,
            new_keys_exp: self.new_keys_exp,
            midi_device: self.selected_midi.and_then(|i| self.midi_inputs.get(i)).map(|d| d.name.clone()),
            mapping: self.mapping.clone(),
            metronome_enabled: self.metronome_enabled,
            metronome_gain: self.metronome_gain,
            tutor_hit_window_ms: self.tutor_hit_window_ms,
            tutor_pre_roll_beats: self.tutor_pre_roll_beats,
            tutor_use_lesson_tempo: self.tutor_use_lesson_tempo,
        }
    }

    fn apply_persisted(&mut self, data: &PersistedSettings) {
        self.exclusive_mode = data.exclusive_mode;
        self.latency_ms = data.latency_ms;
        self.main_volume = data.main_volume;
        self.app_sounds = data.app_sounds;
        self.auto_preview = data.auto_preview;
        self.high_contrast = data.high_contrast;
        self.play_streaks = data.play_streaks;
        self.new_keys_exp = data.new_keys_exp;
        self.mapping = data.mapping.clone();
        self.metronome_enabled = data.metronome_enabled;
        self.metronome_gain = data.metronome_gain;
        self.tutor_hit_window_ms = data.tutor_hit_window_ms;
        self.tutor_pre_roll_beats = data.tutor_pre_roll_beats;
        self.tutor_use_lesson_tempo = data.tutor_use_lesson_tempo;
        if let Some(name) = &data.audio_device {
            if let Some(i) = self.audio_devices.iter().position(|n| n == name) { self.selected_audio = Some(i); }
        }
        if let Some(name) = &data.midi_device {
            if let Some(i) = self.midi_inputs.iter().position(|d| &d.name == name) { self.selected_midi = Some(i); }
        }
    }

    fn apply_style(&self, ctx: &egui::Context) {
        if self.high_contrast {
            let mut style = (*ctx.style()).clone();
            style.visuals = egui::Visuals::dark();
            style.visuals.override_text_color = Some(egui::Color32::WHITE);
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
            ctx.set_style(style);
        } else {
            // Use default dark visuals
            let mut style = (*ctx.style()).clone();
            style.visuals = egui::Visuals::dark();
            ctx.set_style(style);
        }
    }

    fn play_test_audio(&mut self) {
        use cpal::traits::{DeviceTrait, StreamTrait};
        let sel_name = self.selected_audio.and_then(|i| self.audio_devices.get(i)).cloned();
        let device = match find_output_device_by_name(sel_name.as_deref()) { Some(d) => d, None => return };
        let cfg = match device.default_output_config() { Ok(c) => c, Err(_) => return };
        let sample_rate = cfg.sample_rate().0 as f32;
        let mut t: f32 = 0.0;
        let freq = 440.0f32;
        let dur_ms = 600.0f32;
        let total_samples = (dur_ms * sample_rate / 1000.0) as usize;
        let mut written: usize = 0;

        let result = match cfg.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(&cfg.config(), move |data: &mut [f32], _| {
                for frame in data.chunks_mut(cfg.channels() as usize) {
                    let s = if written < total_samples { (2.0*std::f32::consts::PI*freq*t/sample_rate).sin() * 0.2 } else { 0.0 };
                    for ch in frame { *ch = s; }
                    written = written.saturating_add(1);
                    t += 1.0;
                }
            }, |_| {}, None) ,
            cpal::SampleFormat::I16 => device.build_output_stream(&cfg.config(), move |data: &mut [i16], _| {
                for frame in data.chunks_mut(cfg.channels() as usize) {
                    let s = if written < total_samples { ((2.0*std::f32::consts::PI*freq*t/sample_rate).sin() * 0.2 * i16::MAX as f32) as i16 } else { 0 };
                    for ch in frame { *ch = s; }
                    written = written.saturating_add(1);
                    t += 1.0;
                }
            }, |_| {}, None) ,
            cpal::SampleFormat::U16 => device.build_output_stream(&cfg.config(), move |data: &mut [u16], _| {
                for frame in data.chunks_mut(cfg.channels() as usize) {
                    let center = (u16::MAX/2) as f32;
                    let s = if written < total_samples { ((2.0*std::f32::consts::PI*freq*t/sample_rate).sin() * 0.2 * center + center) as u16 } else { center as u16 };
                    for ch in frame { *ch = s; }
                    written = written.saturating_add(1);
                    t += 1.0;
                }
            }, |_| {}, None) ,
            _ => return,
        };
        if let Ok(stream) = result {
            let _ = stream.play();
            let deadline = Instant::now() + std::time::Duration::from_millis(dur_ms as u64 + 100);
            self.test_stream = Some(stream);
            self.test_end = Some(deadline);
        }
    }

    // Lightweight beep for metronome or calibration
    fn play_tone(&mut self, freq_hz: f32, dur_ms: u64, gain: f32) {
        use cpal::traits::{DeviceTrait, StreamTrait};
        let device = match find_output_device_by_name(self.selected_audio.and_then(|i| self.audio_devices.get(i)).map(|s| s.as_str())) { Some(d) => d, None => return };
        let cfg = match device.default_output_config() { Ok(c) => c, Err(_) => return };
        let sample_rate = cfg.sample_rate().0 as f32;
        let mut t: f32 = 0.0;
        let total_samples = (dur_ms as f32 * sample_rate / 1000.0) as usize;
        let mut written: usize = 0;
        let result = match cfg.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(&cfg.config(), move |data: &mut [f32], _| {
                for frame in data.chunks_mut(cfg.channels() as usize) {
                    let s = if written < total_samples { (2.0*std::f32::consts::PI*freq_hz*t/sample_rate).sin() * (0.5*gain).clamp(0.0, 1.0) } else { 0.0 };
                    for ch in frame { *ch = s; }
                    written = written.saturating_add(1);
                    t += 1.0;
                }
            }, |_| {}, None),
            _ => return,
        };
        if let Ok(stream) = result {
            let _ = stream.play();
            self.test_stream = Some(stream);
            self.test_end = Some(Instant::now() + std::time::Duration::from_millis(dur_ms + 50));
        }
    }

    // Simple synthesized drum voices per piece
    fn play_drum(&mut self, piece: DrumPiece, vel: u8, dur_ms: u64, base_gain: f32) {
        use cpal::traits::{DeviceTrait, StreamTrait};
        let device = match find_output_device_by_name(self.selected_audio.and_then(|i| self.audio_devices.get(i)).map(|s| s.as_str())) { Some(d) => d, None => return };
        let cfg = match device.default_output_config() { Ok(c) => c, Err(_) => return };
        let sr = cfg.sample_rate().0 as f32;
        let mut t: f32 = 0.0;
        let total_samples = (dur_ms as f32 * sr / 1000.0) as usize;
        let mut written: usize = 0;
        let gain = (base_gain * (vel as f32 / 127.0)).clamp(0.0, 1.0);
        // voice params
        let (kind, freq) = match piece {
            DrumPiece::Bass => (0, 55.0),
            DrumPiece::Snare | DrumPiece::CrossStick => (1, 220.0),
            DrumPiece::HiHatClosed => (1, 8000.0),
            DrumPiece::HiHatOpen => (1, 6000.0),
            DrumPiece::HighTom => (0, 180.0),
            DrumPiece::LowTom => (0, 140.0),
            DrumPiece::FloorTom => (0, 110.0),
            DrumPiece::Ride => (1, 4500.0),
            DrumPiece::Crash => (1, 5000.0),
            _ => (0, 220.0),
        };
        let result = match cfg.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(&cfg.config(), move |data: &mut [f32], _| {
                let mut rng = 0x12345678u32;
                for frame in data.chunks_mut(cfg.channels() as usize) {
                    let env = ((total_samples.saturating_sub(written)) as f32 / total_samples as f32).powf(2.0);
                    let sample = if kind == 0 {
                        // Sine with exponential decay
                        ((2.0*std::f32::consts::PI*freq*t/sr).sin()) * env * gain
                    } else {
                        // Simple noise burst
                        rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
                        let n = ((rng as f32 / u32::MAX as f32) * 2.0 - 1.0) * env * gain;
                        n
                    };
                    for ch in frame { *ch = sample; }
                    written = written.saturating_add(1);
                    t += 1.0;
                }
            }, |_| {}, None),
            cpal::SampleFormat::I16 => device.build_output_stream(&cfg.config(), move |data: &mut [i16], _| {
                let mut rng = 0x12345678u32;
                for frame in data.chunks_mut(cfg.channels() as usize) {
                    let env = ((total_samples.saturating_sub(written)) as f32 / total_samples as f32).powf(2.0);
                    let s = if kind == 0 {
                        ((2.0*std::f32::consts::PI*freq*t/sr).sin()) * env * gain
                    } else {
                        rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
                        ((rng as f32 / u32::MAX as f32) * 2.0 - 1.0) * env * gain
                    };
                    let i = (s * i16::MAX as f32) as i16;
                    for ch in frame { *ch = i; }
                    written = written.saturating_add(1);
                    t += 1.0;
                }
            }, |_| {}, None),
            cpal::SampleFormat::U16 => device.build_output_stream(&cfg.config(), move |data: &mut [u16], _| {
                let mut rng = 0x12345678u32;
                let center = (u16::MAX/2) as f32;
                for frame in data.chunks_mut(cfg.channels() as usize) {
                    let env = ((total_samples.saturating_sub(written)) as f32 / total_samples as f32).powf(2.0);
                    let s = if kind == 0 {
                        ((2.0*std::f32::consts::PI*freq*t/sr).sin()) * env * gain
                    } else {
                        rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
                        ((rng as f32 / u32::MAX as f32) * 2.0 - 1.0) * env * gain
                    };
                    let u = (s * center + center).clamp(0.0, u16::MAX as f32) as u16;
                    for ch in frame { *ch = u; }
                    written = written.saturating_add(1);
                    t += 1.0;
                }
            }, |_| {}, None),
            _ => return,
        };
        if let Ok(stream) = result {
            let _ = stream.play();
            self.test_stream = Some(stream);
            self.test_end = Some(Instant::now() + std::time::Duration::from_millis(dur_ms + 50));
        }
    }

    fn mark_dirty(&mut self) {
        self.autosave_due = Some(Instant::now() + std::time::Duration::from_millis(600));
    }

    fn tick_autosave(&mut self) {
        if let Some(due) = self.autosave_due {
            if Instant::now() >= due {
                let _ = save_settings(&self.to_persisted());
                self.autosave_due = None;
            }
        }
    }

    // Latency calibration state
    fn start_calibration(&mut self) {
        self.calibrating = true;
        self.calibration_avg_ms = None;
        self.calibration_trials_done = 0;
        self.calibration_offsets.clear();
        self.next_beep_at = Some(Instant::now() + std::time::Duration::from_millis(500));
        if self.midi_rx.is_none() {
            if let Some(name) = self.selected_midi.and_then(|i| self.midi_inputs.get(i)).map(|d| d.name.clone()) {
                if let Ok((conn, rx)) = open_midi_capture(&name) { self.midi_conn = Some(conn); self.midi_rx = Some(rx); }
            }
        }
    }

    fn end_calibration(&mut self) {
        self.calibrating = false;
        if !self.calibration_offsets.is_empty() {
            let avg = self.calibration_offsets.iter().copied().sum::<f64>() / self.calibration_offsets.len() as f64;
            self.latency_ms = avg as f32;
            self.calibration_avg_ms = Some(avg as f32);
        }
        self.next_beep_at = None;
        let _ = save_settings(&self.to_persisted());
    }

    fn calibration_tick(&mut self) {
        if !self.calibrating { return; }
        let now = Instant::now();
        // Schedule next beep
        if let Some(t) = self.next_beep_at { if now >= t && self.calibration_trials_done < self.calibration_trials_total { self.play_tone(1000.0, 80, self.main_volume); self.last_beep_time = Some(now); self.next_beep_at = None; self.awaiting_hit = true; } }
        // Read first incoming note after beep
        if self.awaiting_hit {
            if let Some(rx) = &self.midi_rx {
                if let Ok((_status, _note, _vel)) = rx.try_recv() {
                    if let Some(beep_t) = self.last_beep_time { let dt = now.duration_since(beep_t).as_secs_f64() * 1000.0; self.calibration_offsets.push(dt); self.calibration_trials_done += 1; self.awaiting_hit = false; self.next_beep_at = Some(now + std::time::Duration::from_millis(500)); }
                }
            }
            // Timeout if no hit in 2s
            if let Some(beep_t) = self.last_beep_time { if now.duration_since(beep_t) > std::time::Duration::from_secs(2) { self.calibration_trials_done += 1; self.awaiting_hit = false; self.next_beep_at = Some(now + std::time::Duration::from_millis(500)); } }
        }
        if self.calibration_trials_done >= self.calibration_trials_total { self.end_calibration(); }
    }

    fn open_mapping_wizard(&mut self) {
        self.show_mapping_wizard = true;
        self.wizard_selected_piece.get_or_insert(DrumPiece::Snare);
        // Open MIDI capture if possible
        if let Some(name) = self.selected_midi.and_then(|i| self.midi_inputs.get(i)).map(|d| d.name.clone()) {
            if let Ok((conn, rx)) = open_midi_capture(&name) {
                self.midi_conn = Some(conn);
                self.midi_rx = Some(rx);
            }
        }
    }

    fn mapping_wizard_ui(&mut self, ui: &mut Ui) {
        ui.label("Hit a pad on your kit or click Learn, then strike the selected piece.");
        ui.add_space(6.0);
        // Poll incoming MIDI
        if let Some(rx) = &self.midi_rx { while let Ok((status, note, _vel)) = rx.try_recv() { let is_on = status & 0xF0 == 0x90; if is_on { if let Some(piece) = self.wizard_selected_piece { self.mapping.insert(piece, note); } } } }

        // Visual kit layout
        let (rect, resp) = ui.allocate_at_least(egui::vec2(ui.available_width(), 260.0), egui::Sense::click());
        let painter = ui.painter_at(rect);
        let center = rect.center();
        let mut draw_pad = |offset: egui::Vec2, r: f32, piece: DrumPiece, label: &str| {
            let pos = center + offset;
            let selected = self.wizard_selected_piece == Some(piece);
            let fill = if selected { egui::Color32::from_rgb(255, 210, 0) } else { egui::Color32::from_gray(140) };
            painter.circle_filled(pos, r, fill);
            painter.circle_stroke(pos, r, egui::Stroke::new(2.0, egui::Color32::BLACK));
            painter.text(pos + egui::vec2(0.0, r + 14.0), egui::Align2::CENTER_TOP, format!("{}{}", label, self.mapping.get(&piece).map(|n| format!(" ({} )", n)).unwrap_or_default()), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::WHITE);
            if resp.clicked() { if let Some(p) = resp.interact_pointer_pos() { let d = (p - pos).length(); if d <= r { self.wizard_selected_piece = Some(piece); } } }
        };
        // Rough layout (screen coords)
        draw_pad(egui::vec2(-160.0, -60.0), 28.0, DrumPiece::Crash, "Crash");
        draw_pad(egui::vec2(160.0, -60.0), 28.0, DrumPiece::Ride, "Ride");
        draw_pad(egui::vec2(-90.0, -10.0), 24.0, DrumPiece::HiHatOpen, "Open HH");
        draw_pad(egui::vec2(-90.0, 25.0), 20.0, DrumPiece::HiHatClosed, "Closed HH");
        draw_pad(egui::vec2(-20.0, 20.0), 30.0, DrumPiece::Snare, "Snare");
        draw_pad(egui::vec2(40.0, 0.0), 26.0, DrumPiece::HighTom, "Hi Tom");
        draw_pad(egui::vec2(90.0, 20.0), 28.0, DrumPiece::LowTom, "Mid Tom");
        draw_pad(egui::vec2(130.0, 45.0), 30.0, DrumPiece::FloorTom, "Floor");
        draw_pad(egui::vec2(0.0, 70.0), 34.0, DrumPiece::Bass, "Kick");

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Learn").clicked() {
                // next incoming Note On will bind automatically (handled above)
            }
            if ui.button("Clear selected").clicked() {
                if let Some(p) = self.wizard_selected_piece { self.mapping.remove(&p); }
            }
            if ui.button("Close").clicked() {
                self.show_mapping_wizard = false;
                self.midi_conn = None; // drop connection
                self.midi_rx = None;
                let _ = save_settings(&self.to_persisted());
            }
        });
    }
}

fn toggle_row(ui: &mut Ui, label: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        let on = *value;
        let mut local = on;
        ui.selectable_value(&mut local, true, "ON");
        ui.selectable_value(&mut local, false, "OFF");
        *value = local;
    });
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct PersistedSettings {
    audio_device: Option<String>,
    exclusive_mode: bool,
    latency_ms: f32,
    main_volume: f32,
    app_sounds: bool,
    auto_preview: bool,
    high_contrast: bool,
    play_streaks: bool,
    new_keys_exp: bool,
    midi_device: Option<String>,
    mapping: HashMap<DrumPiece, u8>,
    metronome_enabled: bool,
    metronome_gain: f32,
    tutor_hit_window_ms: f64,
    tutor_pre_roll_beats: u8,
    tutor_use_lesson_tempo: bool,
}

fn settings_path() -> Option<std::path::PathBuf> {
    let base = dirs::config_dir()?;
    let dir = base.join("taal");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("settings.json"))
}

fn save_settings(data: &PersistedSettings) -> anyhow::Result<()> {
    if let Some(path) = settings_path() {
        let json = serde_json::to_string_pretty(data)?;
        std::fs::write(path, json)?;
    }
    Ok(())
}

fn load_settings() -> anyhow::Result<PersistedSettings> {
    let path = settings_path().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    let data = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

fn default_mapping() -> HashMap<DrumPiece, u8> {
    use DrumPiece::*;
    let mut m = HashMap::new();
    m.insert(Snare, 38);
    m.insert(Bass, 36);
    m.insert(HiHatClosed, 42);
    m.insert(HiHatOpen, 46);
    m.insert(Crash, 49);
    m.insert(Ride, 51);
    m.insert(HighTom, 50);
    m.insert(LowTom, 47);
    m.insert(FloorTom, 41);
    m
}

fn find_output_device_by_name(target: Option<&str>) -> Option<cpal::Device> {
    use cpal::traits::{HostTrait, DeviceTrait};
    let mut chosen: Option<cpal::Device> = None;
    for host_id in cpal::available_hosts() {
        if let Ok(host) = cpal::host_from_id(host_id) {
            if let Ok(mut devs) = host.output_devices() {
                while let Some(d) = devs.next() {
                    if let Ok(name) = d.name() {
                        if target.is_none() || Some(name.as_str()) == target { chosen = Some(d); break; }
                    }
                }
            }
        }
        if chosen.is_some() { break; }
    }
    if chosen.is_none() {
        let host = cpal::default_host();
        chosen = host.default_output_device();
    }
    chosen
}

fn open_midi_capture(name: &str) -> anyhow::Result<(MidiInputConnection<()>, Receiver<(u8, u8, u8)>)> {
    let mut input = MidiInput::new("taal-map")?;
    input.ignore(midir::Ignore::None);
    let ports = input.ports();
    let mut idx = None;
    for (i, p) in ports.iter().enumerate() {
        if let Ok(n) = input.port_name(p) { if n == name { idx = Some(i); break; } }
    }
    let port = ports.get(idx.ok_or_else(|| anyhow::anyhow!("midi port not found"))?).unwrap().clone();
    let (tx, rx) = mpsc::channel();
    let conn = input.connect(&port, "taal-map", move |_stamp, msg, _| {
        if msg.len() >= 3 { let _ = tx.send((msg[0], msg[1], msg[2])); }
        else if msg.len() >= 2 { let _ = tx.send((msg[0], msg[1], 100)); }
    }, ()).map_err(|e| anyhow::anyhow!(format!("midi connect error: {e:?}")))?;
    Ok((conn, rx))
}

struct MarketplacePane {
    client: MarketplaceClient,
    runtime: Arc<Runtime>,
    last_fetch: Option<Vec<String>>,
}

impl MarketplacePane {
    fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            client: MarketplaceClient::new("https://example.com"),
            runtime,
            last_fetch: None,
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.heading("Marketplace");
        if ui.button("Refresh").clicked() {
            let client = self.client.clone();
            let result = self.runtime.block_on(client.list_items());
            match result {
                Ok(items) => {
                    self.last_fetch = Some(items.iter().map(|item| item.title.clone()).collect());
                }
                Err(err) => {
                    error!(?err, "failed to fetch marketplace items");
                    self.last_fetch = Some(vec![format!("Error: {}", err)]);
                }
            }
        }
        if let Some(items) = &self.last_fetch {
            for item in items {
                ui.label(item);
            }
        } else {
            ui.label("Press refresh to view available lessons.");
        }
    }
}
