# PR: Studio + Practice overhaul, A/B loop, Review, configurable timing

## Summary
This PR refreshes the Desktop app with a modern authoring + practice experience, renames the Tutor surface to Practice, adds Free Play/Test modes, A/B loop, and an end‑of‑run Review.

- Studio (formerly Extractor)
  - Native file picker; New Chart; Load Sample
  - Waveform backdrop with zoom/pan; loop A/B
  - Editor Tools: piece/velocity, snap-to-grid; click to add, select/drag, right-click delete
  - Transport with Play/Pause, BPM, playhead, metronome clicks
  - Live MIDI recording into the chart using mapping, latency-compensated and snapped

- Settings
  - Audio device dropdown; test tone playback; high-contrast theme
  - MIDI device picker
  - Mapping wizard with visual kit and learn-by-hit; revert mappings
  - Latency calibration wizard (beep + hit average)
  - Debounced autosave for Practice prefs (metronome, pre‑roll, use-lesson-tempo)
  - Practice section: Match/On‑Time windows (% of beat + ms caps), countdown each loop, default loop count, default tempo scaling

- Practice (formerly Tutor)
  - Modes: Free Play (infinite loop, no missed penalties) and Test (single pass, Review at end)
  - A/B loop region with on-canvas highlight; quick Set A/B here buttons
  - Note highway (Crash/Ride/HH/Snare/Toms/Kick), beat/bar markers and measure numbers
  - Colors: green (on‑time), blue (early), orange (late), red (missed), gray (not‑yet‑played)
  - Legend; Freeze playhead (scroll notes) option
  - Transport: Play/Pause, BPM; Use lesson tempo (TempoMap)
  - Pre‑roll countdown overlay (3‑2‑1‑Go)
  - Live MIDI hits mapped + latency‑compensated; configurable timing windows as % of beat with ms caps
  - End‑of‑run Review summary (Accuracy) using existing scoring engine

- Docs
  - README: renamed Practice, features, color legend, Docs Update Checklist
  - UX_OVERVIEW: Studio/Practice capabilities, quick actions, modes, A/B loop
  - ARCHITECTURE: “Practice Environment/Core”, Settings > Practice
  - LOW_LEVEL_DESIGN: Practice UI flows, timing windows, modes, loop behavior

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
- Persist freeze playhead toggle
- Enhanced highway visuals (icons/skins), stronger theme and typography
- TempoMap playback polish with section labels; click track on/off in Practice
- Review details: per‑instrument stats, timing histogram
