# UI Overview and Roadmap

## Current UI

1. Studio (formerly “Extractor”)
   - Import audio via a file dialog and run transcription.
   - Create a new empty chart and add notes by clicking in the timeline.
   - Load a built-in sample to explore without importing audio.
   - Transport: play/pause, BPM slider, loop A/B, playhead, metronome clicks.
   - Live MIDI record: arm “Record MIDI”, press Play, and perform to lay down notes (snaps to grid).

2. Tutor
   - Practice loaded charts with a per-instrument note highway (lanes) and moving playhead.
   - Color-coded hits: green (on time), purple (late), yellow (early), red (missed).
   - Not-yet-played notes render in blue until evaluated.
   - Play/Pause, BPM control, “Use lesson tempo” (variable tempo via the lesson’s TempoMap), metronome toggle/volume, pre‑roll count‑in, adjustable hit window.
   - Scoring uses millisecond timing derived from the current practice BPM (or lesson tempo when enabled). Uses mapping and latency compensation from Settings.
   - Legend explains color states: green (on‑time), purple (late), yellow (early), red (missed), blue (not yet played).
   - Countdown overlay during pre‑roll displays 3‑2‑1‑Go.
   - “Freeze playhead” mode keeps the playhead centered while notes scroll.

## Quality of Life

- Auto-save preferences
  - Tutor metronome, hit window, pre-roll, and lesson-tempo toggle are saved automatically with a short debounce after changes.
- Highway visuals
  - Alternating lane backgrounds for readability, beat/bar markers with thicker bar lines, and measure numbers along the top.

3. Marketplace
   - Fetches and lists available lessons (placeholder endpoint for now).

4. Settings
   - MIDI input selection with live device enumeration.
   - Audio section with device dropdown (placeholder if backend unavailable), exclusive mode toggle, latency slider, and main volume.
   - Options: app sounds, auto‑preview, high contrast, note streaks, new keys experience.
   - Mapping wizard: visual drum kit, click a pad then hit your kit to bind notes; revert all mappings. Latency calibration implemented (beep + hit average of multiple trials).

## Near-Term Improvements

1. Chart Editing
   - Grid snapping, delete/drag notes, quantization, and bar/tempo markers.
   - Visual staff with drum-specific notation and articulations.

2. MIDI Configuration
   - Latency calibration wizard (beep + hit to measure), per-pad mapping/curve settings.
   - Live MIDI-in audition for editor placement.

3. Transcription Workflow
   - Waveform display, region selection, and manual correction tools.
   - Export to MusicXML/MIDI/JSON from the Studio.

4. Design Polish
   - Theming, spacing, and a more modern visual hierarchy in egui.
