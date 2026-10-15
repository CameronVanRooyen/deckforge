# FX v2 design

This document combines the sound-design proposals in `fx-pro-sound.md` and `fx-tension.md` with the DDJ-400 control surface now implemented in Deckforge. The DDJ-400 has one Beat FX section, per-channel Color FX/filter knobs, and two eight-pad banks per deck; effects that have no mapped hardware control remain available in the app UI.

## Sound design

The sound-design goals remain:

- Change Echo, Delay, Ping Pong and Up/Down Echo divisions without audible pitch jumps or clicks.
- Keep low frequencies clean in echo and reverb tails, with natural darkening and controlled saturation.
- Give Space a wet-only high-frequency shimmer while keeping the dry path and bass intact.
- Use frequency-dependent reverb decay, subtle modulation and dry-keyed ducking to keep tails clear.
- Make Noise sweep upward as a riser and duck it under the playing track.
- Apply Beat FX to selectable LOW/MID/HIGH bands through the existing isolator split.
- Continue developing tension effects such as Pulse Verb, Dub Tape, Halo, Ascend and Backwash as separate effects, without adding a proprietary controller-only macro layer.

These are sound-design goals; detailed values and open audio questions are in `../research/fx-pro-sound.md` and `../research/fx-tension.md`.

## DDJ-400 controls

- **Beat FX LEVEL/DEPTH** controls effect level. Holding deck SHIFT changes it to the currently focused effect parameter; SHIFT + BEAT left/right changes the focused parameter.
- **FX SELECT** cycles effects, and SHIFT + FX SELECT cycles backward. Beat left/right changes the current effect's division or primary parameter.
- **FX ON/OFF** toggles the Beat FX. SHIFT + FX ON/OFF triggers Release FX.
- **CH SELECT** chooses Deck 1, Deck 2 or Master as the Beat FX target.
- **Color FX/filter knobs** directly control each channel's bipolar filter amount.
- **PAD FX 1** provides momentary effects and Release FX actions per deck. **PAD FX 2** provides beat/parameter quick selection and effect favourites.
- Pads, loops, hot cues, beat jump, jog, mixer, browser, load/eject, LEDs and both decks follow the published DDJ-400 MIDI message list.

The complete byte table and live CoreMIDI observations are in `../research/ddj-400-midi.md`; hardware validation status is in `../hardware-verification.md`.

## Current implementation boundary

Deckforge supports the DDJ-400 as its hardware controller. General Beat FX, Color FX, Pad FX and Release FX remain available. Controller-only Smart CFX macros are not part of this fork; there is no corresponding mode, state, command or UI.
