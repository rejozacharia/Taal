# UI Assets Layout

This repo expects application-bundled UI assets under `assets/`.

- Fonts
  - `assets/fonts/Inter-Regular.ttf`
  - `assets/fonts/Inter-Bold.ttf`
  - License: SIL Open Font License (include `assets/fonts/OFL.txt`).

- Icons (Lucide SVG)
  - `assets/icons/lucide/` with individual SVGs, e.g.:
    - `file-plus.svg`, `music.svg`, `folder-open.svg`, `waveform.svg`, `save.svg`, `settings.svg`, `help-circle.svg`.
  - License: ISC; keep `LICENSE` file from Lucide in the same directory.

Integration notes
- The examples in `docs/UI_DESIGN.md` use `include_bytes!` with these relative paths from a UI crate/module.
- For runtime loading (optional), mirror the same structure and provide a configuration knob.
- Dark/Light: one SVG set is sufficient. Icons are tinted at runtime based on theme and state (default/hover/active/disabled). Avoid baking colors into SVGs unless brand requires it.
- Accent color: icons follow the app accent (Settings → Appearance) via `visuals.selection.bg_fill`.
- Loader aliasing: the runtime loader supports renamed Lucide glyphs. For example: `waveform` → `audio-waveform`, `export` → `share`, `record` → `circle-dot`, `metronome` → `timer`, `sliders` → `sliders-horizontal`, `help-circle` → `circle-question-mark`, `alert-triangle` → `triangle-alert`.

## Exact Icon List (initial)

Legend: path `assets/icons/lucide/<name>.svg` • [Screen] usage • Notes/tooltip hints.

### App Tabs
- `sliders.svg` • [Studio] • Tab icon for Studio; “Arrange and edit charts”.
- `drum.svg` (or `drumstick.svg` / `music-2.svg` alt) • [Practice] • Tab icon for Practice; “Play and get feedback”.
- `shopping-cart.svg` • [Marketplace] • Tab icon for Marketplace; “Browse lessons”.
- `settings.svg` • [Settings] • Tab icon for Settings; “Configure devices and preferences”.

### Studio — Top Toolbar
- `file-plus.svg` • New Chart • “Create a new empty chart”.
- `music.svg` • Load Sample • “Load a sample groove”.
- `folder-open.svg` • Open Chart • “Open an existing chart (.json)”.
- `waveform.svg` or `mic.svg` • Transcribe • “Transcribe audio into a chart”.
- `save.svg` • Save • “Save chart (.json)”.
- `export.svg` (or `share.svg`) • Export • “Export as MIDI / MusicXML”.

### Studio — Transport Dock
- `play.svg` • Play • “Start preview”.
- `pause.svg` • Pause • “Pause preview”.
- `repeat.svg` • Loop • “Loop between Start and End”.
- `record.svg` (alt: `circle-dot.svg`) • Record MIDI • “Capture MIDI into chart”.
- `metronome.svg` (alt: `timer.svg`) • Metronome • “Enable click”.

### Practice — Transport Dock
- `play.svg` / `pause.svg` • Play/Pause.
- `repeat.svg` • Loop region (A/B handles in ruler).
- `metronome.svg` • Metronome toggle.
- `gauge.svg` (alt: `speedometer.svg`) • Use lesson tempo • “Lock to lesson tempo”.

### Settings / Misc
- `help-circle.svg` • Help / tooltips entry points.
- `info.svg` • Info banners / empty states.
- `alert-triangle.svg` • Warning banners.

> Alt names are provided in case a chosen set does not include the preferred glyph. One icon per action is sufficient — we tint them to theme.

## Sizing and Format
- Source: SVG, 24 px viewport, round caps/joints; stroke ≥ 1.5 px for clarity.
- Runtime size targets (desktop/tablet/mobile): 16, 18, 20, 24, 28, 32 px. The same SVG scales cleanly.
- Optional raster fallbacks: export PNG at 1.0× (24), 1.5× (36), 2.0× (48), 3.0× (72) if needed by a platform.

## Tint/States (runtime)
- Default: Dark = Neon Blue `#00B4FF`, Light = Cool Blue `#2B86FF`; Hover: brighten ×1.1; Active: brighten ×1.2; Disabled: `text_muted × 0.6`.
- Use `ImageButton::new((tex, size)).tint(color)` or apply tint to `Image` next to a `SelectableLabel`.
- In code, `taal_ui::icons::default_tint(ui)` picks a readable default for Dark/Light.

```text
assets/
  fonts/
    Inter-Regular.ttf
    Inter-Bold.ttf
    OFL.txt
  icons/
    lucide/
      file-plus.svg
      music.svg
      folder-open.svg
      waveform.svg
      save.svg
      settings.svg
      help-circle.svg
      play.svg
      pause.svg
      repeat.svg
      record.svg
      metronome.svg
      export.svg
      sliders.svg
      drum.svg
      shopping-cart.svg
      LICENSE
```
