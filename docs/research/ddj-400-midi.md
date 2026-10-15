# Pioneer DJ DDJ-400 MIDI reference

This note records the MIDI values used by Deckforge and separates the official message list from live observations on the connected controller.

## Sources and observations

- **Official MIDI reference:** [DDJ-400 MIDI Message List E1](https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-400/DDJ-400_MIDI_Message_List_E1.pdf), AlphaTheta.
- **Audio setup reference:** [DDJ-400 Mac setup guide](https://forums.pioneerdj.com/hc/en-us/articles/360029307091-DDJ-400-Troubleshooting-Mac).
- **Live CoreMIDI ports:** both input and output are named exactly `DDJ-400` on the development Mac.
- **Live CoreAudio device:** `DDJ-400 Audio Out`, four output channels at 44.1 kHz.
- **Representative observed input:** deck 1 pad `[97 00 7F]`, jog touch `[90 36 7F]`, jog relative values around `[B0 22 3F]` / `[B0 22 41]`, fader `[B0 13 xx]` + `[B0 33 xx]`, tempo `[B0 00 xx]` + `[B0 20 xx]`.

Status values below are hexadecimal. MIDI channel numbers in the manual are one-based; the low nibble in a MIDI status byte is zero-based.

## Channel assignment

| Surface | MIDI channel | Status nibble |
|---|---:|---:|
| Deck 1, excluding pads | 1 | `0` |
| Deck 2, excluding pads | 2 | `1` |
| Beat FX | 5 | `4` |
| Browser and global mixer | 7 | `6` |
| Deck 1 pads, normal / SHIFT | 8 / 9 | `7` / `8` |
| Deck 2 pads, normal / SHIFT | 10 / 11 | `9` / `A` |
| Controller feedback | 16 | `F` |

## Deck controls

| Control | Message |
|---|---|
| PLAY / PAUSE | Note `0B`; SHIFT function `47` |
| CUE | Note `0C`; SHIFT function `48` |
| SHIFT | Note `3F` |
| BEAT SYNC | Note `58`; long press `5C`; SHIFT function `60` |
| LOOP IN / 4BEAT | Note `10`; long-press event `14` |
| LOOP OUT | Note `11`; SHIFT function `4E` |
| RELOOP / EXIT | Note `4D`; SHIFT function `50` |
| CUE / LOOP CALL left, right | Notes `51`, `53`; SHIFT functions `3E`, `3D` |
| Jog touch | Notes `36`; SHIFT function `67` |
| Jog platter | CC `22` in vinyl mode; CC `23` with vinyl mode off |
| Jog wheel edge | CC `21` |
| SHIFT + jog | CC `29` |
| Fader Start | Notes `66` (play) and `52` (cue) |
| Channel cue | Note `54`; SHIFT function `68` |

Relative jog values are centered at `0x40`: clockwise increases from `0x41`, counterclockwise decreases from `0x3F`.

## Mixer controls

The following knob/fader controls use 14-bit CC pairs: the LSB controller number is the MSB number plus `0x20`.

| Control | MSB / LSB | Channel |
|---|---|---|
| Tempo | `00` / `20` | Deck 1/2 |
| Trim | `04` / `24` | Deck 1/2 |
| EQ High | `07` / `27` | Deck 1/2 |
| EQ Mid | `0B` / `2B` | Deck 1/2 |
| EQ Low | `0F` / `2F` | Deck 1/2 |
| Channel fader | `13` / `33` | Deck 1/2 |
| Crossfader | `1F` / `3F` | Browser/global |
| Master level | `08` / `28` | Browser/global |
| Headphones mix | `0C` / `2C` | Browser/global |
| Headphones level | `0D` / `2D` | Browser/global |
| Channel 1/2 filter | `17` / `37`; `18` / `38` | Browser/global |

In Deckforge, the physical CFX/filter knobs directly control each channel's bipolar Color FX amount. Master and headphone output knob data is displayed in the app; the DDJ-400 handles its hardware headphone circuit when hardware headphone handling is enabled.

## Browser and load

| Control | Message | Channel |
|---|---|---|
| Browse rotary | CC `40` | Browser/global |
| SHIFT + browse rotary | CC `64` | Browser/global |
| Browse push | Note `41`; SHIFT `42` | Browser/global |
| LOAD deck 1 / deck 2 | Notes `46` / `47` | Browser/global |
| SHIFT + LOAD deck 1 / deck 2 | Notes `68` / `7A` | Browser/global |

## Pad modes and pad notes

Physical mode-button notes are on deck channel 1/2:

| Button gesture | Note |
|---|---:|
| HOT CUE | `1B`; SHIFT+HOT CUE selects Keyboard `69` |
| BEAT LOOP | `1E`; SHIFT+BEAT LOOP selects PAD FX 1 `6D` |
| BEAT JUMP | `20`; SHIFT+BEAT JUMP selects PAD FX 2 `6B` |
| SAMPLER | `22`; SHIFT+SAMPLER selects Key Shift `6F` |

Pad presses use mode-specific note ranges on channels 8–11. The upper nibble selects the mode; the lower nibble is pad index 0–7.

| Mode | Note range |
|---|---|
| Hot Cue | `00`–`07` |
| PAD FX 1 | `10`–`17` |
| Beat Jump | `20`–`27` |
| Sampler | `30`–`37` |
| Keyboard | `40`–`47` |
| PAD FX 2 | `50`–`57` |
| Beat Loop | `60`–`67` |
| Key Shift | `70`–`77` |

## Beat FX

All Beat FX controls are on channel 5 (`0x94` for Note On, `0xB4` for CC).

| Control | Message |
|---|---|
| Beat left / right | Notes `4A` / `4B` |
| SHIFT + beat left / right | Notes `66` / `6B` |
| FX select forward / backward | Notes `63` / `64` |
| FX channel target: Deck 1 / Deck 2 / Master | Note On `10` / `11` / `14` |
| LEVEL/DEPTH | CC `02` / `22` 14-bit pair |
| FX ON/OFF; SHIFT release function | Notes `47` / `43` |

The channel selector sends the selected target as Note On and sends Note Off for the prior target. Deckforge treats the three Note On values as absolute target selection. The MIDI list documents the FX ON/OFF LED behavior: Note On flashes and Note Off lights steadily.

## Audio routing and lifecycle

The four-channel DDJ-400 audio output is routed as Master outputs 1/2 (`[0,1]`) and Phones outputs 3/4 (`[2,3]`). When connected, Deckforge selects the DDJ-400 output. On disconnect it falls back to the saved non-DDJ output when available, otherwise the system default. An explicitly saved hardware headphone-handling preference takes precedence; when there is no saved preference Deckforge enables hardware headphone handling for the DDJ-400.

The DDJ-400 mapping does not require a proprietary keepalive message. No MIDI output is sent for position polling.
