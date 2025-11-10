# Taal

Taal is a drum tutoring software built around two primary experiences:

1. **Studio (formerly Extractor)** – Import audio via file dialog, transcribe into editable notation, or start a new chart from scratch.
2. **Practice** – Connect electronic drum kits over MIDI for real-time feedback and practice tools.

## Documentation Index

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) – high-level system overview and roadmap.
- [`docs/LOW_LEVEL_DESIGN.md`](docs/LOW_LEVEL_DESIGN.md) – module-by-module responsibilities ready for implementation.

## Development Environment Setup

1. **Install Rust**
   - Use the official installer at <https://rustup.rs/> (supports Windows, macOS, and Linux).
   - After installation, ensure the toolchain is available: `rustc --version` and `cargo --version`.
2. **Add Required Targets (optional for cross-platform builds)**
   - Windows users planning to build native GUIs should install the [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio) with the "Desktop development with C++" workload.
   - macOS users should install Xcode Command Line Tools: `xcode-select --install`.
3. **Install System Dependencies**
   - Audio backends rely on platform SDKs:
     - Windows: ASIO SDK (optional), WASAPI provided by the OS.
     - macOS: CoreAudio/CoreMIDI (included).
     - Linux: `alsa-lib` headers (`sudo apt install libasound2-dev`) and optionally JACK.
4. **Clone the Repository**
   ```bash
   git clone https://github.com/<your-org>/Taal.git
   cd Taal
   ```
5. **Bootstrap the Workspace**
   - The repository is now a Cargo workspace with dedicated crates for the domain model, audio utilities, transcription engine, tutor core, services, and the desktop application shell.
   - Fetch dependencies and build all crates: `cargo check`.
   - Format code before committing: `cargo fmt`.

## Workspace Layout

```
Taal/
├─ Cargo.toml
├─ crates/
│  ├─ domain/            # Shared data structures and serialization helpers
│  ├─ audio/             # Audio backend abstraction, DSP helpers, classifier glue
│  ├─ transcriber/       # Audio → notation pipeline plus CLI
│  ├─ notation/          # `egui` widgets for drum staff rendering and editing
│  ├─ tutor/             # MIDI integration, practice session logic, scoring
│  └─ services/          # Marketplace and cloud-service integration stubs
├─ apps/
│  └─ desktop/           # Unified GUI combining extractor, tutor, marketplace panes
└─ tools/
   └─ dataset-pipeline/  # Annotation preparation utilities for the classifier
```

## Running Key Components

- **Desktop App:** `cargo run -p taal-desktop` launches the GUI with Studio, Tutor, Marketplace, and Settings.
- **Transcription CLI:** `cargo run -p taal-transcriber -- <path-to-audio>` prints a JSON transcription using the current mock pipeline.
- **Dataset Tool:** `cargo run -p dataset-pipeline -- <annotations.json>` validates and counts classifier annotations.

Each crate includes targeted unit tests. Execute `cargo test --workspace` for the full suite (requires network access to download dependencies on first run).

## What’s New in the UI

- Bottom transport docks (Studio & Practice): play/pause, integer BPM labels, loop toggle with Start/End, metronome gain as percentage. Studio adds Record MIDI.
- Ruler A/B handles: draggable loop handles in the ruler (Studio live; Practice to follow), numeric A/B in the dock.
- Mute/Solo pills per lane: horizontal pills in lane gutter, mutually exclusive per lane; global Solo dims others.
- Light/High‑Contrast polish: higher contrast tracks and labels in Light; high‑contrast strengthens strokes for the active theme.
- Countdown overlay: large centered numerals with soft circular background; counts in seconds (not BPM).
- Review overlay: centered card, consistent instrument order, encouraging summary text.
- Icons: Lucide SVGs tinted at runtime; see `docs/ASSETS.md` for exact list.

### Practice Settings
- Configurable hit windows (percent of beat + ms caps), countdown behavior, “Test loops before review” (default 2), default tempo scaling, and pre‑roll beats. See `docs/UX_SPECS.md` and `docs/ARCHITECTURE.md`.

See also: `docs/UX_OVERVIEW.md` for the short roadmap.

## MusicXML Import

- Import `.musicxml`/`.xml` drum charts in the Studio via “Import MusicXML”.
- Handles multi‑voice layering and chords; reads tempo from `<sound tempo>` or `<metronome><per-minute>`.
- Instrument mapping sources:
  - Preferred: `<notations><technical><instrument>` text (e.g., “Hi-Hat Closed”, “Bass Drum”, “Crash Cymbal”, “High Tom”, “Mid Tom”).
  - Fallback: heuristics from `<notehead>` and `<unpitched><display-step>/<display-octave>` (x‑head cymbals; F4≈Kick, C5≈Snare, E/D5≈Toms).
- Supported pieces: Kick, Snare, Hi‑Hat (closed/open via `<open/>` articulation), Ride, Crash, High/Low/Floor Tom.
- Tip: For best results, include `<instrument>` on first occurrence of each piece; the importer will still infer when omitted.

## What “Transcribe” Does Today

The current transcriber is a functional prototype meant to validate data flow end‑to‑end:

- Decodes audio using `symphonia` (WAV/MP3/FLAC/AAC/Vorbis supported by enabled features).
- Estimates a placeholder tempo from signal length.
- Performs a simple quantization pass that emits alternating bass/snare events with velocities derived from local energy.
- Exports a `LessonDescriptor` as JSON via the domain exporter.

This is not a production model yet. Tempo tracking, onset detection, drum classification, and quantization are intentionally simple and will be replaced.

Recommended input: short mono/stereo files at 44.1k or 48k sample rate in common formats (WAV/MP3/FLAC/OGG/AAC). Very long files will work but are slower to process.

## Notable Implementation Choices

- TLS stack uses `rustls` via `reqwest` to avoid system OpenSSL requirements.
- Audio decoding uses `symphonia` with features for `aac`, `flac`, `mp3`, `vorbis`, and `wav`.
- `realfft` is pinned to `3.5` (future DSP), not yet used in code.

## Docs Update Checklist
- When adding or changing UI features, update:
  - `docs/UX_OVERVIEW.md` (user-facing flow, controls, legends)
  - `docs/ARCHITECTURE.md` (high-level modules, Settings surfaces)
  - `docs/LOW_LEVEL_DESIGN.md` (crate responsibilities and state machines)
  - `README.md` (Features or What’s New summaries)
- Keep terminology consistent between Studio and Tutor (e.g., “Chart”).
- If Settings gain new options, add them to the Settings pane and persisted model.
