# PR: Studio + Practice overhaul — docks, loop handles, numbers, Light/HC polish

## Summary
This PR refreshes the Desktop app with a modern authoring + practice experience, renames the Tutor surface to Practice, adds Free Play/Test modes, A/B loop, and an end‑of‑run Review.

- Studio (formerly Extractor)
  - Native file picker; New Chart; Load Sample
  - Waveform backdrop with zoom/pan; loop A/B
  - Editor Tools: piece/velocity, snap-to-grid; click to add, select/drag, right-click delete
  - Bottom transport dock: Play/Pause, integer BPM label, Loop toggle with Start/End, Record MIDI, Metronome gain as percent
  - Ruler A/B handles (draggable) with numeric A/B in dock
  - Lane Mute/Solo pills (horizontal, exclusive per lane); global Solo dims others
  - Live MIDI recording into the chart using mapping, latency-compensated and snapped

- Settings
  - Audio device dropdown; test tone playback; high-contrast overlay tuned for current theme
  - MIDI device picker
  - Mapping wizard with visual kit and learn-by-hit; revert mappings
  - Latency calibration wizard (beep + hit average)
  - Debounced autosave for Practice prefs (metronome, pre‑roll, use-lesson-tempo)
  - Practice section: Match/On‑Time windows (% of beat + ms caps), countdown each loop, Test loops before review (default 2), default tempo scaling

- Practice (formerly Tutor)
  - Modes: Free Play (infinite loop, no Review) and Test (runs N loops before Review; N from Settings)
  - A/B loop region with on-canvas highlight; quick Set A/B buttons; (Studio ruler handles implemented; Practice to follow)
  - Note highway (Crash/Ride/HH/Snare/Toms/Kick), beat/bar markers and measure numbers
  - Colors: green (on‑time), blue (early), orange (late), red (missed), gray (not‑yet‑played)
  - Legend; Freeze playhead (scroll notes) option
  - Bottom dock: Play/Pause; Use lesson tempo; integer BPM label; metronome gain as percent; Pre‑roll slider; Freeze playhead
  - Pre‑roll countdown (seconds): large centered numeral with soft circular background + ring
  - Live MIDI hits mapped + latency‑compensated; configurable timing windows as % of beat with ms caps
  - Review summary: centered card, stable instrument order, encouraging text based on accuracy

- Docs
  - README: docks, numbers policy (integers/percents), countdown/Review, iconization
  - UX_OVERVIEW: Studio/Practice capabilities, quick actions, modes, A/B loop
  - ARCHITECTURE: UI theming & assets section, Settings > Practice updated default
  - LOW_LEVEL_DESIGN: countdown seconds, loop behavior, centered review, transport numerics
  - UX_SPECS: per‑screen docks, M/S placement, numeric rules
  - ASSETS: exact icon list + tinting + sizing

## Code Highlights
- apps/desktop/src/main.rs: Studio + Practice UI, A/B loop, Review modal, settings wiring, mapping wizard, metronome/tone
- crates/notation/src/lib.rs: editor API + draw_with_timeline with loop/playhead
- crates/tutor/src/scoring.rs: score_with_spb for ms-based evaluation
- crates/domain/src/tempo.rs: time_at_beat + supportive helpers

## Testing
- cargo build --workspace: passes
- Manual smoke tests recommended:
  - Studio: pick audio → see waveform; add/move/delete notes; loop/zoom; metronome click; record MIDI
  - Settings: refresh audio/MIDI, test tone, toggle high-contrast; open mapping wizard; run latency calibration; confirm autosave
  - Practice: import/open/close chart; Free Play vs Test; A/B loop; toggle Use lesson tempo; Play/Pause; pre‑roll overlay; metronome; colors; freeze playhead; timing windows; run through Test to Review

## Follow-ups
- Practice ruler A/B handles (mirror Studio) — implemented
- Studio drawers: Left Tools (Piece/Velocity/Grid/Snap), Right Inspector (Quantize/Undo/Redo) — implemented; removed duplicate Editor Tools row
- Iconization: Play/Pause + Metronome in Practice dock; consistent tint across Dark/Light; complete remaining toolbar/tab icons as assets land
- Persist freeze playhead toggle; Review details (timing histogram)

---

# PR: Accent system, Settings Appearance overhaul, micro‑interactions

## Summary
This PR introduces a user‑selectable accent system, cleans up the Settings → Appearance layout, adds optional glass surfaces and playhead glow, and finishes the first pass of icon tint + hover animations across Studio and Practice.

- Accent & Themes
  - Settings → Appearance now exposes an accent selector with presets (Blue, Orange, Green, Neon Pink, Neon Purple) plus a Custom color picker.
  - Accent is the single source of truth for icon tint, selection highlight, and hovered/active widget fills (sliders/buttons).
  - Optional "Glass surfaces (Dark only)" mode makes panels slightly translucent for a modern DAW feel.

- Studio & Practice UI
  - Practice ruler now uses the shared `draw_highway` with accent‑colored playhead; optional Playhead glow toggle under Appearance → Display & Motion.
  - Studio uses left "Tools" and right "Inspector" drawers (already landed), freeing the canvas; toolbar buttons (New, Sample, Open, Transcribe) use tinted Lucide icons with hover/press animation.
  - Iconization is consistent across tabs and transports: Studio/Practice tabs, Studio transport (Play/Pause, Record, Metronome), Practice transport (Play/Pause, Metronome) all use the accent via the shared tint helper.

- Settings Layout
  - Appearance card now separates concerns:
    - Theme & Accent: Dark (Studio), Light (Practice), accent presets + Custom picker.
    - Surfaces: Glass surfaces toggle (Dark only).
  - New cards:
    - Display & Motion: Playhead glow, Reduced motion, High contrast.
    - Sound & Behavior: App sounds, Auto preview, Play screen note streaks, New Keys Experience.
  - Result: fewer "one‑off" flags jammed together; controls are grouped by intent and feel like modern toggles.

- Micro‑Interactions
  - Tabs, toolbar buttons, and transport icons now animate using `hover_scale` and `hover_tint` helpers (slight scale‑up and brightness on hover/press) tied to the accent.
  - Playhead line in Practice is drawn in the accent color; optional soft glow uses two wider strokes with reduced alpha, gated by Reduced Motion.

- Docs
  - Added `docs/DESIGN_PHILOSOPHY.md` to capture cross‑screen UI rules: fonts, spacing, color/accents, motion ceilings, component patterns.
  - Updated `docs/UX_SPECS.md` to reference the philosophy, document accent‑driven icon/slider styling, and describe the Playhead glow option.
  - Updated `docs/ASSETS.md` to clarify that icons are tinted from the accent (no separate dark/light asset sets) and to note loader alias behavior.

## Code Highlights
- apps/desktop/src/main.rs
  - `SettingsPane`: new `accent_choice`, `glass_mode`, `playhead_glow`; reshaped `ui_appearance_card`, added `ui_display_motion_card` and `ui_options_card`.
  - Tabs and transports: use `taal_ui::icons::default_tint`, `hover_tint`, and `hover_scale` for icon rendering.
  - `draw_highway`: now takes `playhead_glow` flag and uses the accent color for the playhead + optional glow.
- crates/ui/src/icons.rs
  - Icon tint helpers: `default_tint`, `hover_tint`, `hover_scale` built on `visuals.selection.bg_fill`.

## Testing
- `cargo build --workspace`: passes.
- Manual smoke checks suggested:
  - Switch accents and themes; verify icons, sliders, and selection highlight follow the accent and remain readable in both Dark/Light and High Contrast.
  - Toggle Glass surfaces, Reduced motion, and Playhead glow; confirm glass only affects Dark, motion gates decorative animations, and playhead glow is subtle.
  - Studio/Practice: hover toolbar and transport icons to confirm micro‑interactions feel responsive but not distracting.
