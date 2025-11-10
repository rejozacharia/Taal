use egui::{Color32, FontData, FontDefinitions, FontFamily, Rounding, Stroke, Visuals};
use once_cell::sync::OnceCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    DarkNeon,
    LightNeumorphic,
}

#[derive(Clone, Debug)]
pub struct ThemeTokens {
    pub accent_warm: Color32,
    pub accent_cool: Color32,
    pub neutral_bg: Color32,
    pub neutral_surface: Color32,
    pub neutral_panel: Color32,
    pub text_primary: Color32,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub tokens: ThemeTokens,
    pub visuals: Visuals,
}

pub fn theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::DarkNeon => dark_neon(),
        ThemeMode::LightNeumorphic => light_neumorphic(),
    }
}

pub fn dark_neon() -> Theme {
    let tokens = ThemeTokens {
        accent_warm: Color32::from_rgb(0xFF, 0x8C, 0x42),
        accent_cool: Color32::from_rgb(0x00, 0xB4, 0xFF),
        neutral_bg: Color32::from_rgb(0x0F, 0x11, 0x15),
        neutral_surface: Color32::from_rgb(0x1C, 0x1F, 0x26),
        neutral_panel: Color32::from_rgb(0x23, 0x28, 0x34),
        text_primary: Color32::from_rgb(0xE6, 0xE6, 0xE6),
    };

    let mut visuals = Visuals::dark();
    visuals.window_rounding = Rounding::same(8.0);
    visuals.panel_fill = tokens.neutral_surface;
    visuals.widgets.noninteractive.bg_fill = tokens.neutral_surface;
    visuals.widgets.inactive.bg_fill = tokens.neutral_panel;
    visuals.widgets.active.bg_fill = tokens.neutral_panel.linear_multiply(1.1);
    visuals.widgets.hovered.bg_fill = tokens.neutral_panel.linear_multiply(1.15);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, tokens.text_primary);

    Theme { tokens, visuals }
}

pub fn light_neumorphic() -> Theme {
    let tokens = ThemeTokens {
        accent_warm: Color32::from_rgb(0xFF, 0x8C, 0x42),
        accent_cool: Color32::from_rgb(0x2B, 0x86, 0xFF),
        neutral_bg: Color32::from_rgb(0xF4, 0xF6, 0xF8),
        neutral_surface: Color32::from_rgb(0xFA, 0xFB, 0xFC), // off-white for contrast
        neutral_panel: Color32::from_rgb(0xE7, 0xEB, 0xF0),   // subtle gray for tracks
        text_primary: Color32::from_rgb(0x2B, 0x2D, 0x33),
    };

    let mut visuals = Visuals::light();
    visuals.window_rounding = Rounding::same(10.0);
    visuals.panel_fill = tokens.neutral_bg;
    // Increase contrast for controls on light background
    visuals.widgets.noninteractive.bg_fill = tokens.neutral_surface;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, tokens.text_primary);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_gray(190));
    visuals.widgets.inactive.bg_fill = tokens.neutral_panel; // tracks, buttons at rest
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, tokens.text_primary);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_gray(200));
    visuals.widgets.hovered.bg_fill = tokens.neutral_panel;
    visuals.widgets.active.bg_fill = tokens.neutral_surface;
    visuals.selection.bg_fill = tokens.accent_cool;
    visuals.selection.stroke = Stroke::new(1.0, tokens.text_primary);

    Theme { tokens, visuals }
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let theme = theme(mode);
    ctx.set_visuals(theme.visuals);
}

// Fonts: Inter Regular/Bold if available; fallback to egui defaults.
static FONTS_DONE: OnceCell<()> = OnceCell::new();

pub fn ensure_inter(ctx: &egui::Context) {
    if FONTS_DONE.get().is_some() { return; }

    // Try to load from typical asset locations. If not found, keep defaults.
    let candidates = [
        // run-from-repo paths
        "assets/fonts/Inter-Regular.ttf",
        "../assets/fonts/Inter-Regular.ttf",
        "../../assets/fonts/Inter-Regular.ttf",
    ];
    let bold_candidates = [
        "assets/fonts/Inter-Bold.ttf",
        "../assets/fonts/Inter-Bold.ttf",
        "../../assets/fonts/Inter-Bold.ttf",
    ];

    let read = |paths: &[&str]| -> Option<Vec<u8>> {
        for p in paths {
            if let Ok(bytes) = std::fs::read(p) { return Some(bytes); }
        }
        None
    };

    if let (Some(reg), Some(bold)) = (read(&candidates), read(&bold_candidates)) {
        let mut defs = FontDefinitions::default();
        defs.font_data.insert("Inter-Regular".into(), FontData::from_owned(reg));
        defs.font_data.insert("Inter-Bold".into(), FontData::from_owned(bold));
        defs.families.entry(FontFamily::Proportional).or_default().insert(0, "Inter-Regular".into());
        defs.families.entry(FontFamily::Proportional).or_default().insert(0, "Inter-Bold".into());
        ctx.set_fonts(defs);
    }

    let _ = FONTS_DONE.set(());
}
