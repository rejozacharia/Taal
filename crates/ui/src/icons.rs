use std::collections::HashMap;
use std::sync::Mutex;

use egui::{Context, TextureId, Ui};
#[allow(deprecated)]
use egui_extras::RetainedImage;
use once_cell::sync::Lazy;

// Keep RetainedImage alive; egui frees textures when the handle is dropped.
#[allow(deprecated)]
static CACHE: Lazy<Mutex<HashMap<String, RetainedImage>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Load a Lucide SVG by name (e.g., "file-plus").
/// Searches common asset paths and caches as a texture handle.
fn alias_list(name: &str) -> Vec<&str> {
    match name {
        // Mappings for Lucide renames or alternates we use in docs
        "waveform" => vec!["waveform", "audio-waveform"],
        "export" => vec!["export", "share"],
        "record" => vec!["record", "circle-dot"],
        "metronome" => vec!["metronome", "timer"],
        "sliders" => vec!["sliders", "sliders-horizontal"],
        "help-circle" => vec!["help-circle", "circle-question-mark"],
        "alert-triangle" => vec!["alert-triangle", "triangle-alert"],
        // Tabs and common actions
        "play" => vec!["play"],
        "pause" => vec!["pause"],
        "repeat" => vec!["repeat"],
        _ => vec![name],
    }
}

#[allow(deprecated)]
pub fn icon_tex(ctx: &Context, name: &str) -> Option<TextureId> {
    if let Some(img) = CACHE.lock().ok()?.get(name) {
        return Some(img.texture_id(ctx));
    }

    for candidate in alias_list(name) {
        let filename = format!("{candidate}.svg");
        let paths = [
            format!("assets/icons/lucide/{filename}"),
            format!("../assets/icons/lucide/{filename}"),
            format!("../../assets/icons/lucide/{filename}"),
        ];
        for p in &paths {
            if let Ok(bytes) = std::fs::read(p) {
                if let Ok(img) = RetainedImage::from_svg_bytes(candidate, &bytes) {
                    let id = img.texture_id(ctx);
                    // Cache by original request name so subsequent lookups are O(1)
                    let _ = CACHE.lock().ok()?.insert(name.to_string(), img);
                    return Some(id);
                }
            }
        }
    }
    #[cfg(debug_assertions)]
    if let Ok(mut cache) = CACHE.lock() {
        // only warn once per missing icon name
        if !cache.contains_key(&format!("missing::{name}")) {
            // store a tiny 1x1 transparent image as a placeholder
            let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>";
            if let Ok(img) = RetainedImage::from_svg_bytes("missing", svg) {
                let _ = cache.insert(format!("missing::{name}"), img);
            }
            eprintln!("[icons] missing SVG for '{}': tried aliases {:?}", name, alias_list(name));
        }
    }
    None
}

/// Reasonable default tint for icons that keeps contrast in Dark/Light.
pub fn default_tint(ui: &Ui) -> egui::Color32 {
    // Use the current selection background as the canonical accent color,
    // so icon tint follows the active theme/accent choice.
    ui.visuals().selection.bg_fill
}

/// Compute a hover/press animated tint based on the current accent color.
pub fn hover_tint(ui: &Ui, base: egui::Color32, hovered: bool, pressed: bool, id: &str) -> egui::Color32 {
    let key = egui::Id::new(format!("icon_hover:{}", id));
    let t = ui.ctx().animate_bool(key, hovered || pressed);
    // Brighten base color slightly on hover, a bit more on press
    let factor = if pressed { 1.20 } else { 1.08 };
    let f = 1.0 + (factor - 1.0) * t;
    base.linear_multiply(f as f32)
}

/// Return a size multiplier 1.0..=scale based on hover/press.
pub fn hover_scale(ui: &Ui, hovered: bool, pressed: bool, id: &str, scale: f32) -> f32 {
    let key = egui::Id::new(format!("icon_scale:{}", id));
    let t = ui.ctx().animate_bool(key, hovered || pressed);
    egui::lerp(1.0..=scale, t)
}
