# Taal UX Specification — Screens, Components, Behaviors

This document describes the macro UX for the three primary screens — Studio, Practice, and Settings — with a component catalog for each: layout, appearance, states, interactions, and motion. It serves as the single source of truth for coordination across design and implementation.

Refer to `docs/UI_DESIGN.md` for theme tokens, fonts, icons, and motion primitives used here.

---

## Global Structure

- App Bar (top): product title, section tabs, optional chart chip, global actions.
- App Bar visuals: subtle vertical gradient (Dark: #0F1115→#1A1E27; Light: #FFFFFF→#F4F6F8) with a 1 px bottom divider.
- Tab affordance: icon + label; animated accent underline on active tab (Blue in Dark, Orange in Light).
- Responsiveness: width ≤ 900 px → icons only in tabs and toolbar; 900–1280 px → icons + short labels; ≥ 1280 px → full labels.
- Canvas First: primary content (grid/highway) fills 70–80% of available space.
- Transport Dock (bottom): time-based controls grouped (Play/Pause, BPM, Loop, Metronome, Record).
- Drawers: contextual tools/inspector appear as collapsible side panels; hidden by default.
- Themes: Dark Neon (Studio) and Light Neumorphic (Practice). Gradient Performance is Phase 2.
- Icons: Lucide; font: Inter.

Motion guidelines: all animations ≤ 200 ms, ease-out; gated by Reduced Motion.

---

## Studio — Chart Authoring and Transcription

### Layout
- Top App Bar
  - Left: “Studio” tab active.
  - Center: `Chart Chip` showing current chart title with dropdown: Open…, Import MusicXML…, Close chart.
  - Right: Primary actions group: New, Load Sample, Transcribe (primary), Save/Export… (segmented control).
- Bottom Transport Dock (implemented)
  - Play/Pause, BPM slider + numeric drag, Loop toggle + Start/End fields, Record MIDI, Metronome with gain.
  - Numbers display without decimals; percentages for fractional controls (e.g., volumes).
  - BPM labels are integers; volumes and gains show integer percents.
- Left Tools Drawer (collapsible)
  - Piece selector, Velocity, Grid total beats, Snap.
- Right Inspector (collapsible)
  - Selection properties, Quantize Selected/All, Undo/Redo list, Waveform toggle.
- Center Canvas
  - Note grid + waveform overlay, banded rows, ruler at top for bars/beats.
- Bottom Transport Dock
  - Play/Pause, BPM slider, Loop A/B (handles on ruler), Record MIDI, Metronome + gain.

### Appearance
- Canvas neutrals: Dark `neutral_surface` and `neutral_panel` with 2–3% value contrast between lanes.
- Bar vs beat lines: bar lines stroke 2 px, beat lines 1 px.
- Playhead: single 2 px accent line; small arrowhead marker in the top ruler only. No glow.
- Lane Labels: text on left; each has inline `M` and `S` pills.
- Buttons: primary (Transcribe) uses orange gradient and inner highlight; others are flat with subtle hover. Focus ring is 2 px accent.

### Interactions
- Chart Chip
  - Click to open dropdown; options close menu on selection.
  - Keyboard: Ctrl/Cmd+O open, Ctrl/Cmd+S save, Ctrl/Cmd+E export.
- Tools Drawer
  - Piece selection updates “add note” tool; Velocity slider sets default velocity.
  - Snap sets placement grid.
- Inspector
  - Quantize operations apply to current selection.
- Canvas
  - Click to add; drag to move; Del/Backspace to remove.
  - Ctrl+Wheel zoom, Middle-drag pan; Ctrl+drag on ruler sets Loop A/B.
- Transport Dock
  - Space toggles Play; L toggles loop; R arms “Record MIDI”.
  - Loop A/B handles are draggable in the ruler; numeric A/B are editable in the dock when Loop is enabled.

### Motion
- Transport controls scale 1.03 on press.
- Record state adds a 2 px orange top border to the app bar.
- Optional pre‑light band ahead of playhead (100 ms) can be enabled from Settings → Appearance.
- Loop handles breathe subtly (disabled when Reduced Motion is on).

---

## Practice — Play & Evaluation

### Layout
- Top Row
  - Mode chips: Free Play, Test.
  - Chart Chip: current title with Replace/Close options.
  - Play/Pause + BPM (or Use Lesson Tempo), Loop toggle.
- Canvas (Highway)
  - Wide lanes with big targets; ruler at top; A/B handles on ruler.
- Bottom Dock
  - Metronome + gain; Evaluation pill (expands to hit windows + caps); Pre‑roll; Freeze Playhead. (Play/BPM/Use lesson tempo moved here.)
  - BPM shows integer values beside the slider; metronome and volumes render as integer percents.
  - Countdown overlay: large centered number with soft circular background; fades during pre‑roll.
  - Review overlay: centered card with encouraging text based on accuracy; per‑instrument lines shown in consistent lane order.

### Appearance
- Theme: Light Neumorphic by default.
- Notes: colored by evaluation state even in Free Play: green (on-time), blue (early), orange (late), red (missed), gray (not‑yet‑played).
- Playhead: 2 px accent line, no circle; optional faint pre‑light band ahead of playhead (user setting under Appearance).

### Interactions
- Mode
  - Free Play: evaluation colors shown; session never “fails”.
  - Test: single pass; opens Review overlay with counters and breakdown.
- Chart Chip
  - Replace/Open/Import/Close; same as Studio.
- Loop
  - Drag A/B handles on ruler; when active, A/B numeric inputs appear in dock.
  - Test loops: configurable (Settings → Practice “Test loops before review”, default 2). Free Play loops indefinitely.

### Motion
- Hit Ripple: 120–180 ms ring on hit lane (accent warm); gated by Reduced Motion.
- Downbeat emphasis: ruler tick at bar start is thicker only; no pulses in the grid.
- Score reveal: slide‑up review card at end of Test.

---

## Settings — Structured Cards with Left Navigation

### Layout
- Left Nav: Audio • MIDI • Practice • Appearance • Accessibility.
- Right Cards
  - Audio: device, latency, volume, exclusive mode, test tone, calibration.
  - MIDI: input selection, mapping wizard launcher, reset mappings.
  - Practice: hit windows, caps, countdown, loop defaults, tempo scaling.
  - Appearance: theme, reduced motion, high contrast, app sounds, auto‑preview.
  - Accessibility: font scaling and colorblind palettes (roadmap).

### Appearance
- Section containers use filled panels without borders (rounded corners, 12 px padding) for a clean, modern look.
- Generous whitespace between controls; toggles instead of checkboxes where appropriate.

### Interactions
- Left nav updates the right pane without scrolling the entire page.
- Mapping wizard opens modal; persists on save.

---

## Component Catalog (Shared)

- Chart Chip
  - States: no chart (button: Open chart…), has chart (title + ▾), disabled when background task running.
  - Colors: `text_primary` on surface; hover raises brightness by 6%.
  - Shortcuts: Ctrl/Cmd+O (Open), Ctrl/Cmd+S (Save), Ctrl/Cmd+E (Export).

- Playhead
  - 2 px stroke `accent_warm` in Light, same in Dark.
  - Arrowhead marker in ruler only; no glow.

- Loop Handles
  - Small capsules on ruler; active color `accent_cool`; breathing animation in normal mode.

- Metronome
  - Checkbox + gain slider; pulses only as short audio and ruler tick accent, no UI ring.

- Hit Ripple (Practice)
  - Expands 6→26 px over 180 ms, alpha fades to 0; disabled when Reduced Motion is ON.

- Lane Mute/Solo Pills
  - ‘M’ and ‘S’ next to lane labels (left gutter) arranged horizontally; fixed-width pills to avoid overlap.
  - Mutual exclusivity per lane: enabling Solo clears Mute for that lane; enabling Mute clears Solo for that lane.
  - Global rule: if any lanes are soloed, only soloed lanes produce sound; other lanes dim and are silent even if not muted.
  - Visual treatment: muted lanes dim; non‑soloed lanes dim while solo set is non‑empty.
  - Tooltips: M → “Mute this lane. If any lanes are soloed, only soloed lanes will play.” S → “Solo this lane. When any lane is soloed, all non‑solo lanes are muted.”
  - Light theme labels use theme `fg_stroke` color for readability; lane banding slightly darker in Light.

---

## Open Questions
- Toolbar responsiveness thresholds (when to collapse labels and show icons only).
- Inspector contents for selection types (multi vs single note).
- Where to persist optional pre‑light band setting (Appearance or Practice?).

---

## Implementation Phases

1. Layout pass (Studio/Practice): app bar with Chart Chip, bottom transport dock, move/remove redundant rows.
2. Canvas pass: playhead, ruler, lane M/S pills; Free Play evaluation colors.
3. Settings pass: left nav + cards; move options to relevant cards.
4. Motion pass: hit ripples, loop handle breathing, review slide‑up; Reduced Motion gates.
5. Polish pass: icons, gradients, segmented toolbar, accessibility tweaks.

This doc will be kept in lock‑step with code and visuals.
