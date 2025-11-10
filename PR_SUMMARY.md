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
- Practice ruler A/B handles (mirror Studio)
- Studio drawers: Left Tools (Piece/Velocity/Grid/Snap), Right Inspector (Quantize/Undo/Redo) and remove toolbar duplication
- Full iconization of tabs/transport once assets land
- Persist freeze playhead toggle; Review details (timing histogram)
