use std::collections::HashMap;
use std::sync::Mutex;

use egui::{Context, TextureId};
#[allow(deprecated)]
use egui_extras::RetainedImage;
use once_cell::sync::Lazy;

static CACHE: Lazy<Mutex<HashMap<String, TextureId>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Load a Lucide SVG by name (e.g., "file-plus").
/// Searches common asset paths and caches as a texture handle.
#[allow(deprecated)]
pub fn icon_tex(ctx: &Context, name: &str) -> Option<TextureId> {
    if let Some(handle) = CACHE.lock().ok()?.get(name).cloned() {
        return Some(handle);
    }

    let filename = format!("{name}.svg");
    let paths = [
        format!("assets/icons/lucide/{filename}"),
        format!("../assets/icons/lucide/{filename}"),
        format!("../../assets/icons/lucide/{filename}"),
    ];
    for p in paths {
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(img) = RetainedImage::from_svg_bytes(name, &bytes) {
                let id = img.texture_id(ctx);
                let _ = CACHE.lock().ok()?.insert(name.to_string(), id);
                return Some(id);
            }
        }
    }
    None
}
