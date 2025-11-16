# Taal UI Design Philosophy

A concise foundation to guide current and future screens (Studio, Practice, Settings, and upcoming modules) so the app feels coherent and polished across platforms.

## Principles
- Canvas First: prioritize the musical canvas (grid/highway/staff). Controls support the canvas, never overwhelm it.
- Modern Minimal: avoid skeuomorphism; use clean surfaces, subtle shadows, and clear hierarchy.
- Color Duality: warm (energy) + cool (stability). Default accents pair Orange (#FF8C42) and Neon Blue (#00B4FF). Users can select the primary accent.
- Delight with Restraint: micro-interactions under 200 ms, gentle easing. Motion enhances rhythm, never distracts.
- Consistency: identical iconography, spacing, rounding, and numerics across screens.

## Foundations
- Font: Inter for UI; Regular/Bold weights. Keep labels short; use tooltips for depth.
- Icons: Lucide SVGs (24 px viewport), tinted at runtime to the accent color. No separate dark/light assets.
- Spacing: 8 px grid; panel rounding 6–10 px.
- Numerics: BPM are integers; fractional controls display as integer percents (e.g., 64%).
- Accessibility: High-Contrast overlays increase strokes and text without changing theme; Reduced Motion disables decorative animations.

## Themes
- Dark Neon (Studio): deep grays, neon accent. Optionally enable “Glass surfaces” for translucent panels.
- Light Neumorphic (Practice): soft surfaces with subtle banding and stronger label contrast for readability.
- Gradient Neon (Phase 2): dark blue → purple gradient, limited to performance visualizations and end-of-session stats.

## Accent System
- User-selectable accent in Settings → Appearance (Blue, Orange, Green, Neon Pink).
- Accent affects icon tint, selection highlights, sliders, and active affordances.
- Implementation note: the accent is applied via `visuals.selection.bg_fill`; icons use the same value.

## Motion & Interactions
- Micro-Interactions
  - Hover: brightness +8–12% or scale 1.03; press 1.05; tooltips on rest.
  - Tab transitions: soft slide/fade under 180 ms.
- Practice Feedback
  - Hit ripple: 120–180 ms ring; disabled in Reduced Motion.
  - Progressive judgments: on-time (fade-in green), early (fade-in blue), late (fade-in orange), missed (fade-in red). Optional slight directional easing.
- Countdown
  - Centered numerals; scale-in with ease_out_cubic; time-based (seconds) rather than beat-based.
- Optional Motifs (toggles under Appearance)
  - Playhead glow/sweep synced to BPM (default OFF).
  - Glass surfaces in Dark theme.

## Layout & Usability
- Side Drawers: left Tools (create/edit), right Inspector (actions/properties). Collapsible; future work: resizable, dockable.
- Transport Dock at bottom for both Studio/Practice.
- Dynamic Grid Scaling (planned): pinch/scroll zoom on the timeline with Ctrl/Cmd modifiers.

## Components & States
- Toolbar Buttons: icon + short label; hover glow; consistent padding and spacing.
- Lane Controls: Mute and Solo pills (🎧/🔇 icons planned); horizontal alignment; mutual exclusivity per lane; global solo dims others.
- Modals & Dropdowns: slight drop shadows; generous padding; centered Review modal.

## Roadmap (Macro)
1. Interactions pass: hover/press animations on toolbars and tabs; icon hover tint.
2. Accent selector & glass surfaces (implemented in Settings); adopt across components.
3. Practice highlighting: progressive color-in for judgments, optional playhead glow toggle.
4. Dockable panels (resizable drawers), grid scaling improvements.
5. Gradient Neon performance theme and end-of-session visualizations.
6. Analytics: streak popups, mini heatmaps, timing trendlines.

## Code Mapping (egui)
- Icons: `taal_ui::icons::icon_tex(ctx, name)` + `ImageButton::tint(accent_color)`.
- Accent: `ui.style().visuals.selection.bg_fill` used as canonical accent.
- Animations: `ui.ctx().animate_bool(id, state)` and time deltas for fades/scales.
- Visuals: theme presets in `crates/ui/src/theme.rs`; runtime tweaks in `SettingsPane::apply_style`.

This document is the baseline for all surfaces. Extend it when adding modules; do not fork conventions per screen.
