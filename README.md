# Taal

Taal is a drum tutoring software built around two primary experiences:

1. **Drum Sheet Extractor** – Transcribe drum performances from audio into editable notation.
2. **Interactive Tutoring** – Connect electronic drum kits over MIDI for real-time lessons, feedback, and practice tools.

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

- **Desktop App:** `cargo run -p taal-desktop` launches the GUI with extractor, tutor, and marketplace placeholders.
- **Transcription CLI:** `cargo run -p taal-transcriber -- <path-to-audio>` prints a JSON transcription using the mock pipeline.
- **Dataset Tool:** `cargo run -p dataset-pipeline -- <annotations.json>` validates and counts classifier annotations.

Each crate includes targeted unit tests. Execute `cargo test --workspace` for the full suite (requires network access to download dependencies on first run).
