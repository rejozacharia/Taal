# Low-Level Architecture & Module Responsibilities

This document dives deeper into each planned crate/application within the Taal workspace so that implementation work can start without additional upfront design.

## Workspace Layout (implemented)
```
Taal/
├─ Cargo.toml            # Workspace manifest
├─ crates/
│  ├─ domain/
│  ├─ transcriber/
│  ├─ notation/
│  ├─ tutor/
│  ├─ audio/
│  └─ services/
├─ apps/
│  └─ desktop/
└─ tools/
   └─ dataset-pipeline/
```

Each directory will contain its own `Cargo.toml` and follow Rust 2021 edition defaults. We keep core logic in library crates and expose binaries only where an executable is required.

## Core Library Crates

### `crates/domain`
Purpose: Define cross-cutting data structures and serialization helpers shared by all other crates.

Key modules:
- `tempo`: tempo map representation, beat grids, and swing descriptors.
- `events`: strongly typed drum events, velocities, articulations, and layout metadata.
- `lesson`: lesson descriptors, progress metrics, and metadata for the tutoring UI.
- `io`: MusicXML/MEI/MIDI import/export adapters using feature flags. MusicXML importer supports:
  - `<sound tempo>` and `<metronome><per-minute>` tempo sources.
  - Layered notes via per‑voice cursors and `<chord/>` handling.
  - Instrument detection from `<notations><technical><instrument>` with keyword mapping (snare, bass/kick, hi‑hat closed/open, crash, ride, tom high/mid/low/floor).
  - Fallback heuristics when `<instrument>` is omitted: evaluate `<notehead>` (x‑head → cymbals), `<unpitched><display-step>/<display-octave>` to infer hats/crash/ride/kick/snare/toms. A weak per‑voice memory is used only if heuristics are unavailable.

Dependencies:
- `serde` with `serde_json` and `serde_yaml` for storage.
- `time` with the `serde` feature for serializing durations.

### `crates/audio`
Purpose: Common audio utilities used by both the transcriber and the tutoring playback engine.

Key modules:
- `backend`: abstraction trait over `cpal` streams with buffer size negotiation and latency measurement utilities.
- `dsp`: resampling, filtering, onset envelopes, and spectral transforms.
- `analysis`: wrappers over ONNX Runtime sessions for instrument classification.
- `io`: audio file decoding via `symphonia`.

Threads:
- Real-time audio thread owning the stream, communicating with analysis/playback tasks via lock-free ring buffers (`ringbuf`).

Notes:
- File decoding via `symphonia` is implemented. Streamed playback is planned.
- `realfft` (v3.5) reserved for future DSP; not yet used.

### `crates/transcriber`
Purpose: Convert audio into structured drum notation.

Key modules:
- `pipeline`: orchestrates ingestion → preprocessing → onset detection → instrument classification → quantization.
- `tempo`: tempo detection (combines autocorrelation and Bayesian beat tracking) producing a tempo map for quantization.
- `notation`: maps classified hits into `domain::events` and `domain::io` export formats.
- `cli`: optional binary exposing batch processing and JSON/CLI reporting.

Data flow (prototype):
1. Audio buffer from `audio::io` using `symphonia`.
2. Basic normalization in `audio::dsp` (available utility).
3. Tempo estimated from buffer statistics (placeholder logic).
4. Quantizer emits alternating bass/snare events from energy (placeholder).
5. Exporter writes JSON via `domain::io` (MusicXML/MIDI later).

### `crates/notation`
Purpose: Visual rendering and editing of drum notation for both the desktop app and potential web exports.

Key modules:
- `layout`: staff layout engine mapping events to glyphs, supports percussion clef positions.
- `render`: `egui`/`wgpu` components for drawing measures, noteheads, articulations.
- `editor`: interaction state (selection, drag, palette drop), quantization overrides, tuplets.
- `playback`: optional integration with `audio` crate for auditioning measures.

### `crates/tutor`
Purpose: Practice logic, scoring, and feedback around MIDI performance.

Key modules:
- `midi`: wrappers over `midir` to enumerate devices, manage input streams, and perform latency calibration.
- `session`: runtime state machines for Learn/Practice/Perform modes.
- `scoring`: match live events to expected notation using tolerance windows and produce per-note feedback.
- `analytics`: compute streaks, accuracy percentages, timing histograms.

Threading model:
- MIDI input callback threads publish events to `session` via lock-free queues.
- Main update loop (owned by the desktop app) polls `session` at frame rate and renders feedback.

#### Practice UI — Low‑Level Design

Terminology:
- Use “Chart” for notated material (keeps Studio/Tutor consistent). Supported inputs: MusicXML, MIDI, and Taal JSON.

User flow (Practice mode):
1. Import Chart: load MusicXML/MIDI/Taal JSON into `domain::events`, build tempo map, and derive an expectation timeline.
2. Configure: select device, map pads/zones, set loop range (default: full sheet) and loop count (default: 2), set tempo scaling (e.g., 70–100%) and click/backing levels.
3. Ready + Countdown: user hits “Ready”; UI shows a preroll count‑in (default 1 bar) with click + visual metronome.
4. Playback + Perform: expectation timeline advances; user plays on e‑drums; MIDI hits are captured and classified in real‑time.
5. Live Feedback: per‑note judgment (Early/On‑Time/Late/Missed/Extra) rendered on staff/scrolling lane and kit visualizer.
6. Looping: on reaching loop end, auto‑rewind and play again until loop count is exhausted; stop after final loop.
7. Review: summary score, timing histogram, per‑measure accuracy; user can save, retry, or adjust loop/tempo.

Core components:
- `tutor::midi`
  - Device enumeration/selection via `midir`.
  - Pad mapping: note/CC to `domain::events::Instrument` (snare, kick, hats open/closed via CC4 threshold, toms, ride, crash, etc.).
  - Latency calibration: tap‑test or loopback; persist per‑device `latency_ms` and optional per‑pad offsets.
- `tutor::session`
  - State machine manages Practice lifecycle and loop control.
  - Ticker integrates tempo map and wall‑clock to produce current playhead position.
  - Emits UI events: countdown ticks, measure changes, judgment updates, end‑of‑loop.
- `tutor::scoring`
  - Matcher aligns incoming MIDI hits to expected onsets using tempo‑aware tolerance windows.
  - Produces `Judgment` with signed timing error (ms), velocity, and matched instrument.
  - Accumulators compute accuracy, streaks, timing distribution, per‑instrument stats.
- `notation::render`
  - Renders staff/scrolling lane with highlight of the current beat and judgment overlays.
  - Loop range visualization and ghosted next notes for anticipation.

State machine (Practice):
- States: `Idle` → `Loading` → `Calibrating?` → `Ready` → `Countdown` → `Playing` (with `Looping`) → `Stopped` → `Review`.
- Events: `ImportSheet`, `MapPads`, `SetLoop`, `SetTempoScale`, `PressReady`, `Tick`, `MidiHit`, `LoopEnd`, `Pause`, `Stop`, `Exit`.
- Transitions:
  - `Ready --PressReady--> Countdown` (initialize preroll, reset accumulators).
  - `Countdown --Tick (0)--> Playing` (start playhead at loop start).
- `Playing --LoopEnd (n < loop_count)--> Countdown|Playing` (configurable: count‑in only on first loop or every loop; default: first only).
  - `Playing --LoopEnd (n == loop_count)--> Stopped --> Review`.

Timing and judgment model:
- Latency compensation: `t_adjusted = t_midi - latency_device - latency_global`.
- Expectation windows derive from local tempo: `beat_ms = 60_000 / bpm_here`.
  - Match window (configurable): default ±12.5% of beat (cap at ±75 ms).
  - Center zone (On‑Time, configurable): default ±7.5% of beat (cap at ±40 ms).
  - Outside match window → `Missed` (if expected had no match) or `Extra` (if incoming cannot map to any expectation).
- One‑to‑one matching: each expected note matches at most one incoming hit; greedy by absolute error, then by velocity proximity.
- Chords/polyphony: expectations at identical timestamps require per‑instrument matching; handle hi‑hat openness via CC4 snapshot near the hit.
- Dynamics: optional grading band (pp–ff) from expected velocity; mismatches recorded but do not fail a match.

- Settings: timing windows are user‑configurable in `Settings > Practice` and persisted per machine.

Modes:
- Free Play: infinite looping by default, no “Missed” penalties, continuous real‑time feedback only.
- Test: single pass (A/B region or full chart), show Review summary at end; no looping.

Real‑time feedback rendering:
- Per‑note halo color: Early (blue), On‑Time (green), Late (orange), Missed (red), Extra (purple).
- Small timing “needle” drawn perpendicular to note stem proportional to error (clamped to ±75 ms).
- Last‑N judgments summary widget and per‑drum mini meters.

Looping behavior:
- Defaults: loop range = full chart; loop count = 2; countdown only on first loop; stop after final loop.
- A/B loop: optional region [A,B] set from UI; used as loop/playback range instead of full chart.
- Options: “infinite loop”, “countdown every loop”, “auto‑advance selection to next phrase”.

Scoring summary (Review screen):
1. Overall: accuracy %, average abs timing error, early/late bias, streak max.
2. Per‑measure heatmap and list of tough bars.
3. Per‑instrument accuracy and timing stats.
4. Save result to history with device/latency snapshot; export CSV/JSON.

Data structures (sketch):
- `Expectation { t_ms, instrument, velocity_hint, measure_idx, is_ghost }`
- `MidiHit { t_ms, note, cc4, velocity }`
- `Judgment { expected_idx?, hit_idx?, category, error_ms?, velocity, instrument }`
- `PracticeConfig { loop_start, loop_end, loop_count, tempo_scale, countdown_bars }`
- `PracticeResult { judgments: Vec<Judgment>, aggregates, started_at, device_id }`

Sequence (text diagram):
1. UI: Import → `session.load(sheet)` → build expectations.
2. User: Ready → `session.start_countdown()` → preroll ticks.
3. Audio/MIDI: `midir` callback → queue `MidiHit` → `scoring.match()` → emit `Judgment`.
4. Ticker: advances playhead → UI renders cursor and expectations.
5. End of loop: either restart (decrement remaining) or stop and compute `PracticeResult` → Review.

### `crates/services`
Purpose: Optional networking and marketplace integration.

Key modules:
- `api`: HTTP client wrappers for marketplace endpoints using `reqwest` with `rustls` TLS.
- `auth`: token storage and refresh flows.
- `sync`: upload/download lesson packs and practice history.

## Applications & Tools

### `apps/desktop`
Primary GUI application bundling the Studio (extractor) and tutoring experiences.

Structure:
- `app.rs`: sets up `egui` window, handles top-level state machines between Studio/Tutor/Marketplace/Settings tabs.
- `studio_ui` (formerly extractor): hosts waveform view, classification lane timeline, notation editor, and export drawer.
- `tutor_ui`: lesson browser, kit visualizer, scoring overlays, session controls.
- `market_ui`: placeholder for future marketplace integration.

State Management:
- Use the `tauri`-style pattern with a central `AppState` struct referencing the active project, audio devices, and loaded lessons.
- Background tasks run on `tokio` runtime to execute transcription without blocking the UI.

Implementation snapshot:
- Desktop app wires Studio/Tutor/Marketplace tabs to crate APIs.
- Error paths avoid non-Send/Sync GUI errors; logging via `tracing`/`tracing-subscriber`.

### `tools/dataset-pipeline`
Utility crate/binary for preparing labeled datasets for the classifier.

Responsibilities:
- Convert annotated audio datasets into the feature format required by the ONNX classifier.
- Split training/validation sets, generate augmentation, and package metadata.

## Sequencing & Dependencies
- Implement `domain` first to stabilize core types.
- Build `audio` helpers next since both `transcriber` and `tutor` rely on them.
- Develop `transcriber` minimal slice (WAV input → onset detection → JSON events) before full notation rendering.
- Parallelize work on `notation` once `domain` event structures are settled.
- Assemble `apps/desktop` skeleton early to validate UI concepts; integrate modules incrementally.

## Coding Conventions
- Prefer async tasks for IO-heavy work (`tokio`), but keep audio/MIDI callbacks lock-free and minimal.
- Wrap unsafe audio backend calls in thin, well-documented abstractions.
- Derive `serde::{Serialize, Deserialize}` for all domain types to simplify persistence.
- Use `anyhow::Result` for CLI/binary entry points, `thiserror` for library errors.

## Definition of Done (initial milestones)
1. `domain` crate exposes tempo maps, drum events, and MusicXML export traits with unit tests.
2. `audio` crate can stream audio from disk and capture microphone/e-drum input with latency measurement.
3. `transcriber` crate CLI converts a WAV file into a JSON event list with tempo map.
4. `apps/desktop` hosts navigation between Studio, Tutor, Marketplace, and Settings tabs and shows content for each.

With this document in place we can begin implementing the workspace according to the `Development Roadmap` in `docs/ARCHITECTURE.md`.
