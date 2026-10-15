# Changelog

All notable changes to deckforge are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
- Forked as a DDJ-400-focused build with observed CoreMIDI port matching, four-channel Master/Phones audio routing, and a DDJ-400 control mapping.
- Removed the previous FLX4-only controller features, including Smart CFX.

## [0.1.0] - 2026-10-15

First public release.

### Added
- Native two-deck engine with sync, key lock, quantize, hot cues, loops and beat jump.
- Track analysis: BPM, beatgrid, key, phrases and three-band waveforms rendered with `wgpu`.
- Mixer with DJM-style EQ and Isolator curves, Colour FX, Smart CFX, 17 Beat FX, Pad FX and Release FX.
- SQLite library with folder import, playlists and Spotify playlist matching.
- Full Pioneer DJ DDJ-FLX4 mapping with LED feedback.
- macOS, Windows and Linux builds.

[Unreleased]: https://github.com/CameronVanRooyen/deckforge/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/CameronVanRooyen/deckforge/releases/tag/v0.1.0
