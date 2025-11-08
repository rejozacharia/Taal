# PR: Modernize Studio + Tutor UI, MIDI mapping, and Settings

## Summary
This PR refreshes the Desktop app with a modern authoring + practice experience inspired by Melodics.

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
  - Debounced autosave for Tutor prefs (metronome, hit window, pre-roll, use-lesson-tempo)

- Tutor
  - Note highway (lanes: Crash/Ride/HH/Snare/Toms/Kick); beat/bar markers with measure numbers
  - Color states: green (on time), purple (late), yellow (early), red (missed), blue (not yet played)
  - Legend under highway; freeze playhead (scroll notes) option
  - Transport: Play/Pause, BPM; Use lesson tempo (TempoMap-driven playhead)
  - Metronome toggle/volume; pre-roll countdown overlay (3-2-1-Go)
  - Live MIDI hits mapped + latency-compensated; adjustable hit window (ms)
  - Ms-based scoring (score_with_spb) derived from practice BPM

- Docs
  - README: new UI, controls, and modes
  - UX_OVERVIEW: Studio/Tutor capabilities, mapping, calibration, autosave
  - ARCHITECTURE/LLD: Extractor → Studio wording and flow

## Code Highlights
- apps/desktop/src/main.rs: Studio + Tutor UI integration, SettingsPane autosave, mapping wizard, metronome/tone
- crates/notation/src/lib.rs: editor API + draw_with_timeline with loop/playhead
- crates/tutor/src/scoring.rs: score_with_spb for ms-based evaluation
- crates/domain/src/tempo.rs: time_at_beat + supportive helpers

## Testing
- cargo build --workspace: passes
- Manual smoke tests recommended:
  - Studio: pick audio → see waveform; add/move/delete notes; loop/zoom; metronome click; record MIDI
  - Settings: refresh audio/MIDI, test tone, toggle high-contrast; open mapping wizard; run latency calibration; confirm autosave
  - Tutor: toggle Use lesson tempo; Play/Pause; pre-roll overlay; metronome; watch highway colors; freeze playhead on/off; adjust hit window; Score Performance

## Follow-ups
- Persist freeze playhead toggle
- Enhanced highway visuals (icons/skins), stronger theme and typography
- TempoMap playback polish with section labels; click track on/off in Tutor
- Scoring weighting curves and per-lane stats
