# Low-Level Architecture & Module Responsibilities

This document dives deeper into each planned crate/application within the Taal workspace so that implementation work can start without additional upfront design.

## Workspace Layout
```
Taal/
├─ Cargo.toml            # Workspace manifest (to be created)
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
- `io`: MusicXML/MEI/MIDI import/export adapters using feature flags.

Dependencies:
- `serde` with `serde_json` and `serde_yaml` for storage.
- `time` for precise timestamp handling.

### `crates/audio`
Purpose: Common audio utilities used by both the transcriber and the tutoring playback engine.

Key modules:
- `backend`: abstraction trait over `cpal` streams with buffer size negotiation and latency measurement utilities.
- `dsp`: resampling, filtering, onset envelopes, and spectral transforms.
- `analysis`: wrappers over ONNX Runtime sessions for instrument classification.
- `io`: audio file decoding via `symphonia`.

Threads:
- Real-time audio thread owning the stream, communicating with analysis/playback tasks via lock-free ring buffers (`ringbuf`).

### `crates/transcriber`
Purpose: Convert audio into structured drum notation.

Key modules:
- `pipeline`: orchestrates ingestion → preprocessing → onset detection → instrument classification → quantization.
- `tempo`: tempo detection (combines autocorrelation and Bayesian beat tracking) producing a tempo map for quantization.
- `notation`: maps classified hits into `domain::events` and `domain::io` export formats.
- `cli`: optional binary exposing batch processing and JSON/CLI reporting.

Data flow:
1. Audio buffer from `audio::io`.
2. Preprocess with `audio::dsp` (normalization, HP filter).
3. Onset detection (`pipeline::onset`) yields candidate hits.
4. Classifier attaches instrument labels + confidence scores.
5. Quantizer snaps to tempo grid and emits domain events.
6. Writer exports to MusicXML/MIDI using `domain::io`.

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

### `crates/services`
Purpose: Optional networking and marketplace integration.

Key modules:
- `api`: HTTP client wrappers (using `reqwest`) for marketplace endpoints.
- `auth`: token storage and refresh flows.
- `sync`: upload/download lesson packs and practice history.

## Applications & Tools

### `apps/desktop`
Primary GUI application bundling the extractor and tutoring experiences.

Structure:
- `app.rs`: sets up `egui` window, handles top-level state machines between Extractor/Tutor/Marketplace tabs.
- `extractor_ui`: hosts waveform view, classification lane timeline, notation editor, and export drawer.
- `tutor_ui`: lesson browser, kit visualizer, scoring overlays, session controls.
- `market_ui`: placeholder for future marketplace integration.

State Management:
- Use the `tauri`-style pattern with a central `AppState` struct referencing the active project, audio devices, and loaded lessons.
- Background tasks run on `tokio` runtime to execute transcription without blocking the UI.

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
4. `apps/desktop` hosts navigation between Extractor and Tutor tabs and shows placeholder content for each.

With this document in place we can begin implementing the workspace according to the `Development Roadmap` in `docs/ARCHITECTURE.md`.
