# Taal UI Design Plan

This plan defines visual themes, tokens, component patterns, motion, and concrete egui mappings for Taal’s desktop UI. It supports two primary modes now (Dark Neon and Light Neumorphic) and a Gradient Performance theme for results views.

## Design Decisions (Locked)

1. Font → Inter (default in-app font). Rubik may be used in marketing only.
2. Icon Set → Lucide (SVG). Integrate as inline SVGs/assets.
3. Gradient/Performance Theme → Phase 2 (after Studio/Practice ship).

## Goals

1. Professional studio look with clear rhythm and hierarchy.
2. Fast, tactile feedback for hits, metronome, and timeline.
3. Accessible, high-contrast defaults with optional reduced motion.
4. One codepath with theme tokens that map cleanly to egui.

## Theme System

Implement three themes that share common tokens. Themes select values for neutrals, accents, elevation, and shadows.

- Tokens
  - `accent_warm`: energetic orange
  - `accent_cool`: stable blue
  - `neutral_bg`, `neutral_surface`, `neutral_panel`
  - `stroke_soft`, `stroke_strong`
  - `text_primary`, `text_secondary`, `text_muted`
  - `success`, `warn`, `error`
  - `elev_0..3`: background → raised surface
  - `radius_sm` (=6), `radius_md` (=8), `radius_lg` (=10)
  - `space_unit` (=8 px)

- Theme Palettes
  - Dark Neon (Studio)
    - `neutral_bg`: #0F1115
    - `neutral_surface`: #1C1F26
    - `neutral_panel`: #232834
    - `accent_warm`: #FF8C42
    - `accent_cool`: #00B4FF
    - `text_primary`: #E6E6E6, `text_secondary`: #C9CED6, `text_muted`: #9AA3AE
    - Shadows: outer glow on active; soft drop for elevation.
  - Light Neumorphic (Practice)
    - `neutral_bg`: #F4F6F8
    - `neutral_surface`: #FFFFFF
    - `neutral_panel`: #EDEFF2
    - `accent_warm`: #FF8C42
    - `accent_cool`: #2B86FF
    - `text_primary`: #2B2D33, `text_secondary`: #50535B, `text_muted`: #7E858E
    - Shadows: subtle outer + inner for tactile depth.
  - Gradient Hybrid (Performance) — Phase 2
    - Background gradient: #0B1220 → #1E2433
    - Accents: `accent_warm` #FF8C42, `accent_cool` #06B6D4
    - Text: #E5E7EB

## egui Mappings

Rust pseudo‑APIs showing how to map tokens to egui. Implement as a `taal_ui::theme` module.

```rust
use egui::{Color32, Rounding, Stroke, Vec2, Visuals};

pub struct ThemeTokens {
    pub accent_warm: Color32,
    pub accent_cool: Color32,
    pub neutral_bg: Color32,
    pub neutral_surface: Color32,
    pub neutral_panel: Color32,
    pub stroke_soft: Color32,
    pub stroke_strong: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub success: Color32,
    pub warn: Color32,
    pub error: Color32,
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub space_unit: f32,
}

pub struct Theme {
    pub tokens: ThemeTokens,
    pub visuals: Visuals,
}

pub fn dark_neon() -> Theme {
    let t = ThemeTokens {
        accent_warm: Color32::from_rgb(0xFF, 0x8C, 0x42),
        accent_cool: Color32::from_rgb(0x00, 0xB4, 0xFF),
        neutral_bg: Color32::from_rgb(0x0F, 0x11, 0x15),
        neutral_surface: Color32::from_rgb(0x1C, 0x1F, 0x26),
        neutral_panel: Color32::from_rgb(0x23, 0x28, 0x34),
        stroke_soft: Color32::from_rgba_premultiplied(0x7A,0x85,0x96,120),
        stroke_strong: Color32::from_rgb(0xC9,0xCE,0xD6),
        text_primary: Color32::from_rgb(0xE6,0xE6,0xE6),
        text_secondary: Color32::from_rgb(0xC9,0xCE,0xD6),
        text_muted: Color32::from_rgb(0x9A,0xA3,0xAE),
        success: Color32::from_rgb(0x34,0xD3,0x89),
        warn: Color32::from_rgb(0xF5,0xB3,0x38),
        error: Color32::from_rgb(0xEF,0x44,0x44),
        radius_sm: 6.0, radius_md: 8.0, radius_lg: 10.0,
        space_unit: 8.0,
    };

    let mut visuals = Visuals::dark();
    visuals.window_rounding = Rounding::same(t.radius_md);
    visuals.panel_fill = t.neutral_surface;
    visuals.widgets.noninteractive.bg_fill = t.neutral_surface;
    visuals.widgets.inactive.bg_fill = t.neutral_panel;
    visuals.widgets.inactive.weak_bg_fill = t.neutral_panel;
    visuals.widgets.active.bg_fill = t.neutral_panel.linear_multiply(1.1);
    visuals.widgets.hovered.bg_fill = t.neutral_panel.linear_multiply(1.15);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text_primary);

    Theme { tokens: t, visuals }
}

pub fn light_neumorphic() -> Theme {
    let t = ThemeTokens {
        accent_warm: Color32::from_rgb(0xFF, 0x8C, 0x42),
        accent_cool: Color32::from_rgb(0x2B, 0x86, 0xFF),
        neutral_bg: Color32::from_rgb(0xF4, 0xF6, 0xF8),
        neutral_surface: Color32::WHITE,
        neutral_panel: Color32::from_rgb(0xED, 0xEF, 0xF2),
        stroke_soft: Color32::from_rgba_premultiplied(0,0,0,25),
        stroke_strong: Color32::from_rgba_premultiplied(0,0,0,90),
        text_primary: Color32::from_rgb(0x2B,0x2D,0x33),
        text_secondary: Color32::from_rgb(0x50,0x53,0x5B),
        text_muted: Color32::from_rgb(0x7E,0x85,0x8E),
        success: Color32::from_rgb(0x16,0xA3,0x69),
        warn: Color32::from_rgb(0xB4,0x73,0x00),
        error: Color32::from_rgb(0xB9,0x1C,0x1C),
        radius_sm: 8.0, radius_md: 10.0, radius_lg: 12.0,
        space_unit: 8.0,
    };

    let mut visuals = Visuals::light();
    visuals.window_rounding = Rounding::same(t.radius_md);
    visuals.panel_fill = t.neutral_bg;
    visuals.widgets.noninteractive.bg_fill = t.neutral_surface;
    visuals.widgets.inactive.bg_fill = t.neutral_surface;
    visuals.widgets.active.bg_fill = t.neutral_surface;
    visuals.widgets.hovered.weak_bg_fill = t.neutral_panel; // subtle lift

    Theme { tokens: t, visuals }
}

// Apply at runtime
pub fn apply(ctx: &egui::Context, theme: &Theme) {
    ctx.set_visuals(theme.visuals.clone());
}
```

### Font: Inter

Register Inter Regular/Bold as the default proportional family. Bundle TTFs in the app or load at runtime.

```rust
use egui::{FontData, FontDefinitions, FontFamily};

pub fn register_inter(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Inter-Regular".into(),
        FontData::from_static(include_bytes!("../../assets/fonts/Inter-Regular.ttf")),
    );
    fonts.font_data.insert(
        "Inter-Bold".into(),
        FontData::from_static(include_bytes!("../../assets/fonts/Inter-Bold.ttf")),
    );
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "Inter-Regular".into());
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "Inter-Bold".into());
    ctx.set_fonts(fonts);
}
```

### Neumorphic Depth in egui

- Outer shadow: draw behind panel using `Frame::new().shadow(egui::epaint::Shadow { offset: (0, 2), blur: 12, spread: 0, color: rgba(0,0,0,48) })`.
- Inner shadow: simulate by overlaying a `RectShape` with inward gradient or by drawing two subtle highlight lines on top and left, and two darker lines on bottom and right.
- Keep elevations limited (0–2) to avoid visual noise.

## Components

- Top App Bar
  - Left: product name. Center: tabs (Studio / Practice / Marketplace / Settings) with icon + label, active underline in theme accent (`accent_cool` for Dark, `accent_warm` for Light).
  - Right: quick actions (New, Open, Load, Transcribe, Save/Export) as segmented icon buttons with tooltips.

- Editor Grid / Highway
  - Banded rows, bar lines thicker than beat lines.
  - Current beat glow: a vertical bar with a faint bloom using `linear_multiply(1.2)` and alpha ramp ahead of the playhead (~100 ms).

- Controls
  - Play/Pause prominent circular button, ring glow when active.
  - Toggle buttons: Loop, Metronome, Simulate Hit.

- Panels
  - Rounded corners radius 6–10 depending on theme token; consistent 8 px spacing system.

## Motion & Micro‑Interactions

- Pad Hit Ripple: expanding circle 120–180 ms, ease‑out. Use accent_warm for hits; reduce alpha quickly.
- Metronome Pulse: ring thickness +5–10% on downbeat; sync to BPM.
- Timeline Glow: pre‑light upcoming beats for 100 ms.
- Score Reveal: slide‑up card; counters animate 0→N; use `success/warn/error`.
- Mode Transitions: cross‑fade background and slide toolbars; target 150–200 ms.
- Performance: allow “Reduce motion” preference to disable animations and extra draws.

### egui sketch for hit ripple

```rust
// in update()
for hit in hits.iter_mut() { // hits: Vec<HitAnim>
    let t = (now - hit.start).as_secs_f32();
    let p = (t / 0.16).min(1.0);
    let r = egui::lerp(6.0..=24.0, p);
    let alpha = (1.0 - p) * 0.6;
    let color = theme.tokens.accent_warm.linear_multiply(alpha);
    painter.circle(hit.pos, r, Color32::TRANSPARENT, Stroke::new(2.0, color));
}
```

## Icons

- Use Lucide SVGs via `egui_extras::RetainedImage` or `egui::Image` with rasterized assets.
- Provide tooltips for all icon buttons.
- Group file actions in a segmented control using `ui.horizontal(|ui| ui.group(|ui| { ... }))`.

Example button:

```rust
ui.add(egui::Button::image_and_text(icon_svg("file-plus"), "New Chart"))
    .on_hover_text("Create a new chart");
```

Loading Lucide SVGs (compile-time embedded):

```rust
use egui_extras::RetainedImage;

fn icon_svg(name: &str) -> RetainedImage {
    // Example expects files under assets/icons/lucide/{name}.svg
    let bytes: &'static [u8] = match name {
        "file-plus" => include_bytes!("../../assets/icons/lucide/file-plus.svg"),
        "music" => include_bytes!("../../assets/icons/lucide/music.svg"),
        "folder-open" => include_bytes!("../../assets/icons/lucide/folder-open.svg"),
        "waveform" => include_bytes!("../../assets/icons/lucide/waveform.svg"),
        "save" => include_bytes!("../../assets/icons/lucide/save.svg"),
        _ => include_bytes!("../../assets/icons/lucide/help-circle.svg"),
    };
    RetainedImage::from_svg_bytes(name, bytes).expect("valid SVG")
}
```

## Accessibility

- Maintain contrast ≥ 4.5:1 for text on surfaces; prefer ≥ 7:1 for body text.
- Keyboard focus ring with theme accent and 2 px stroke.
- Settings → Reduced motion toggle; disable non‑essential animations.

## Implementation Roadmap

1. Theme crate/module (`taal_ui::theme`) with tokens + `Visuals` builders and runtime switching.
2. Font registration for Inter (Regular/Bold) and default text styles.
3. Global spacing helpers and `Frame` helpers for panels (elevations 0–2).
4. Top bar + segmented Lucide icon toolbar with tooltips.
5. Grid/highway visuals and playhead glow.
6. Hit ripple + metronome pulse prototypes behind a feature flag.
7. Settings toggles for theme switching and reduced motion.
8. Phase 2: Gradient Performance theme and animated counters.

## Current Implementation Status

- Theme runtime toggle wired in desktop app (`Settings → Appearance`).
- Inter registration at startup when fonts are present under `assets/fonts/`.
- Icon toolbar (Lucide) in Studio with New/Open/Sample/Transcribe actions; falls back to text if assets are missing.
- Playhead glow and metronome pulse visuals in Studio and Practice.
- Hit ripple prototype in Practice when using "Simulate Hit"; prepared for MIDI events.

See `docs/UX_SPECS.md` for per-screen component catalogs (Studio/Practice/Settings) and behavior specs (layout, interactions, motion).

## Open Questions

- Shared font choice (Inter or Rubik) and licensing for bundling.
- Whether to embed SVGs or rasterize at build time; evaluate perf/memory.
- Theme persistence format in `taal_config` crate.

```
This document will evolve as we prototype and validate UI interactions.
```
