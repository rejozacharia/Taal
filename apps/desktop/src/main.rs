use std::sync::Arc;

use eframe::{egui, egui::Ui};
use taal_domain::LessonDescriptor;
use taal_notation::NotationEditor;
use taal_services::MarketplaceClient;
use taal_transcriber::{TranscriptionJob, TranscriptionPipeline};
use taal_tutor::{PracticeMode, ScoringEngine, SessionAnalytics, SessionState};
use tokio::runtime::Runtime;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let rt = Arc::new(Runtime::new()?);
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Taal Desktop",
        options,
        Box::new(|_cc| Box::new(DesktopApp::new(rt.clone()))),
    )?;
    Ok(())
}

struct DesktopApp {
    active_tab: ActiveTab,
    extractor: ExtractorPane,
    tutor: TutorPane,
    marketplace: MarketplacePane,
}

impl DesktopApp {
    fn new(rt: Arc<Runtime>) -> Self {
        Self {
            active_tab: ActiveTab::Extractor,
            extractor: ExtractorPane::new(),
            tutor: TutorPane::new(),
            marketplace: MarketplacePane::new(rt),
        }
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Extractor, "Extractor");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Tutor, "Tutor");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Marketplace, "Marketplace");
            });
        });

        match self.active_tab {
            ActiveTab::Extractor => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.extractor.ui(ui, &mut self.tutor);
                });
            }
            ActiveTab::Tutor => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.tutor.ui(ui);
                });
            }
            ActiveTab::Marketplace => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.marketplace.ui(ui);
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
}

struct ExtractorPane {
    pipeline: TranscriptionPipeline,
    input_path: String,
    status_message: Option<String>,
    editor: Option<NotationEditor>,
}

impl ExtractorPane {
    fn new() -> Self {
        Self {
            pipeline: TranscriptionPipeline::new(),
            input_path: String::new(),
            status_message: None,
            editor: None,
        }
    }

    fn ui(&mut self, ui: &mut Ui, tutor: &mut TutorPane) {
        ui.heading("Drum Sheet Extractor");
        ui.horizontal(|ui| {
            ui.label("Audio file path:");
            ui.text_edit_singleline(&mut self.input_path);
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
        if let Some(message) = &self.status_message {
            ui.label(message);
        }
        if let Some(editor) = &mut self.editor {
            editor.draw(ui);
        } else {
            ui.label("No transcription yet. Provide an audio path and press Transcribe.");
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

struct TutorPane {
    session: Option<SessionState>,
    hits: Vec<taal_domain::DrumEvent>,
    scoring: ScoringEngine,
    analytics: Option<SessionAnalytics>,
}

impl TutorPane {
    fn new() -> Self {
        Self {
            session: None,
            hits: Vec::new(),
            scoring: ScoringEngine,
            analytics: None,
        }
    }

    fn load_lesson(&mut self, lesson: LessonDescriptor) {
        info!("loading lesson into tutor", id = %lesson.id);
        self.session = Some(SessionState::new(lesson, PracticeMode::Learn));
        self.hits.clear();
        self.analytics = None;
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.heading("Tutor Mode");
        if let Some(session) = &mut self.session {
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
                let report = self.scoring.score(&session.lesson, &self.hits);
                let mut stats = session.lesson.stats.clone();
                let analytics = SessionAnalytics::new(report.clone());
                analytics.update_statistics(&mut stats);
                self.analytics = Some(analytics);
                ui.label(format!("Accuracy: {:.0}%", report.accuracy * 100.0));
            }
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
