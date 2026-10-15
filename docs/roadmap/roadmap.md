# Roadmap

## Done (MVP foundation)
- M1 Two-deck audio foundation — native Rust engine, symphonia decode, cubic varispeed, shared mixer, cpal output.
- M3 Routing — single synchronized multichannel stream; master 1-2, headphones 3-4; PFL independent of fader/crossfader; stereo fallback (master-only / split-mono); hardware-phones mode for the DDJ-400.
- M5 Performance features — BPM/beatgrid analysis, tempo ranges, tempo+phase sync with continuous correction, CDJ cue, 8 hot cues (persisted), manual/beat loops, beat jump, quantize, bipolar filter, 3-band isolator, WSOLA key lock, jog scratch/bend/search.
- DDJ-400 mapping — implemented from the official MIDI list and verified with live CoreMIDI input; complete hardware checks are in `docs/hardware-verification.md`.

## Next
1. Complete the DDJ-400 hardware verification checklist: mapped controls, LED feedback and Master/Phones output routing.
2. Tune jog scratch feel (velocity filter, touch ramp) with real hardware.
3. Measure real output latency (buffer 128/256) and xruns on the DDJ-400.

## Performance-mode polish
- Slip mode; reverse; censor.
- Memory cues (slots ≥ 8 in `cues` table), saved loops.
- Beat FX (echo, reverb, flanger) mapped to the DDJ-400 Beat FX section; Pad FX.
- Sampler pad-mode integration.
- Higher-quality time stretch (phase-vocoder with transient preservation) and an A/B against WSOLA.
- Dynamic beatgrids (tempo-varying tracks); downbeat (bar) detection.
- Musical key detection (chromagram + key profiles) with Camelot display.

## Phase 2 — Export mode
- Advanced library: smart playlists, tags, ratings, colours, history.
- Harmonic mixing tools (Camelot wheel, key compatibility in browser).
- Advanced beatgrid editor (drag/tap, per-section grids).
- rekordbox XML import (collection, cues, grids) and playlist export (M3U8 / XML).
- USB library export through documented/supported formats only.

## Cross-platform
- Windows: cpal WASAPI (ASIO via the `asio` feature + Steinberg SDK where licensing allows); midir WinMM.
- Linux: cpal ALSA/JACK/PipeWire; midir ALSA. DDJ-400 class compliance is to be verified there.
- CI matrix builds for macOS/Windows/Linux; packaging via `tauri build`.
