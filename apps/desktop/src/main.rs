use std::sync::Arc;

use eframe::{egui, egui::Ui};
use taal_ui::theme as ui_theme;
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
use std::time::Instant;
use taal_ui::icons;

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
    // UI polish for app bar tabs
    tab_underline_x: f32,
    tab_underline_w: f32,
}

impl DesktopApp {
    fn new(rt: Arc<Runtime>) -> Self {
        Self {
            active_tab: ActiveTab::Extractor,
            extractor: ExtractorPane::new(),
            tutor: TutorPane::new(),
            marketplace: MarketplacePane::new(rt),
            settings: SettingsPane::new(),
            tab_underline_x: 0.0,
            tab_underline_w: 0.0,
        }
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme preferences (e.g., high-contrast) each frame.
        self.settings.apply_style(ctx);
        // Keep Tutor in sync with current settings
        self.tutor.sync_settings(&self.settings);
        let is_dark = ctx.style().visuals.dark_mode;
        egui::TopBottomPanel::top("top_bar").frame(egui::Frame::default().fill(if is_dark { egui::Color32::from_rgb(20,22,27) } else { egui::Color32::from_rgb(246,248,250) })).show(ctx, |ui| {
            let rect = ui.max_rect();
            // Subtle gradient approximation: two translucent stripes
            let grad_top = if ui.visuals().dark_mode { egui::Color32::from_rgba_unmultiplied(255,255,255,6) } else { egui::Color32::from_rgba_unmultiplied(0,0,0,8) };
            let grad_bot = if ui.visuals().dark_mode { egui::Color32::from_rgba_unmultiplied(0,0,0,12) } else { egui::Color32::from_rgba_unmultiplied(0,0,0,16) };
            ui.painter().rect_filled(egui::Rect::from_min_max(rect.left_top(), egui::pos2(rect.right(), rect.top()+8.0)), 0.0, grad_top);
            ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom()-8.0), rect.right_bottom()), 0.0, grad_bot);

            // Tabs with icon + label and animated underline; icons tinted for contrast and animated on hover
            ui.horizontal(|ui| {
                let tint = icons::default_tint(ui);
                let mut active_rect = None;
                let mut render_tab = |ui: &mut egui::Ui, target: ActiveTab, label: &str, icon: &str| {
                    let mut r = egui::Rect::NAN;
                    ui.horizontal(|ui| {
                        if let Some(id) = icons::icon_tex(ui.ctx(), icon) {
                            let resp_icon = ui.add(egui::ImageButton::new((id, egui::vec2(16.0,16.0))).tint(tint));
                            // Hover/press animation: brighten + slight scale
                            let scaled = icons::hover_scale(ui, resp_icon.hovered(), resp_icon.is_pointer_button_down_on(), &format!("tab:{}:icon", label), 1.06);
                            let tint2 = icons::hover_tint(ui, tint, resp_icon.hovered(), resp_icon.is_pointer_button_down_on(), &format!("tab:{}:tint", label));
                            let _ = ui.put(resp_icon.rect, egui::Image::new((id, egui::vec2(16.0*scaled, 16.0*scaled))).tint(tint2));
                            r = r.union(resp_icon.rect);
                            if resp_icon.clicked() { self.active_tab = target; }
                        }
                        let resp_lbl = ui.selectable_label(self.active_tab == target, label);
                        r = if r.is_finite() { r.union(resp_lbl.rect) } else { resp_lbl.rect };
                        if resp_lbl.clicked() { self.active_tab = target; }
                    });
                    if self.active_tab == target { active_rect = Some(r); }
                };
                render_tab(ui, ActiveTab::Extractor, "Studio", "sliders");
                ui.add_space(6.0);
                render_tab(ui, ActiveTab::Tutor, "Practice", "drum");
                ui.add_space(6.0);
                render_tab(ui, ActiveTab::Marketplace, "Marketplace", "shopping-cart");
                ui.add_space(6.0);
                render_tab(ui, ActiveTab::Settings, "Settings", "settings");
                if let Some(r) = active_rect {
                    let accent = if ui.visuals().dark_mode { egui::Color32::from_rgb(0,180,255) } else { egui::Color32::from_rgb(255,140,66) };
                    let y = rect.bottom() - 2.0;
                    self.tab_underline_x = egui::lerp(self.tab_underline_x..=r.left(), 0.2);
                    self.tab_underline_w = egui::lerp(self.tab_underline_w..=r.width(), 0.2);
                    ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(self.tab_underline_x, y), egui::vec2(self.tab_underline_w, 2.0)), 1.0, accent);
                }
            });
        });

        match self.active_tab {
            ActiveTab::Extractor => {
                // Studio drawers
                egui::SidePanel::left("studio_tools").default_width(260.0).resizable(false).show(ctx, |ui| {
                    ui.heading("Tools");
                    ui.add_space(6.0);
                    self.extractor.ui_tools(ui);
                });
                egui::SidePanel::right("studio_inspector").default_width(260.0).resizable(false).show(ctx, |ui| {
                    ui.heading("Inspector");
                    ui.add_space(6.0);
                    self.extractor.ui_inspector(ui, &mut self.settings);
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.extractor.ui(ui, &mut self.tutor, &mut self.settings);
                });
                // Bottom transport dock for Studio
                egui::TopBottomPanel::bottom("studio_transport").show(ctx, |ui| {
                    self.extractor.transport_ui(ui, &mut self.settings);
                });
            }
            ActiveTab::Tutor => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.tutor.ui(ui, &mut self.settings);
                });
                // Bottom transport dock for Practice
                egui::TopBottomPanel::bottom("practice_transport").show(ctx, |ui| {
                    self.tutor.transport_ui(ui, &mut self.settings);
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
    drag_handle: Option<LoopHandle>,
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
            drag_handle: None,
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

    fn ui_tools(&mut self, ui: &mut Ui) {
        ui.label("Piece").on_hover_text("Select drum piece for new notes");
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
                ] { ui.selectable_value(&mut self.selected_piece, piece, format!("{:?}", piece)); }
            });
        ui.add_space(8.0);
        ui.label("Velocity").on_hover_text("MIDI velocity (1–127) for new notes");
        ui.add(egui::Slider::new(&mut self.selected_velocity, 1..=127));
        ui.add_space(8.0);
        ui.label("Grid beats").on_hover_text("Total beats in chart timeline");
        ui.add(egui::Slider::new(&mut self.grid_total_beats, 4.0..=256.0).logarithmic(true));
        ui.add_space(8.0);
        ui.label("Snap").on_hover_text("Note placement grid resolution");
        egui::ComboBox::from_id_source("snap_select")
            .selected_text(format!("1/{}", self.snap_den))
            .show_ui(ui, |ui| {
                for d in [4_u32, 8, 16, 32] { ui.selectable_value(&mut self.snap_den, d, format!("1/{}", d)); }
            });
        ui.add_space(8.0);
        ui.toggle_value(&mut self.lane_mode, "Lane editor").on_hover_text("Compose per instrument in lanes (Snare first)");
    }

    fn ui_inspector(&mut self, ui: &mut Ui, _settings: &mut SettingsPane) {
        ui.horizontal(|ui| {
            if ui.button("Quantize sel").on_hover_text("Quantize selected notes to current snap").clicked() { self.quantize_selected(); }
            if ui.button("Quantize all").on_hover_text("Quantize all notes to current snap").clicked() { self.status_message = Some("__DO_QUANTIZE_ALL__".into()); }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Undo").on_hover_text("Undo last change (Ctrl+Z)").clicked() { self.undo(); }
            if ui.button("Redo").on_hover_text("Redo (Ctrl+Shift+Z)").clicked() { self.redo(); }
        });
        // Selection details could go here later
    }

    fn create_new_chart(&mut self) {
        let tempo = TempoMap::constant(120.0).unwrap();
        let lesson = LessonDescriptor::new("new","Untitled Chart","",1,tempo,vec![]);
        self.editor = Some(NotationEditor::new(lesson));
        self.status_message = Some("Created new empty chart".to_string());
        self.selected_event = None;
        self.playhead = 0.0;
    }

    fn load_sample(&mut self) {
        let tempo = TempoMap::constant(100.0).unwrap();
        let mut events = Vec::new();
        for i in 0..8 { let beat = i as f64; events.push(NotatedEvent::new( DrumEvent::new(beat, DrumPiece::Bass, 110, DrumArticulation::Normal), Duration::milliseconds(500) )); events.push(NotatedEvent::new( DrumEvent::new(beat + 0.5, DrumPiece::Snare, 100, DrumArticulation::Normal), Duration::milliseconds(500) )); }
        let lesson = LessonDescriptor::new("sample","Sample Groove","Bass on beats, snare on offbeats",1,tempo,events);
        self.editor = Some(NotationEditor::new(lesson));
        self.status_message = Some("Loaded sample transcription".to_string());
    }

    fn open_chart(&mut self) {
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

    fn start_transcribe(&mut self, tutor: &mut TutorPane) {
        match self.transcribe() {
            Ok(lesson) => {
                tutor.load_lesson(lesson.clone());
                self.editor = Some(NotationEditor::new(lesson.clone()));
                self.status_message = Some(format!("Transcribed {} events", lesson.notation.len()));
            }
            Err(err) => { error!(?err, "failed to transcribe"); self.status_message = Some(format!("Error: {}", err)); }
        }
    }

    fn transport_ui(&mut self, ui: &mut Ui, settings: &mut SettingsPane) {
        egui::Frame::none().inner_margin(egui::Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let tint = icons::default_tint(ui);
                // Play/Pause with icon + hover animation
                if let Some(tex) = icons::icon_tex(ui.ctx(), if self.playing { "pause" } else { "play" }) {
                    let resp = ui.add(egui::ImageButton::new((tex, egui::vec2(16.0,16.0))).tint(tint)).on_hover_text("Start/stop preview");
                    let size = 16.0 * icons::hover_scale(ui, resp.hovered(), resp.is_pointer_button_down_on(), "studio:play", 1.08);
                    let tint2 = icons::hover_tint(ui, tint, resp.hovered(), resp.is_pointer_button_down_on(), "studio:play");
                    let _ = ui.put(resp.rect, egui::Image::new((tex, egui::vec2(size,size))).tint(tint2));
                    if resp.clicked() {
                        self.playing = !self.playing;
                        if self.playing { self.last_tick = Some(std::time::Instant::now()); self.next_click_beat = self.playhead.ceil(); }
                    }
                } else if ui.button(if self.playing {"Pause"} else {"Play"}).clicked() {
                    self.playing = !self.playing;
                    if self.playing { self.last_tick = Some(std::time::Instant::now()); self.next_click_beat = self.playhead.ceil(); }
                }
                ui.separator();
                ui.label("BPM");
                ui.add(egui::Slider::new(&mut self.bpm, 40.0..=220.0).show_value(false));
                // Show integer BPM next to slider
                ui.label(format!("{}", self.bpm.round() as i32));
                ui.separator();
                // Loop toggle with icon
                if let Some(tex) = icons::icon_tex(ui.ctx(), "repeat") {
                    if ui.add(egui::SelectableLabel::new(self.loop_enabled, "")).on_hover_text("Loop between Start and End beats").clicked() { self.loop_enabled = !self.loop_enabled; }
                    ui.add(egui::Image::new((tex, egui::vec2(16.0,16.0))).tint(tint));
                } else { ui.toggle_value(&mut self.loop_enabled, "Loop"); }
                ui.label("Start"); ui.add(egui::DragValue::new(&mut self.loop_start).speed(0.1));
                ui.label("End"); ui.add(egui::DragValue::new(&mut self.loop_end).speed(0.1));
                if ui.button("Reset").on_hover_text("Reset playhead to start").clicked() { self.playhead = self.loop_start.min(0.0); }
                ui.separator();
                // Record icon toggle
                if let Some(tex) = icons::icon_tex(ui.ctx(), "record") {
                    let resp = ui.add(egui::ImageButton::new((tex, egui::vec2(16.0,16.0))).tint(if self.record_enabled { tint } else { ui.visuals().widgets.noninteractive.weak_bg_fill }))
                        .on_hover_text("Record MIDI");
                    let size = 16.0 * icons::hover_scale(ui, resp.hovered(), resp.is_pointer_button_down_on(), "studio:record", 1.08);
                    let tint2 = if self.record_enabled { icons::hover_tint(ui, tint, resp.hovered(), resp.is_pointer_button_down_on(), "studio:record") } else { ui.visuals().widgets.noninteractive.weak_bg_fill };
                    let _ = ui.put(resp.rect, egui::Image::new((tex, egui::vec2(size,size))).tint(tint2));
                    if resp.clicked() { self.record_enabled = !self.record_enabled; }
                } else { ui.toggle_value(&mut self.record_enabled, "Record MIDI"); }
                ui.separator();
                if let Some(tex) = icons::icon_tex(ui.ctx(), "metronome") {
                    let enabled = settings.metronome_enabled;
                    let resp = ui.add(egui::ImageButton::new((tex, egui::vec2(16.0,16.0))).tint(if enabled { tint } else { ui.visuals().widgets.noninteractive.weak_bg_fill })).on_hover_text("Metronome");
                    if resp.clicked() { settings.metronome_enabled = !settings.metronome_enabled; }
                } else {
                    ui.toggle_value(&mut settings.metronome_enabled, "Metronome");
                }
                ui.add(egui::Slider::new(&mut settings.metronome_gain, 0.0..=1.0).show_value(false));
                ui.label(format!("{}%", (settings.metronome_gain*100.0).round() as i32));
            });
        });
    }

    fn ui(&mut self, ui: &mut Ui, tutor: &mut TutorPane, settings: &mut SettingsPane) {
        // Top section – title + quick actions toolbar with icons
        ui.horizontal(|ui| {
            ui.heading("Chart Studio");
            // Chart chip with dropdown actions
            if let Some(editor) = &self.editor {
                let title = &editor.lesson().title;
                ui.menu_button(format!("{}  ▾", title), |ui| {
                    if ui.button("Open…").clicked() { self.open_chart(); ui.close_menu(); }
                    if ui.button("Import MusicXML…").clicked() {
                        if let Some(path) = FileDialog::new().add_filter("MusicXML", &["musicxml","xml"]).pick_file() {
                            match std::fs::read_to_string(&path) {
                                Ok(text) => match taal_domain::io::MusicXmlImporter::import_str(&text) {
                                    Ok(lesson) => { self.editor = Some(NotationEditor::new(lesson)); self.status_message = Some(format!("Imported: {}", path.display())); },
                                    Err(err) => { self.status_message = Some(format!("Import failed: {}", err)); }
                                },
                                Err(err) => { self.status_message = Some(format!("Read failed: {}", err)); }
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Close chart").clicked() { self.editor = None; ui.close_menu(); }
                });
            }
            ui.add_space(12.0);
            ui.group(|ui| {
                let tint = icons::default_tint(ui);
                ui.horizontal(|ui| {
                    // New Chart
                    if let Some(tex) = icons::icon_tex(ui.ctx(), "file-plus") {
                        let b = ui.add(egui::ImageButton::new((tex, egui::vec2(18.0,18.0))).tint(tint)).on_hover_text("New chart");
                        let size = 18.0 * icons::hover_scale(ui, b.hovered(), b.is_pointer_button_down_on(), "toolbar:new", 1.08);
                        let tint2 = icons::hover_tint(ui, tint, b.hovered(), b.is_pointer_button_down_on(), "toolbar:new");
                        let _ = ui.put(b.rect, egui::Image::new((tex, egui::vec2(size,size))).tint(tint2));
                        if b.clicked() { self.create_new_chart(); }
                        ui.label("New");
                    } else { if ui.button("New").clicked() { self.create_new_chart(); } }
                    ui.add_space(10.0);
                    // Load Sample
                    if let Some(tex) = icons::icon_tex(ui.ctx(), "music") {
                        let b = ui.add(egui::ImageButton::new((tex, egui::vec2(18.0,18.0))).tint(tint)).on_hover_text("Load sample groove");
                        let size = 18.0 * icons::hover_scale(ui, b.hovered(), b.is_pointer_button_down_on(), "toolbar:sample", 1.08);
                        let tint2 = icons::hover_tint(ui, tint, b.hovered(), b.is_pointer_button_down_on(), "toolbar:sample");
                        let _ = ui.put(b.rect, egui::Image::new((tex, egui::vec2(size,size))).tint(tint2));
                        if b.clicked() { self.load_sample(); }
                        ui.label("Sample");
                    } else { if ui.button("Sample").clicked() { self.load_sample(); } }
                    ui.add_space(10.0);
                    // Open Chart
                    if let Some(tex) = icons::icon_tex(ui.ctx(), "folder-open") {
                        let b = ui.add(egui::ImageButton::new((tex, egui::vec2(18.0,18.0))).tint(tint)).on_hover_text("Open chart");
                        let size = 18.0 * icons::hover_scale(ui, b.hovered(), b.is_pointer_button_down_on(), "toolbar:open", 1.08);
                        let tint2 = icons::hover_tint(ui, tint, b.hovered(), b.is_pointer_button_down_on(), "toolbar:open");
                        let _ = ui.put(b.rect, egui::Image::new((tex, egui::vec2(size,size))).tint(tint2));
                        if b.clicked() { self.open_chart(); }
                        ui.label("Open");
                    } else { if ui.button("Open").clicked() { self.open_chart(); } }
                    ui.add_space(10.0);
                    // Transcribe
                    if let Some(tex) = icons::icon_tex(ui.ctx(), "waveform") {
                        let b = ui.add(egui::ImageButton::new((tex, egui::vec2(18.0,18.0))).tint(tint)).on_hover_text("Transcribe audio to chart");
                        let size = 18.0 * icons::hover_scale(ui, b.hovered(), b.is_pointer_button_down_on(), "toolbar:transcribe", 1.08);
                        let tint2 = icons::hover_tint(ui, tint, b.hovered(), b.is_pointer_button_down_on(), "toolbar:transcribe");
                        let _ = ui.put(b.rect, egui::Image::new((tex, egui::vec2(size,size))).tint(tint2));
                        if b.clicked() { self.start_transcribe(tutor); }
                        ui.label("Transcribe");
                    } else { if ui.button("Transcribe").clicked() { self.start_transcribe(tutor); } }
                    ui.add_space(10.0);
                    // Save / Export dropdown
                    ui.menu_button("Save/Export ▾", |ui| {
                        if ui.button("Save JSON…").clicked() {
                            if let Some(editor) = &self.editor {
                                if let Some(path) = FileDialog::new().set_file_name("chart.json").save_file() {
                                    match serde_json::to_string_pretty(editor.lesson()) {
                                        Ok(s) => { let _ = std::fs::write(&path, s); }
                                        Err(err) => { self.status_message = Some(format!("Failed to save: {err}")); }
                                    }
                                }
                            }
                            ui.close_menu();
                        }
                        if ui.button("Export MIDI…").clicked() {
                            if let Some(editor) = &self.editor {
                                if let Some(path) = FileDialog::new().set_file_name("chart.mid").save_file() {
                                    let exp = taal_domain::io::MidiExporter;
                                    match exp.export(editor.lesson(), taal_domain::io::ExportFormat::Midi) {
                                        Ok(bytes) => { let _ = std::fs::write(&path, bytes); }
                                        Err(err) => { self.status_message = Some(format!("Export failed: {}", err)); }
                                    }
                                }
                            }
                            ui.close_menu();
                        }
                        if ui.button("Export MusicXML…").clicked() {
                            if let Some(editor) = &self.editor {
                                if let Some(path) = FileDialog::new().set_file_name("chart.musicxml").save_file() {
                                    let exp = taal_domain::io::SimpleMusicXmlExporter;
                                    match exp.export(editor.lesson(), taal_domain::io::ExportFormat::MusicXml) {
                                        Ok(bytes) => { let _ = std::fs::write(&path, bytes); }
                                        Err(err) => { self.status_message = Some(format!("Export failed: {}", err)); }
                                    }
                                }
                            }
                            ui.close_menu();
                        }
                    });
                });
            });
        });
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
            if ui.button("Transcribe").clicked() { self.start_transcribe(tutor); }
        });
        // Toolbar actions moved to the top app bar.

        if let Some(message) = &self.status_message {
            ui.label(message);
        }
        // (Editor Tools moved to left/right drawers)

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

            // Zoom/pan/loop interactions + loop handle dragging
            let response = if self.lane_mode {
                draw_studio_lanes(ui, editor.lesson(), self.view_start, self.view_span, Some(self.playhead), if self.loop_enabled { Some((self.loop_start, self.loop_end)) } else { None }, &mut self.lane_solo, &mut self.lane_mute)
            } else {
                editor.draw_with_timeline(ui, self.view_start, self.view_span, self.waveform.as_deref(), Some(self.playhead), if self.loop_enabled { Some((self.loop_start, self.loop_end)) } else { None })
            };
            // Loop handles in ruler (top of response rect)
            let margin = 8.0f32; let top = response.rect.top() + margin; let left = if self.lane_mode { response.rect.left() + 120.0 } else { response.rect.left() } ; let right = response.rect.right() - if self.lane_mode { 8.0 } else { 0.0 } ;
            let width = (right - left).max(1.0);
            if self.loop_enabled {
                let to_x = |beat: f64| left + width * (((beat - self.view_start) / self.view_span).clamp(0.0, 1.0) as f32);
                let x0 = to_x(self.loop_start); let x1 = to_x(self.loop_end);
                let handle_sz = egui::vec2(10.0, 14.0);
                let r0 = egui::Rect::from_min_size(egui::pos2(x0 - 5.0, top - handle_sz.y - 2.0), handle_sz);
                let r1 = egui::Rect::from_min_size(egui::pos2(x1 - 5.0, top - handle_sz.y - 2.0), handle_sz);
                ui.painter().rect_filled(r0, 2.0, egui::Color32::from_rgb(60,130,255));
                ui.painter().rect_filled(r1, 2.0, egui::Color32::from_rgb(60,130,255));
                let h0 = ui.interact(r0, egui::Id::new("loop_handle_start"), egui::Sense::click_and_drag());
                let h1 = ui.interact(r1, egui::Id::new("loop_handle_end"), egui::Sense::click_and_drag());
                if h0.drag_started() { self.drag_handle = Some(LoopHandle::Start); }
                if h1.drag_started() { self.drag_handle = Some(LoopHandle::End); }
                if let Some(handle) = self.drag_handle {
                    if ui.input(|i| i.pointer.any_down()) {
                        if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                            let t = ((p.x - left) / width).clamp(0.0, 1.0) as f64;
                            let beat = self.view_start + t * self.view_span;
                            match handle { LoopHandle::Start => self.loop_start = beat.min(self.loop_end), LoopHandle::End => self.loop_end = beat.max(self.loop_start) }
                        }
                    } else { self.drag_handle = None; }
                }
            }
            // Mouse wheel to zoom
            let scroll = ui.input(|i| i.scroll_delta.y);
            if response.hovered() && scroll.abs() > 0.0 {
                let factor = (1.0 - scroll * 0.001).clamp(0.5, 1.5);
                self.view_span = (self.view_span * factor as f64).clamp(1.0, self.grid_total_beats);
            }
            // Middle button drag to pan
            if response.dragged_by(egui::PointerButton::Middle) {
                if let Some(_pos) = response.interact_pointer_pos() {
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
                (response.rect.left() + 120.0, response.rect.right() - 8.0)
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

    #[allow(dead_code)]
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
    // metronome click tracking
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
    // Practice settings cached from Settings
    practice_match_window_pct: f32,
    practice_on_time_pct: f32,
    practice_match_cap_ms: f32,
    practice_on_time_cap_ms: f32,
    countdown_each_loop: bool,
    loop_total: u8,
    loops_done: u8,
    end_beat: f64,
    tempo_scale_default: f32,
    mode: PracticeUIMode,
    // A/B loop region
    loop_use_region: bool,
    loop_a: f64,
    loop_b: f64,
    drag_handle: Option<LoopHandle>,
    // Review
    review_active: bool,
    bpm_initialized_from_settings: bool,
    // FX
    ripples: Vec<Ripple>,
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
            next_click_beat: 0.0,
            pre_roll_beats: 4,
            pre_roll_active: false,
            pre_roll_remaining: 0.0,
            elapsed_secs: 0.0,
            freeze_playhead: false,
            statuses: Vec::new(),
            practice_match_window_pct: 12.5,
            practice_on_time_pct: 7.5,
            practice_match_cap_ms: 75.0,
            practice_on_time_cap_ms: 40.0,
            countdown_each_loop: false,
            loop_total: 2,
            loops_done: 0,
            end_beat: 0.0,
            tempo_scale_default: 0.8,
            mode: PracticeUIMode::FreePlay,
            loop_use_region: false,
            loop_a: 0.0,
            loop_b: 0.0,
            drag_handle: None,
            review_active: false,
            bpm_initialized_from_settings: false,
            ripples: Vec::new(),
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
        // Compute end beat as the last expected note (fallback to 0.0)
        self.end_beat = self.session.as_ref()
            .and_then(|s| s.lesson.notation.iter().map(|e| e.event.beat).reduce(f64::max))
            .unwrap_or(0.0);
        self.loops_done = 0;
        self.review_active = false;
        self.bpm_initialized_from_settings = false;
    }

    fn ui(&mut self, ui: &mut Ui, settings: &mut SettingsPane) {
        ui.heading("Practice");
        // Selected MIDI device is configured in Settings
        self.poll_midi();
        // Deferred actions across UI sections to avoid double-borrows
        let mut do_open_chart = false;
        let mut do_import_xml = false;
        let mut do_close_chart = false;

        let mut simulate_hit_clicked = false;
        if let Some(session) = &mut self.session {
            let chart_title = session.lesson.title.clone();
            // Mode selector
            ui.horizontal(|ui| {
                ui.label("Mode:");
                ui.selectable_value(&mut self.mode, PracticeUIMode::FreePlay, "Free Play");
                ui.selectable_value(&mut self.mode, PracticeUIMode::Test, "Test");
                ui.add_space(12.0);
                // Chart chip with dropdown for Open/Import/Close
                ui.menu_button(format!("{}  ▾", chart_title), |ui| {
                    if ui.button("Open chart…").clicked() { do_open_chart = true; ui.close_menu(); }
                    if ui.button("Import MusicXML…").clicked() { do_import_xml = true; ui.close_menu(); }
                    if ui.button("Close chart").clicked() { do_close_chart = true; ui.close_menu(); }
                });
            });
            ui.add_space(4.0);
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
                        self.loops_done = 0;
                        self.loop_total = match self.mode { PracticeUIMode::FreePlay => u8::MAX, PracticeUIMode::Test => settings.practice_default_loop_count };
                        // Jump to loop start if region enabled
                        if self.loop_use_region {
                            self.playhead = self.loop_a.min(self.loop_b);
                            self.elapsed_secs = 0.0;
                        }
                        self.review_active = false;
                    }
                }
                // Use lesson tempo toggle
                let r_use = ui.checkbox(&mut settings.tutor_use_lesson_tempo, "Use lesson tempo");
                if r_use.changed() { settings.mark_dirty(); }
                if settings.tutor_use_lesson_tempo {
                    self.bpm = session.lesson.default_tempo.events()[0].bpm;
                }
                ui.label("BPM");
                let r_bpm = ui.add_enabled(!settings.tutor_use_lesson_tempo, egui::Slider::new(&mut self.bpm, 40.0..=240.0));
                if r_bpm.changed() { settings.mark_dirty(); }
                if ui.button("Reset").clicked() { self.playhead = 0.0; self.loops_done = 0; }
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

            // A/B loop controls
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.loop_use_region, "Loop region (A/B)");
                ui.label("A:"); ui.add(egui::DragValue::new(&mut self.loop_a).speed(0.1));
                if ui.button("Set A here").clicked() { self.loop_a = self.playhead; }
                ui.label("B:"); ui.add(egui::DragValue::new(&mut self.loop_b).speed(0.1));
                if ui.button("Set B here").clicked() { self.loop_b = self.playhead; }
                if self.loop_use_region && self.loop_b < self.loop_a { std::mem::swap(&mut self.loop_a, &mut self.loop_b); }
            });

            // Chart actions moved to the title chip above.

            // Advance playhead
            if self.playing {
                let now = std::time::Instant::now();
                if let Some(last) = self.last_tick {
                    let dt = now.duration_since(last).as_secs_f64();
                    // compute beats advanced based on source (lesson tempo or fixed bpm)
                    let beats_advanced = if settings.tutor_use_lesson_tempo {
                        // approximate using instantaneous bpm at current elapsed time
                        let bpm_now = session.lesson.default_tempo.bpm_at(self.elapsed_secs);
                        dt * (bpm_now as f64) / 60.0
                    } else {
                        dt * (self.bpm as f64) / 60.0
                    };
                    if self.pre_roll_active {
                        // Pre-roll counts in seconds, independent of BPM.
                        if settings.metronome_enabled && settings.app_sounds {
                            while self.next_click_beat < self.pre_roll_remaining {
                                settings.play_tone( if (self.next_click_beat as i64) % 4 == 0 { 1000.0 } else { 800.0 }, 70, settings.main_volume * settings.metronome_gain);
                                self.next_click_beat += 1.0; // tick every second
                            }
                        }
                        self.pre_roll_remaining -= dt; // seconds
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
                        // Compute match window (beats) using % of beat with ms cap
                        let bpm_used = if settings.tutor_use_lesson_tempo { session.lesson.default_tempo.bpm_at(self.elapsed_secs) } else { self.bpm } as f64;
                        let pct_beats = (self.practice_match_window_pct as f64) / 100.0; // e.g., 0.125 beats at 12.5%
                        let cap_beats = (self.practice_match_cap_ms as f64 / 1000.0) * (bpm_used) / 60.0;
                        let beat_window = pct_beats.min(cap_beats);
                        let head = self.playhead;
                        if self.mode != PracticeUIMode::FreePlay {
                            for (i, ev) in session.lesson.notation.iter().enumerate() {
                                if self.statuses.get(i).copied().flatten().is_none() && ev.event.beat + beat_window < head {
                                    if let Some(s) = self.statuses.get_mut(i) { *s = Some(HitLabel::Missed); }
                                }
                            }
                        }
                        // Determine end boundary (A/B region or full chart)
                        let end_boundary = if self.loop_use_region { self.loop_b.max(self.loop_a) } else { self.end_beat };
                        // Looping logic at end of boundary
                        if self.playhead >= end_boundary {
                            self.loops_done = self.loops_done.saturating_add(1);
                            if self.loops_done < self.loop_total || self.mode == PracticeUIMode::FreePlay {
                                self.playhead = if self.loop_use_region { self.loop_a.min(self.loop_b) } else { 0.0 };
                                self.elapsed_secs = 0.0;
                                self.next_click_beat = 0.0;
                                if self.countdown_each_loop {
                                    self.pre_roll_active = true;
                                    self.pre_roll_beats = settings.tutor_pre_roll_beats;
                                    self.pre_roll_remaining = self.pre_roll_beats as f64;
                                }
                                // reset statuses to allow another pass
                                for s in &mut self.statuses { *s = None; }
                            } else {
                                // Stop and show end state
                                self.playing = false;
                                self.last_tick = None;
                                // Prepare review summary
                                let bpm_used = self.bpm as f64;
                                let spb = 60.0 / bpm_used;
                                let report = self.scoring.score_with_spb(&session.lesson, &self.hits, spb);
                                let mut stats = session.lesson.stats.clone();
                                let analytics = SessionAnalytics::new(report.clone());
                                analytics.update_statistics(&mut stats);
                                self.analytics = Some(analytics);
                                self.review_active = true;
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
            let loop_region = if self.loop_use_region { Some((self.loop_a.min(self.loop_b), self.loop_b.max(self.loop_a))) } else { None };
            let rect = draw_highway(ui, &session.lesson, self.playhead, &self.statuses, start, window_span, self.freeze_playhead, loop_region, Some(&mut self.ripples), settings.reduced_motion, settings.playhead_glow);
            // A/B loop handles in Practice ruler (mirror Studio behavior)
            if self.loop_use_region {
                let margin = 8.0f32; let top = rect.top() + margin;
                let left = rect.left() + 120.0; let right = rect.right() - 10.0; // match draw_highway layout
                let width = (right - left).max(1.0);
                let to_x = |beat: f64| left + width * (((beat - start) / window_span).clamp(0.0, 1.0) as f32);
                let from_x = |x: f32| {
                    let t = ((x - left) / width).clamp(0.0, 1.0) as f64;
                    start + t * window_span
                };
                let x0 = to_x(self.loop_a.min(self.loop_b)); let x1 = to_x(self.loop_b.max(self.loop_a));
                let handle_sz = egui::vec2(10.0, 14.0);
                let r0 = egui::Rect::from_min_size(egui::pos2(x0 - 5.0, top - handle_sz.y - 2.0), handle_sz);
                let r1 = egui::Rect::from_min_size(egui::pos2(x1 - 5.0, top - handle_sz.y - 2.0), handle_sz);
                ui.painter().rect_filled(r0, 2.0, egui::Color32::from_rgb(60,130,255));
                ui.painter().rect_filled(r1, 2.0, egui::Color32::from_rgb(60,130,255));
                let h0 = ui.interact(r0, egui::Id::new("practice_loop_handle_start"), egui::Sense::click_and_drag());
                let h1 = ui.interact(r1, egui::Id::new("practice_loop_handle_end"), egui::Sense::click_and_drag());
                if h0.drag_started() { self.drag_handle = Some(LoopHandle::Start); }
                if h1.drag_started() { self.drag_handle = Some(LoopHandle::End); }
                if let Some(handle) = self.drag_handle {
                    if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                            match handle {
                                LoopHandle::Start => { self.loop_a = from_x(pos.x); }
                                LoopHandle::End => { self.loop_b = from_x(pos.x); }
                            }
                        }
                        ui.ctx().request_repaint();
                    } else {
                        self.drag_handle = None;
                    }
                }
            }
            // Countdown overlay during pre-roll (big number + soft background)
            if self.pre_roll_active {
                let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("tutor_pre_roll")));
                let screen = ui.ctx().screen_rect();
                let center = screen.center();
                let remaining = self.pre_roll_remaining.ceil() as i32;
                let text = if remaining > 0 { format!("{}", remaining) } else { "Go!".to_string() };
                let bg = if ui.visuals().dark_mode { egui::Color32::from_rgba_unmultiplied(0,0,0,160) } else { egui::Color32::from_rgba_unmultiplied(240,244,248,220) };
                painter.circle_filled(center, 60.0, bg);
                let ring = egui::Color32::from_rgb(255,210,0);
                painter.circle_stroke(center, 60.0, egui::Stroke::new(2.0, ring));
                let font = egui::FontId::proportional(64.0);
                let col = if ui.visuals().dark_mode { egui::Color32::WHITE } else { egui::Color32::from_rgb(30,30,30) };
                painter.text(center, egui::Align2::CENTER_CENTER, text, font, col);
            }
            // Prepare per-instrument stats before opening review window (avoids borrow issues)
            let per_piece_snapshot: Option<std::collections::HashMap<DrumPiece, (u32,u32,u32,u32)>> = if self.review_active {
                use std::collections::HashMap;
                let mut per_piece: HashMap<DrumPiece, (u32,u32,u32,u32)> = HashMap::new();
                for (i, ev) in session.lesson.notation.iter().enumerate() {
                    let ent = per_piece.entry(ev.event.piece).or_insert((0,0,0,0));
                    match self.statuses.get(i).and_then(|s| *s) {
                        Some(HitLabel::OnTime) => ent.0 += 1,
                        Some(HitLabel::Early) => ent.1 += 1,
                        Some(HitLabel::Late) => ent.2 += 1,
                        Some(HitLabel::Missed) => ent.3 += 1,
                        None => {},
                    }
                }
                Some(per_piece)
            } else { None };

            // Review overlay at end of run — centered card with consistent ordering
            if self.review_active {
                egui::Area::new("review_center").anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ui.ctx(), |ui| {
                    egui::Frame::window(ui.style()).rounding(egui::Rounding::same(8.0)).show(ui, |ui| {
                        ui.set_min_width(320.0);
                        ui.set_max_width(460.0);
                        if let Some(analytics) = &self.analytics {
                            let pct = (analytics.report.accuracy * 100.0).round() as i32;
                            ui.heading("Review Summary");
                            ui.separator();
                            ui.label(format!("Accuracy: {}%", pct));
                            let mood = if pct >= 90 { "Excellent!" } else if pct >= 75 { "Great work!" } else if pct >= 50 { "Nice progress — keep going!" } else { "You’ve got this — try once more!" };
                            ui.label(mood);
                        }
                        if let Some(per_piece) = &per_piece_snapshot {
                            ui.separator();
                            ui.label("Per‑instrument:");
                            for piece in ordered_lanes() {
                                if let Some((on, early, late, missed)) = per_piece.get(&piece) {
                                    ui.label(format!("{:?}: on {} · early {} · late {} · missed {}", piece, on, early, late, missed));
                                }
                            }
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Retry").clicked() {
                                self.playhead = if self.loop_use_region { self.loop_a.min(self.loop_b) } else { 0.0 };
                                self.elapsed_secs = 0.0;
                                self.statuses.iter_mut().for_each(|s| *s = None);
                                self.hits.clear();
                                self.loops_done = 0;
                                self.review_active = false;
                            }
                            if ui.button("Close").clicked() { self.review_active = false; }
                        });
                    });
                });
            }
            // Legend
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                legend_dot(ui, egui::Color32::from_rgb(80,200,120), "On time");
                legend_dot(ui, egui::Color32::from_rgb(240,160,60), "Late");
                legend_dot(ui, egui::Color32::from_rgb(120,170,255), "Early");
                legend_dot(ui, egui::Color32::from_rgb(220,80,80), "Missed");
                legend_dot(ui, egui::Color32::from_gray(150), "Not yet played");
            });

            ui.label(format!("Mode: {:?}", session.mode));
            ui.label(format!(
                "Progress: {}/{}",
                session.current_index,
                session.lesson.notation.len()
            ));
            if ui.button("Simulate Hit").clicked() { simulate_hit_clicked = true; }
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
            // Process deferred chart actions outside of the borrow of `session`
            // Note: We use separate scopes above to avoid nested mutable borrows of `self`.
            // The actions are performed here once `session` borrow has ended.
            // (open/import may replace the current lesson; close resets the pane.)
            // SAFETY: ui interactions above set these flags for this frame only.
        } else {
            ui.label("Load a chart to begin practice.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Import Chart").on_hover_text("Load a saved chart (.json)").clicked() { do_open_chart = true; }
                if ui.button("Import MusicXML").on_hover_text("Import a MusicXML (.musicxml/.xml)").clicked() { do_import_xml = true; }
                if ui.button("Load Sample").clicked() {
                    // simple bass/snare groove like Studio sample
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
                    self.load_lesson(lesson);
                }
            });
            ui.add_space(8.0);
            ui.label("Tip: Use Studio to transcribe audio into a chart, then switch back to Practice.");
        }
        if simulate_hit_clicked { self.handle_live_hit(DrumPiece::Snare, 100); }
        // Handle deferred actions (works both when session is Some or None)
        if do_close_chart { self.session = None; self.hits.clear(); self.analytics = None; }
        if do_open_chart {
            if let Some(path) = FileDialog::new().add_filter("Chart", &["json"]).pick_file() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(lesson) = serde_json::from_str::<LessonDescriptor>(&text) { self.load_lesson(lesson); }
                }
            }
        }
        if do_import_xml {
            if let Some(path) = FileDialog::new().add_filter("MusicXML", &["musicxml", "xml"]).pick_file() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(lesson) = taal_domain::io::MusicXmlImporter::import_str(&text) { self.load_lesson(lesson); }
                }
            }
        }
    }

    fn transport_ui(&mut self, ui: &mut Ui, settings: &mut SettingsPane) {
        egui::Frame::none().inner_margin(egui::Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let tint = icons::default_tint(ui);
                if let Some(tex) = icons::icon_tex(ui.ctx(), if self.playing { "pause" } else { "play" }) {
                    let resp = ui.add(egui::ImageButton::new((tex, egui::vec2(16.0,16.0))).tint(tint)).on_hover_text("Play/Pause");
                    let size = 16.0 * icons::hover_scale(ui, resp.hovered(), resp.is_pointer_button_down_on(), "practice:play", 1.08);
                    let tint2 = icons::hover_tint(ui, tint, resp.hovered(), resp.is_pointer_button_down_on(), "practice:play");
                    let _ = ui.put(resp.rect, egui::Image::new((tex, egui::vec2(size,size))).tint(tint2));
                    if resp.clicked() {
                        self.playing = !self.playing;
                        if self.playing {
                            self.last_tick = Some(std::time::Instant::now());
                            self.pre_roll_active = true;
                            self.pre_roll_beats = settings.tutor_pre_roll_beats;
                            self.pre_roll_remaining = self.pre_roll_beats as f64;
                            self.next_click_beat = 0.0;
                            self.loops_done = 0;
                        }
                    }
                } else if ui.button(if self.playing {"Pause"} else {"Play"}).clicked() {
                    self.playing = !self.playing;
                    if self.playing {
                        self.last_tick = Some(std::time::Instant::now());
                        self.pre_roll_active = true;
                        self.pre_roll_beats = settings.tutor_pre_roll_beats;
                        self.pre_roll_remaining = self.pre_roll_beats as f64;
                        self.next_click_beat = 0.0;
                        self.loops_done = 0;
                    }
                }
                ui.separator();
                let r_use = ui.toggle_value(&mut settings.tutor_use_lesson_tempo, "Use lesson tempo");
                if r_use.changed() { settings.mark_dirty(); }
                ui.label("BPM");
                ui.add_enabled(!settings.tutor_use_lesson_tempo, egui::Slider::new(&mut self.bpm, 40.0..=240.0).show_value(false));
                ui.label(format!("{}", self.bpm.round() as i32));
                ui.separator();
                if let Some(tex) = icons::icon_tex(ui.ctx(), "metronome") {
                    let enabled = settings.metronome_enabled;
                    let resp = ui.add(egui::ImageButton::new((tex, egui::vec2(16.0,16.0))).tint(if enabled { tint } else { ui.visuals().widgets.noninteractive.weak_bg_fill })).on_hover_text("Metronome");
                    let size = 16.0 * icons::hover_scale(ui, resp.hovered(), resp.is_pointer_button_down_on(), "studio:metronome", 1.08);
                    let base = if enabled { tint } else { ui.visuals().widgets.noninteractive.weak_bg_fill };
                    let tint2 = icons::hover_tint(ui, base, resp.hovered(), resp.is_pointer_button_down_on(), "studio:metronome");
                    let _ = ui.put(resp.rect, egui::Image::new((tex, egui::vec2(size,size))).tint(tint2));
                    if resp.clicked() { settings.metronome_enabled = !settings.metronome_enabled; settings.mark_dirty(); }
                } else {
                    ui.toggle_value(&mut settings.metronome_enabled, "Metronome");
                }
                ui.add(egui::Slider::new(&mut settings.metronome_gain, 0.0..=1.0).show_value(false));
                ui.label(format!("{}%", (settings.metronome_gain*100.0).round() as i32));
                ui.separator();
                ui.label("Pre‑roll");
                ui.add(egui::Slider::new(&mut settings.tutor_pre_roll_beats, 0..=8));
                ui.toggle_value(&mut self.freeze_playhead, "Freeze playhead");
            });
        });
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
        // Practice settings
        self.practice_match_window_pct = settings.practice_match_window_pct;
        self.practice_on_time_pct = settings.practice_on_time_pct;
        self.practice_match_cap_ms = settings.practice_match_cap_ms;
        self.practice_on_time_cap_ms = settings.practice_on_time_cap_ms;
        self.countdown_each_loop = settings.practice_countdown_each_loop;
        self.loop_total = settings.practice_default_loop_count;
        self.tempo_scale_default = settings.practice_tempo_scale_default;
        // Initialize BPM from practice tempo scaling once per loaded lesson
        if let Some(session) = &self.session {
            if !settings.tutor_use_lesson_tempo && !self.playing && self.playhead == 0.0 && !self.bpm_initialized_from_settings {
                let base = session.lesson.default_tempo.events()[0].bpm;
                self.bpm = (base * self.tempo_scale_default).clamp(40.0, 240.0);
                self.bpm_initialized_from_settings = true;
            }
            // Update end_beat if not set
            if self.end_beat <= 0.0 {
                self.end_beat = session.lesson.notation.iter().map(|e| e.event.beat).fold(0.0, f64::max);
            }
        }
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
        // Convert latency to beats using current BPM
        let latency_beats = (self.latency_ms as f64) / 1000.0 * (self.bpm as f64) / 60.0;
        let hit_beat = (self.playhead - latency_beats).max(0.0);
        if let Some(session) = &mut self.session {
            // Find nearest unmatched expected for this piece
            let mut best: Option<(usize, f64)> = None;
            for (i, ev) in session.lesson.notation.iter().enumerate() {
                if ev.event.piece != piece { continue; }
                if self.statuses.get(i).copied().flatten().is_some() { continue; }
                let d = (ev.event.beat - hit_beat).abs();
                let pct_beats = (self.practice_match_window_pct as f64) / 100.0;
                let cap_beats = (self.practice_match_cap_ms as f64 / 1000.0) * (self.bpm as f64) / 60.0;
                let beat_window = pct_beats.min(cap_beats);
                if d < beat_window && best.map(|(_, bd)| d < bd).unwrap_or(true) { best = Some((i, d)); }
            }
            if let Some((idx, _)) = best {
                let expected = session.lesson.notation[idx].event.beat;
                let delta = hit_beat - expected;
                let ontime_beats = ((self.practice_on_time_pct as f64) / 100.0)
                    .min((self.practice_on_time_cap_ms as f64 / 1000.0) * (self.bpm as f64) / 60.0);
                let label = if delta.abs() < ontime_beats { HitLabel::OnTime } else if delta > 0.0 { HitLabel::Late } else { HitLabel::Early };
                if let Some(s) = self.statuses.get_mut(idx) { *s = Some(label); }
                let ev = DrumEvent::new(hit_beat, piece, vel, DrumArticulation::Normal);
                session.register_hit(&ev);
                self.hits.push(ev);
                self.ripples.push(Ripple { beat: hit_beat, piece, start: Instant::now() });
            }
        }
    }

}

fn draw_highway(ui: &mut Ui, lesson: &LessonDescriptor, playhead: f64, statuses: &[Option<HitLabel>], start: f64, window_span: f64, _freeze_playhead: bool, loop_region: Option<(f64,f64)>, fx: Option<&mut Vec<Ripple>>, reduced_motion: bool, playhead_glow: bool) -> egui::Rect {
    let lanes = ordered_lanes();
        let lane_h = 28.0f32;
        let margin = 8.0f32;
        let width = ui.available_width();
        let height = lanes.len() as f32 * lane_h + margin * 2.0;
        let (rect, _resp) = ui.allocate_at_least(egui::vec2(width, height), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let left = rect.left() + 120.0; // space for lane labels
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
            let label_col = ui.visuals().widgets.noninteractive.fg_stroke.color;
            painter.text(egui::pos2(rect.left() + 6.0, y), egui::Align2::LEFT_CENTER, format!("{:?}", piece), egui::TextStyle::Body.resolve(ui.style()), label_col);
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

        // Loop region highlight
        if let Some((a, b)) = loop_region {
            let x0 = to_x(a.max(start));
            let x1 = to_x(b.min(end));
            if x1 > x0 {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x0, top),
                        egui::pos2(x1, rect.bottom())
                    ),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 0, 24)
                );
            }
        }

        // Draw expected notes and status colors
        for (i, ev) in lesson.notation.iter().enumerate() {
            if ev.event.beat < start || ev.event.beat > end { continue; }
            let lane = lanes.iter().position(|p| *p == ev.event.piece).unwrap_or(0);
            let y = top + lane as f32 * lane_h + lane_h * 0.5;
            let x = to_x(ev.event.beat);
            let color = match statuses.get(i).and_then(|s| *s) {
                Some(HitLabel::OnTime) => egui::Color32::from_rgb(80, 200, 120),
                Some(HitLabel::Late) => egui::Color32::from_rgb(240, 160, 60),
                Some(HitLabel::Early) => egui::Color32::from_rgb(120, 170, 255),
                Some(HitLabel::Missed) => egui::Color32::from_rgb(220, 80, 80),
                None => egui::Color32::from_gray(150), // Not yet played
            };
            painter.circle_filled(egui::pos2(x, y), 7.5, color);
        }

        // Playhead glow + line
        let x = to_x(playhead);
        // Base line uses accent color for consistency
        let accent = ui.visuals().selection.bg_fill;
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(2.0, accent));
        // Optional soft glow
        if playhead_glow && !reduced_motion {
            let c1 = accent.linear_multiply(0.25);
            let c2 = accent.linear_multiply(0.12);
            painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(6.0, c1));
            painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(10.0, c2));
        }

        // Hit ripples
        if let Some(rips) = fx {
            if !reduced_motion {
                let now = Instant::now();
                rips.retain(|r| now.duration_since(r.start).as_millis() < 240);
                for r in rips.iter() {
                    // Position from beat + piece lane
                    let x = to_x(r.beat);
                    let lane = lanes.iter().position(|p| *p == r.piece).unwrap_or(0);
                    let y = top + lane as f32 * lane_h + lane_h * 0.5;
                    let t = now.duration_since(r.start).as_secs_f32() / 0.18;
                    let p = t.min(1.0);
                    let rr = egui::lerp(6.0..=26.0, p);
                    let a = ((1.0 - p) * 0.65 * 255.0) as u8;
                    let col = egui::Color32::from_rgba_unmultiplied(255, 140, 66, a);
                    painter.circle_stroke(egui::pos2(x, y), rr, egui::Stroke::new(2.0, col));
                }
            } else {
                rips.clear();
            }
        }
        rect
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
    solo: &mut HashSet<DrumPiece>,
    mute: &mut HashSet<DrumPiece>,
) -> egui::Response {
    let lanes = studio_lanes();
    let lane_h = 26.0f32;
    let margin = 8.0f32;
    let height = lanes.len() as f32 * lane_h + margin * 2.0;
    let (rect, response) = ui.allocate_at_least(egui::vec2(ui.available_width(), height), egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let left = rect.left() + 120.0;
    let right = rect.right() - 8.0;
    let top = rect.top() + margin;

    // lane backgrounds and labels with M/S toggles near labels
    for (row, piece) in lanes.iter().enumerate() {
        let lane_top = top + row as f32 * lane_h;
        let bg = if row % 2 == 0 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 6) } else { egui::Color32::TRANSPARENT };
        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(left, lane_top), egui::vec2(right - left, lane_h)), 0.0, bg);
        let y = lane_top + lane_h * 0.5;
        // lane label — theme-aware color for readability in light/dark
        let label_col = ui.visuals().widgets.noninteractive.fg_stroke.color;
        painter.text(egui::pos2(rect.left() + 6.0, y), egui::Align2::LEFT_CENTER, format!("{:?}", piece), egui::TextStyle::Body.resolve(ui.style()), label_col);
        // Mute/Solo pills (horizontal inside the gutter, no overlap)
        let w = 16.0; let h = 12.0; let gap = 4.0;
        let start_x = left - (w*2.0 + gap + 6.0); // always within 120px gutter
        let m_rect = egui::Rect::from_min_size(egui::pos2(start_x, y - h*0.5), egui::vec2(w, h));
        let s_rect = egui::Rect::from_min_size(egui::pos2(start_x + w + gap, y - h*0.5), egui::vec2(w, h));
        // M pill
        let m_active = mute.contains(piece);
        let m_fill = if m_active { egui::Color32::from_rgb(60,130,255) } else { egui::Color32::from_gray(60) };
        painter.rect_filled(m_rect, 3.0, m_fill);
        painter.text(m_rect.center(), egui::Align2::CENTER_CENTER, "M", egui::TextStyle::Small.resolve(ui.style()), egui::Color32::WHITE);
        let m_resp = ui.interact(m_rect, egui::Id::new((row, "M")), egui::Sense::click());
        m_resp.clone().on_hover_text("Mute this lane. If any lanes are soloed, only soloed lanes will play.");
        if m_resp.clicked() {
            if !mute.insert(*piece) { mute.remove(piece); }
            solo.remove(piece);
        }
        // S pill
        let s_active = solo.contains(piece);
        let s_fill = if s_active { egui::Color32::from_rgb(60,130,255) } else { egui::Color32::from_gray(60) };
        painter.rect_filled(s_rect, 3.0, s_fill);
        painter.text(s_rect.center(), egui::Align2::CENTER_CENTER, "S", egui::TextStyle::Small.resolve(ui.style()), egui::Color32::WHITE);
        let s_resp = ui.interact(s_rect, egui::Id::new((row, "S")), egui::Sense::click());
        s_resp.clone().on_hover_text("Solo this lane. When any lane is soloed, all non‑solo lanes are muted.");
        if s_resp.clicked() {
            if !solo.insert(*piece) { solo.remove(piece); }
            mute.remove(piece);
        }
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

    // playhead (clean line, no glow)
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
    let left = rect.left() + 120.0;
    let right = rect.right() - 8.0;
    let top = rect.top() + margin;
    if pos.x < left || pos.x > right { return None; }
    let row = ((pos.y - top) / lane_h).floor() as isize;
    if row < 0 { return None; }
    lanes.get(row as usize).cloned()
}

#[derive(Clone, Copy, Debug)]
enum HitLabel { OnTime, Late, Early, Missed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PracticeUIMode { FreePlay, Test }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsSection { Audio, Midi, Practice, Appearance, Accessibility }

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
    // Appearance
    ui_theme: ui_theme::ThemeMode,
    reduced_motion: bool,
    // Accent & glass
    accent_choice: AccentChoice,
    glass_mode: bool,
    playhead_glow: bool,
    // fonts are ensured per-frame via ui_theme::ensure_inter
    // Settings UI
    section: SettingsSection,
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
    // Practice (Tutor) settings
    practice_match_window_pct: f32,
    practice_on_time_pct: f32,
    practice_match_cap_ms: f32,
    practice_on_time_cap_ms: f32,
    practice_countdown_each_loop: bool,
    practice_default_loop_count: u8,
    practice_tempo_scale_default: f32,
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
            ui_theme: ui_theme::ThemeMode::DarkNeon,
            reduced_motion: false,
            accent_choice: AccentChoice::Blue,
            glass_mode: false,
            playhead_glow: false,
            section: SettingsSection::Audio,
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
            practice_match_window_pct: 12.5,
            practice_on_time_pct: 7.5,
            practice_match_cap_ms: 75.0,
            practice_on_time_cap_ms: 40.0,
            practice_countdown_each_loop: false,
            practice_default_loop_count: 2,
            practice_tempo_scale_default: 0.8,
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
        ui.horizontal(|ui| {
            // Left navigation
            ui.vertical(|ui| {
                ui.set_min_width(140.0);
                for (sec, label) in [
                    (SettingsSection::Audio, "Audio"),
                    (SettingsSection::Midi, "MIDI"),
                    (SettingsSection::Practice, "Practice"),
                    (SettingsSection::Appearance, "Appearance"),
                    (SettingsSection::Accessibility, "Accessibility"),
                ] {
                    let selected = self.section == sec;
                    if ui.selectable_label(selected, label).clicked() { self.section = sec; }
                }
            });
            ui.separator();
            // Right content
            ui.vertical(|ui| {
                match self.section {
                    SettingsSection::Audio => self.ui_audio_card(ui),
                    SettingsSection::Midi => self.ui_midi_card(ui),
                    SettingsSection::Practice => self.ui_practice_card(ui),
                    SettingsSection::Appearance => { self.ui_appearance_card(ui); ui.add_space(10.0); self.ui_display_motion_card(ui); ui.add_space(10.0); self.ui_options_card(ui); },
                    SettingsSection::Accessibility => self.ui_accessibility_card(ui),
                }
            });
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

    fn ui_audio_card(&mut self, ui: &mut Ui) {
        let t = ui_theme::theme(self.ui_theme).tokens;
        egui::Frame::group(ui.style())
            .fill(t.neutral_surface)
            .rounding(egui::Rounding::same(8.0))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.heading("Audio");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Device");
                    egui::ComboBox::from_id_source("audio_device")
                        .selected_text(self.selected_audio.and_then(|i| self.audio_devices.get(i)).cloned().unwrap_or_else(|| "OS Default".into()))
                        .show_ui(ui, |ui| {
                            for (i, name) in self.audio_devices.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_audio, Some(i), name.clone());
                            }
                        });
                    ui.add_space(12.0);
                    ui.toggle_value(&mut self.exclusive_mode, "Exclusive mode");
                });
                ui.add_space(8.0);
                ui.columns(2, |cols| {
                    cols[0].label("Latency");
                    cols[0].add(egui::Slider::new(&mut self.latency_ms, 1.0..=100.0).suffix(" ms"));
                    cols[1].label("Main volume");
                    cols[1].add(egui::Slider::new(&mut self.main_volume, 0.0..=1.0).show_value(false));
                    cols[1].label(format!("{}%", (self.main_volume*100.0).round() as i32));
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Play test").clicked() { self.play_test_audio(); }
                    if ui.button("Refresh devices").clicked() { self.refresh_audio_devices(); }
                    if ui.button("Save").clicked() { let _ = save_settings(&self.to_persisted()); }
                });
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                ui.heading("Latency calibration");
                ui.add_space(6.0);
                if ui.button(if self.calibrating {"Stop"} else {"Calibrate latency"}).clicked() { if self.calibrating { self.end_calibration(); } else { self.start_calibration(); } }
                if let Some(avg) = self.calibration_avg_ms { ui.label(format!("Estimated latency: {:.1} ms", avg)); }
            });
    }

    fn ui_midi_card(&mut self, ui: &mut Ui) {
        let t = ui_theme::theme(self.ui_theme).tokens;
        egui::Frame::group(ui.style())
            .fill(t.neutral_surface)
            .rounding(egui::Rounding::same(8.0))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.heading("MIDI");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Refresh inputs").clicked() { self.refresh_midi(); }
                    egui::ComboBox::from_id_source("midi_inputs_settings")
                        .selected_text(self.selected_midi.and_then(|i| self.midi_inputs.get(i)).map(|d| d.name.clone()).unwrap_or_else(|| "Select instrument".to_string()))
                        .show_ui(ui, |ui| {
                            for (i, dev) in self.midi_inputs.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_midi, Some(i), dev.name.clone());
                            }
                        });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Mapping wizard").clicked() { self.open_mapping_wizard(); }
                    if ui.button("Revert mappings").clicked() { self.mapping = default_mapping(); }
                });
            });
    }

    fn ui_practice_card(&mut self, ui: &mut Ui) {
        let t = ui_theme::theme(self.ui_theme).tokens;
        egui::Frame::group(ui.style())
            .fill(t.neutral_surface)
            .rounding(egui::Rounding::same(8.0))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.heading("Practice");
                ui.add_space(8.0);
                ui.label("Hit windows (% of beat)");
                ui.add_space(4.0);
                ui.columns(2, |cols| {
                    cols[0].label("Match");
                    cols[0].add(egui::Slider::new(&mut self.practice_match_window_pct, 2.5..=25.0).suffix(" %"));
                    cols[1].label("On-time");
                    cols[1].add(egui::Slider::new(&mut self.practice_on_time_pct, 2.5..=20.0).suffix(" %"));
                });
                ui.add_space(6.0);
                ui.label("Caps (ms)");
                ui.columns(2, |cols| {
                    cols[0].label("Match");
                    cols[0].add(egui::Slider::new(&mut self.practice_match_cap_ms, 20.0..=150.0).suffix(" ms"));
                    cols[1].label("On-time");
                    cols[1].add(egui::Slider::new(&mut self.practice_on_time_cap_ms, 10.0..=100.0).suffix(" ms"));
                });
                ui.add_space(8.0);
                ui.toggle_value(&mut self.practice_countdown_each_loop, "Countdown each loop");
                ui.add_space(6.0);
                ui.columns(2, |cols| {
                    cols[0].label("Test loops before review");
                    cols[0].add(egui::Slider::new(&mut self.practice_default_loop_count, 1..=10));
                    cols[1].label("Default tempo scaling");
                    cols[1].add(egui::Slider::new(&mut self.practice_tempo_scale_default, 0.5..=1.25).suffix("×"));
                });
            });
    }

    fn ui_appearance_card(&mut self, ui: &mut Ui) {
        let t = ui_theme::theme(self.ui_theme).tokens;
        egui::Frame::group(ui.style())
            .fill(t.neutral_surface)
            .rounding(egui::Rounding::same(8.0))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.heading("Appearance");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.ui_theme, ui_theme::ThemeMode::DarkNeon, "Dark (Studio)");
                    ui.selectable_value(&mut self.ui_theme, ui_theme::ThemeMode::LightNeumorphic, "Light (Practice)");
                });
                ui.add_space(8.0);
                // Accent swatches + custom picker
                ui.horizontal_wrapped(|ui| {
                    ui.label("Accent:");
                    ui.selectable_value(&mut self.accent_choice, AccentChoice::Blue, "Blue");
                    ui.selectable_value(&mut self.accent_choice, AccentChoice::Orange, "Orange");
                    ui.selectable_value(&mut self.accent_choice, AccentChoice::Green, "Green");
                    ui.selectable_value(&mut self.accent_choice, AccentChoice::Pink, "Neon Pink");
                    ui.selectable_value(&mut self.accent_choice, AccentChoice::Purple, "Neon Purple");
                    // Custom color picker
                    let mut custom = match self.accent_choice { AccentChoice::Custom(c) => c, _ => self.accent_choice.color() };
                    if ui.add(egui::Button::new("Custom…")).on_hover_text("Pick a custom accent color").clicked() {
                        // no-op: color button below opens the picker
                    }
                    if egui::color_picker::color_edit_button_srgba(ui, &mut custom, egui::color_picker::Alpha::Opaque).changed() {
                        self.accent_choice = AccentChoice::Custom(custom);
                    }
                });
                ui.add_space(8.0);
                // Surfaces
                ui.heading("Surfaces");
                ui.add_space(6.0);
                ui.toggle_value(&mut self.glass_mode, "Glass surfaces (Dark only)")
                    .on_hover_text("Make panels slightly translucent for a DAW-like look");
            });
    }

    fn ui_display_motion_card(&mut self, ui: &mut Ui) {
        let t = ui_theme::theme(self.ui_theme).tokens;
        egui::Frame::group(ui.style())
            .fill(t.neutral_surface)
            .rounding(egui::Rounding::same(8.0))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.heading("Display & Motion");
                ui.add_space(6.0);
                ui.toggle_value(&mut self.playhead_glow, "Playhead glow")
                    .on_hover_text("Soft glow around the playhead line (Practice)");
                ui.toggle_value(&mut self.reduced_motion, "Reduced motion");
                ui.toggle_value(&mut self.high_contrast, "High contrast");
            });
    }

    fn ui_options_card(&mut self, ui: &mut Ui) {
        let t = ui_theme::theme(self.ui_theme).tokens;
        egui::Frame::group(ui.style())
            .fill(t.neutral_surface)
            .rounding(egui::Rounding::same(8.0))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.heading("Sound & Behavior");
                ui.add_space(6.0);
                ui.toggle_value(&mut self.app_sounds, "App sounds");
                ui.toggle_value(&mut self.auto_preview, "Auto preview");
                ui.toggle_value(&mut self.play_streaks, "Play screen note streaks");
                ui.toggle_value(&mut self.new_keys_exp, "New Keys Experience");
            });
    }

    fn ui_accessibility_card(&mut self, ui: &mut Ui) {
        ui.heading("Accessibility");
        ui.label("Font scaling and colorblind palettes — coming soon.");
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
            practice_match_window_pct: Some(self.practice_match_window_pct),
            practice_on_time_pct: Some(self.practice_on_time_pct),
            practice_match_cap_ms: Some(self.practice_match_cap_ms),
            practice_on_time_cap_ms: Some(self.practice_on_time_cap_ms),
            practice_countdown_each_loop: Some(self.practice_countdown_each_loop),
            practice_default_loop_count: Some(self.practice_default_loop_count),
            practice_tempo_scale_default: Some(self.practice_tempo_scale_default),
            ui_theme: Some(match self.ui_theme { ui_theme::ThemeMode::DarkNeon => "dark".into(), ui_theme::ThemeMode::LightNeumorphic => "light".into() }),
            reduced_motion: Some(self.reduced_motion),
            accent_choice: Some(self.accent_choice.as_str().to_string()),
            glass_mode: Some(self.glass_mode),
            playhead_glow: Some(self.playhead_glow),
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
        // Practice defaults with fallbacks for older settings files
        self.practice_match_window_pct = data.practice_match_window_pct.unwrap_or(12.5);
        self.practice_on_time_pct = data.practice_on_time_pct.unwrap_or(7.5);
        self.practice_match_cap_ms = data.practice_match_cap_ms.unwrap_or(75.0);
        self.practice_on_time_cap_ms = data.practice_on_time_cap_ms.unwrap_or(40.0);
        self.practice_countdown_each_loop = data.practice_countdown_each_loop.unwrap_or(false);
        self.practice_default_loop_count = data.practice_default_loop_count.unwrap_or(2);
        self.practice_tempo_scale_default = data.practice_tempo_scale_default.unwrap_or(0.8);
        // Appearance
        if let Some(t) = &data.ui_theme {
            self.ui_theme = if t == "light" { ui_theme::ThemeMode::LightNeumorphic } else { ui_theme::ThemeMode::DarkNeon };
        }
        self.reduced_motion = data.reduced_motion.unwrap_or(false);
        if let Some(a) = &data.accent_choice { self.accent_choice = AccentChoice::from_str(a); }
        self.glass_mode = data.glass_mode.unwrap_or(false);
        self.playhead_glow = data.playhead_glow.unwrap_or(false);
        if let Some(name) = &data.audio_device {
            if let Some(i) = self.audio_devices.iter().position(|n| n == name) { self.selected_audio = Some(i); }
        }
        if let Some(name) = &data.midi_device {
            if let Some(i) = self.midi_inputs.iter().position(|d| &d.name == name) { self.selected_midi = Some(i); }
        }
    }

    fn apply_style(&self, ctx: &egui::Context) {
        // One-time font registration; safe to call repeatedly.
        ui_theme::ensure_inter(ctx);
        // Base visuals from selected theme.
        ui_theme::apply(ctx, self.ui_theme);
        // Accent overrides
        let mut style = (*ctx.style()).clone();
        let accent = self.accent_choice.color();
        style.visuals.selection.bg_fill = accent;
        style.visuals.selection.stroke = egui::Stroke::new(1.0, style.visuals.widgets.noninteractive.fg_stroke.color);
        // Make active/hovered widget fills respond to accent subtly so sliders/buttons feel alive
        style.visuals.widgets.hovered.bg_fill = accent.linear_multiply(0.18).additive();
        style.visuals.widgets.active.bg_fill = accent.linear_multiply(0.24).additive();
        // Glass panels in Dark theme
        if self.glass_mode && matches!(self.ui_theme, ui_theme::ThemeMode::DarkNeon) {
            let mut v = style.visuals.clone();
            let a = 240; // ~94% opacity
            v.panel_fill = egui::Color32::from_rgba_unmultiplied(28, 31, 38, a);
            v.widgets.noninteractive.bg_fill = egui::Color32::from_rgba_unmultiplied(28, 31, 38, a);
            v.widgets.inactive.bg_fill = egui::Color32::from_rgba_unmultiplied(35, 40, 52, a);
            style.visuals = v;
        }
        ctx.set_style(style);
        // High-contrast overlay tuned per theme instead of forcing Dark.
        if self.high_contrast {
            let mut style = (*ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            // Strengthen strokes for readability
            let accent = match self.ui_theme { ui_theme::ThemeMode::DarkNeon => egui::Color32::WHITE, ui_theme::ThemeMode::LightNeumorphic => egui::Color32::from_gray(40) };
            style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.5, accent);
            style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, accent);
            style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, accent);
            style.visuals.override_text_color = Some(accent);
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

// legacy helper removed; toggles now use `ui.toggle_value` directly

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
    // Practice (Tutor) settings — optional for backward compatibility
    practice_match_window_pct: Option<f32>,
    practice_on_time_pct: Option<f32>,
    practice_match_cap_ms: Option<f32>,
    practice_on_time_cap_ms: Option<f32>,
    practice_countdown_each_loop: Option<bool>,
    practice_default_loop_count: Option<u8>,
    practice_tempo_scale_default: Option<f32>,
    // Appearance
    ui_theme: Option<String>,
    reduced_motion: Option<bool>,
    accent_choice: Option<String>,
    glass_mode: Option<bool>,
    playhead_glow: Option<bool>,
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
#[derive(Clone, Debug)]
struct Ripple { beat: f64, piece: DrumPiece, start: Instant }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopHandle { Start, End }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AccentChoice { Blue, Orange, Green, Pink, Purple, Custom(egui::Color32) }

impl AccentChoice {
    fn color(self) -> egui::Color32 {
        match self {
            AccentChoice::Blue => egui::Color32::from_rgb(0, 180, 255),
            AccentChoice::Orange => egui::Color32::from_rgb(255, 140, 66),
            AccentChoice::Green => egui::Color32::from_rgb(34, 197, 94),
            AccentChoice::Pink => egui::Color32::from_rgb(255, 46, 185),
            AccentChoice::Purple => egui::Color32::from_rgb(140, 105, 255),
            AccentChoice::Custom(c) => c,
        }
    }
    fn as_str(self) -> &'static str {
        match self { Self::Blue => "blue", Self::Orange => "orange", Self::Green => "green", Self::Pink => "pink", Self::Purple => "purple", Self::Custom(_) => "custom" }
    }
    fn from_str(s: &str) -> Self {
        if let Some(rest) = s.strip_prefix("custom-") {
            if rest.len() == 6 {
                if let Ok(rgb) = u32::from_str_radix(rest, 16) {
                    let r = ((rgb >> 16) & 0xFF) as u8;
                    let g = ((rgb >> 8) & 0xFF) as u8;
                    let b = (rgb & 0xFF) as u8;
                    return Self::Custom(egui::Color32::from_rgb(r, g, b));
                }
            }
        }
        match s { "orange" => Self::Orange, "green" => Self::Green, "pink" => Self::Pink, "purple" => Self::Purple, "blue" => Self::Blue, _ => Self::Blue }
    }
}
