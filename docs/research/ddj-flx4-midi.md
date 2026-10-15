# Pioneer DJ DDJ-FLX4 - MIDI and device reference for deckforge

Research date: 2026-10-13. Purpose: byte-level reference for the deckforge controller mapping.
No Rust code in this document. Everything here is a fact about the hardware, written in our own words and tables.

## 0. Sources and confidence tags

| Tag | Source | Notes |
|---|---|---|
| PDF | "DDJ-FLX4 List of MIDI messages", Ver 1.0, (c) 2022 AlphaTheta, 5 pages. `https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-FLX4/DDJ-FLX4_MIDI_message_List_E1.pdf` | Official. The old pioneerdj.com URL now redirects to support.alphatheta.com; the PDF link is on `support.alphatheta.com/en-US/articles/12294087060377` ("DDJ-FLX4 - MIDI-compatible software"). Downloading needs a browser User-Agent header. |
| MAN | "DDJ-FLX4 Operating Instructions" (English), 165 PDF pages. `https://assets.ctfassets.net/q602vtcuu3w3/2vz7GMLHawLZBK6gWpCP9V/26bd62eef3eca3714ddb1d3396d30f7b/DDJ-FLX4_manual_EN.pdf` | Official. Page numbers below are the printed "NN En" footers. |
| MIXXX | `Pioneer-DDJ-FLX4.midi.xml` and `Pioneer-DDJ-FLX4-script.js` from mixxxdj/mixxx `main` (GPL). Mixxx manual says mapping was made on firmware 1.02. | Behavioural cross-check only. Nothing is copied. |
| LSDJ | `protocol-works/lsdj` `docs/midi-ddj-flx4.md` (community, itself derived from Mixxx plus hardware testing) | Weak source; used only where noted. |

Each fact is tagged:

- **VERIFIED**: stated in PDF or MAN (page cited).
- **INFERRED**: from MIXXX/LSDJ, secondary web sources, or reasoning. Needs a hardware check before it is trusted.
- **UNKNOWN**: cannot be determined from available sources. A verification method is given.

Conventions:

- Hex bytes are written `0x..`. `hh` means any data byte `0x00..0x7F`.
- MIDI channel "N" (1-16) corresponds to status low nibble `N-1`. Example: channel 7 = status `0x96` (note-on) or `0xB6` (CC).
- Buttons: Note On, data2 `0x7F` = press, `0x00` = release (PDF: "OFF=0x00, ON=0x7F"). No Note Off status (0x8n) is documented; accept Note On velocity 0 and Note Off defensively.
- "14-bit CC pair": MSB on CC `n`, LSB on CC `n+0x20`. Value = `(msb << 7) | lsb`, range `0x0000..0x3FFF`.
- Where the PDF says "+SHIFT" the controller itself changes what it sends while SHIFT is held (different note number or channel). The SHIFT button also sends its own message, so the host can track shift state.

---

## 1. Device identity and audio

| Fact | Value | Tag |
|---|---|---|
| Marketing name | DDJ-FLX4 | VERIFIED (PDF p1, MAN) |
| Host connection | USB Type-C "device connection" port; second USB-C port is power-only | VERIFIED (MAN p.34 rear panel) |
| Bus power | Works from computer bus power (5 V, 500 mA) unless: low-power port, other devices on same bus, headphones under 32 ohm, or mono plug in phones jack. Otherwise use 9 V 3 A USB adapter | VERIFIED (MAN p.37, p.159) |
| Driver | Class-compliant USB audio and USB MIDI, no driver on macOS/Linux | INFERRED (Mixxx manual statement; MAN audio-setup text for Mac names only `DDJ-FLX4`, p.49) |
| USB vendor ID | `0x2B73` (Pioneer DJ / AlphaTheta) | INFERRED (known Pioneer DJ VID; web search) |
| USB product ID | `0x0045` | INFERRED (Pioneer firmware-update guide mirror, `VID_2B73&PID_0045`); verify with `system_profiler SPUSBDataType`. One third-party guide shows `PID_0038`, which is likely a different FLX model |
| CoreMIDI device/port name | `DDJ-FLX4`. ALSA shows `DDJ-FLX4 MIDI 1`, which suggests one MIDI in + one MIDI out port pair | INFERRED. Match by case-insensitive substring `FLX4`; do not hard-code a full endpoint name. Verify with Audio MIDI Setup or a MIDI monitor |
| CoreAudio device name | `DDJ-FLX4` (rekordbox Mac setup: Audio = `DDJ-FLX4`) | VERIFIED (MAN p.49) |
| Output pairs | Two logical outputs: `DDJ-FLX4 : MASTER` and `DDJ-FLX4 : PHONES` | VERIFIED (MAN p.49) |
| Output channel layout | 4 output channels at 48 kHz: ch 1-2 = MASTER (RCA), ch 3-4 = PHONES (3.5 mm) | INFERRED (Mixxx manual "Master 1-2, Headphones 3-4"; LSDJ measured with `system_profiler`). The MAN confirms two named pairs but not the numeric order |
| USB audio inputs | None usable. The MIC input is analog-mixed inside the unit and "is not routed back to the computer" | INFERRED (Mixxx manual). Verify: `system_profiler SPAudioDataType` should show 0 input channels |
| Sample rates | 44.1 kHz and 48 kHz | VERIFIED (MAN p.159) |
| Converter depth | 16-bit / 24-bit | VERIFIED (MAN p.159) |
| Frequency response (USB, MIC) | 20 Hz - 20 kHz; S/N 103 dB (USB); THD 0.005 % | VERIFIED (MAN p.159) |
| Rated MASTER out | 2.1 Vrms / 10 kohm, RCA unbalanced | VERIFIED (MAN p.159) |
| Headphones | 3.5 mm stereo, needs 32 ohm or more | VERIFIED (MAN p.36) |
| Android MONO/STEREO switch | Rear slide switch (default STEREO). Also reports over MIDI, see 4.9 | VERIFIED (MAN p.34, PDF p2 fig 3-14) |
| Switching DJ software | AlphaTheta says: do not run two DJ apps at once; when switching apps, power-cycle the unit after the switch | VERIFIED (AlphaTheta support article "DDJ-FLX4 - MIDI-compatible software") |
| Bluetooth MIDI | The unit also has BLE MIDI for iOS/Android; the "Bluetooth MIDI indicator" turns off when a USB DJ app is recognised | VERIFIED (MAN p.25-26) |
| Firmware | Mixxx mapping was made on firmware 1.02. Hardware ID suffix `REV_0100` corresponds to FW 1.00 | INFERRED |

Hardware-analog vs MIDI controls: see section 8.

---

## 2. MIDI channel assignment

| Channel (dec) | Status (note / CC) | Used for |
|---|---|---|
| 1 | `0x90` / `0xB0` | Deck 1: transport buttons, loop buttons, jog, jog touch, channel 1 EQ/trim/fader/tempo, channel 1 headphone CUE, pad-mode buttons (deck 1) |
| 2 | `0x91` / `0xB1` | Same set for deck 2 |
| 5 | `0x94` / `0xB4` | Beat FX section (select, beat left/right, on/off, FX channel switch part 1, level/depth CC) |
| 6 | `0x95` / `0xB5` | Beat FX channel-2 variants (FX on/off when CH2 selected, FX channel switch part 2). Level/depth is listed as "6" in the PDF but status `0xB4`; see discrepancies |
| 7 | `0x96` / `0xB6` | Global mixer and browse: master level, mic level, headphone mix/level, crossfader, CFX knobs, master CUE, smart CFX, smart fader, browse rotary and push, LOAD buttons, Android switch |
| 8 | `0x97` | Deck 1 performance pads (unshifted) |
| 9 | `0x98` | Deck 1 performance pads (SHIFT held) |
| 10 | `0x99` | Deck 2 performance pads (unshifted) |
| 11 | `0x9A` | Deck 2 performance pads (SHIFT held) |
| 16 | `0x9F` | Output only: "track loaded" illumination |

All rows VERIFIED (PDF p1-p5). There is no separate channel per pad mode: the pad mode is encoded in the note number (section 4.8). The host can learn the active mode from the mode-button message and from the note numbers of pad presses.

---

## 3. How to read the input tables

- "Deck 1 / Deck 2" columns give the status byte for deck 1 and deck 2.
- Pairs listed as `0x90 / 0x91` mean deck 1 / deck 2.
- Value column is data2.
- PDF page: 1 = deck section (PLAY, CUE, SHIFT, jog, IN, OUT, 4 BEAT, CUE/LOOP CALL); 2 = BEAT SYNC, TEMPO, FX, mixer, browse; 3-5 = pads, illumination, settings, footnotes.

---

## 4. INPUT messages (controller to host)

### 4.1 Deck buttons (notes, channel 1 / 2)

| Control | Status D1 / D2 | Note (unshifted) | Note (+SHIFT) | Value | Notes | Source |
|---|---|---|---|---|---|---|
| PLAY/PAUSE | `0x90` / `0x91` | `0x0B` | `0x0E` | `7F`/`00` | Mixxx uses shifted as "reverse roll" | VERIFIED PDF p1 |
| CUE | `0x90` / `0x91` | `0x0C` | `0x48` | `7F`/`00` | MAN p.19: shift+CUE = return to track start | VERIFIED PDF p1 |
| SHIFT | `0x90` / `0x91` | `0x3F` | none | `7F`/`00` | No LED | VERIFIED PDF p1 |
| BEAT SYNC (short press) | `0x90` / `0x91` | `0x58` | `0x60` | `7F`/`00` | **PDF footnote *3: only this button sends when the finger is RELEASED, not when pressed.** Whether a `0x00` follows is UNKNOWN | VERIFIED PDF p2 |
| BEAT SYNC (long press) | `0x90` / `0x91` | `0x5C` | none listed | `7F`/`00` | MAN p.17: long press (about 1 s) sets deck as master | VERIFIED PDF p2 |
| IN | `0x90` / `0x91` | `0x10` | `0x4C` | `7F`/`00` | | VERIFIED PDF p1 |
| OUT | `0x90` / `0x91` | `0x11` | `0x4E` | `7F`/`00` | | VERIFIED PDF p1 |
| 4 BEAT/EXIT | `0x90` / `0x91` | `0x4D` | `0x50` | `7F`/`00` | One physical button. The task's "RELOOP/EXIT" and "4 BEAT/EXIT" are the same note; the PDF only has "4 BEAT / EXIT". Mixxx calls `0x4D` reloop and `0x50` reloop-and-stop | VERIFIED PDF p1 |
| CUE/LOOP CALL left | `0x90` / `0x91` | `0x51` | `0x3E` | `7F`/`00` | MAN p.16: halves loop while looping | VERIFIED PDF p1 |
| CUE/LOOP CALL right | `0x90` / `0x91` | `0x53` | `0x3D` | `7F`/`00` | MAN p.16: doubles loop while looping | VERIFIED PDF p1 |
| Headphone CUE (channel) | `0x90` / `0x91` | `0x54` | `0x68` | `7F`/`00` | Shift = tap-tempo per MAN p.27 | VERIFIED PDF p2 |
| Jog platter touch | `0x90` / `0x91` | `0x36` | `0x67` | `7F` touch / `00` release | No LED | VERIFIED PDF p1 |

Beat jump is done through the BEAT JUMP pad mode (4.8), not a dedicated deck button.

### 4.2 Jog wheel (relative, CC on channel 1 / 2)

Relative encoding for every jog CC (VERIFIED PDF p1: "Difference count value from previous operation. Turn clockwise: increases from 0x41. Turn counterclockwise: decreases from 0x3F"):

- `0x40` = zero (never sent because there is no movement).
- Clockwise: `0x41`, `0x42`, ... (delta = `value - 0x40`, positive).
- Counterclockwise: `0x3F`, `0x3E`, ... (delta = `value - 0x40`, negative).
- Magnitude = number of ticks since the previous message, so speed shows up as larger deltas, not just more messages.
- Maximum usable magnitude is 63 either way (7-bit). Observed ranges: UNKNOWN.

| Control | Status D1 / D2 | CC | Condition | Notes |
|---|---|---|---|---|
| Platter (top), vinyl mode ON | `0xB0` / `0xB1` | `0x22` | Vinyl mode ON (default) | Normally used with touch (note `0x36`) for scratching |
| Platter (top), vinyl mode OFF | `0xB0` / `0xB1` | `0x23` | Vinyl mode OFF | Use as pitch bend |
| Platter (top) + SHIFT | `0xB0` / `0xB1` | `0x29` | SHIFT held | Fast seek. Single CC regardless of vinyl mode |
| Wheel side (outer ring) | `0xB0` / `0xB1` | `0x21` | none | Pitch bend. Same CC `0x21` with SHIFT held (no separate shifted CC) |

All VERIFIED PDF p1. Vinyl mode is set by the HOST (section 5.4); the unit cannot change it (PDF footnote *2). MAN p.19: with vinyl mode on, turning the top scratches; outer section or vinyl-off top bends pitch.

### 4.3 14-bit absolute controls (CC pairs)

MSB and LSB are separate CC messages on the same status byte. Min = `0x0000`, max = `0x3FFF` (PDF Details: "MSB 0x00 LSB 0x00" to "MSB 0x7F LSB 0x7F"). Ordering of the MSB/LSB messages within a movement is UNKNOWN (see section 10). Combine whichever arrives by keeping both latest bytes.

| Control | Status D1 / D2 | MSB CC | LSB CC | Value meaning | Source |
|---|---|---|---|---|---|
| TEMPO slider | `0xB0` / `0xB1` | `0x00` | `0x20` | `0x0000` = "-" side, `0x3FFF` = "+" side. Mechanical centre expected at `0x2000` (INFERRED). Shifted variant sends the same CCs | VERIFIED PDF p2 |
| CHANNEL FADER | `0xB0` / `0xB1` | `0x13` | `0x33` | `0` = bottom, `0x3FFF` = top | VERIFIED PDF p2 |
| TRIM | `0xB0` / `0xB1` | `0x04` | `0x24` | fully CCW = 0 | VERIFIED PDF p2 |
| EQ HI | `0xB0` / `0xB1` | `0x07` | `0x27` | fully CCW = 0 | VERIFIED PDF p2 |
| EQ MID | `0xB0` / `0xB1` | `0x0B` | `0x2B` | fully CCW = 0 | VERIFIED PDF p2 |
| EQ LOW | `0xB0` / `0xB1` | `0x0F` | `0x2F` | fully CCW = 0 | VERIFIED PDF p2 |
| CFX knob, channel 1 | `0xB6` | `0x17` | `0x37` | fully CCW = 0 | VERIFIED PDF p2 |
| CFX knob, channel 2 | `0xB6` | `0x18` | `0x38` | fully CCW = 0 | VERIFIED PDF p2 |
| CROSSFADER | `0xB6` | `0x1F` | `0x3F` | `0` = left, `0x3FFF` = right | VERIFIED PDF p2 |
| MASTER LEVEL | `0xB6` | `0x08` | `0x28` | sends MIDI (see section 8) | VERIFIED PDF p2 |
| MIC LEVEL | `0xB6` | `0x05` | `0x25` | sends MIDI (see section 8) | VERIFIED PDF p2 |
| HEADPHONES MIX | `0xB6` | `0x0C` | `0x2C` | sends MIDI (see section 8) | VERIFIED PDF p2 |
| HEADPHONES LEVEL | `0xB6` | `0x0D` | `0x2D` | sends MIDI (see section 8) | VERIFIED PDF p2 |
| BEAT FX LEVEL/DEPTH | `0xB4` | `0x02` | `0x22` | fully CCW = 0. See discrepancy D1 (PDF channel column says 6) | VERIFIED PDF p2, status per MIXXX |

The EQ knob centre is detent-based: electrical centre should read near `0x2000`; exact detent value UNKNOWN.

### 4.4 Rotary encoders (relative)

| Control | Status | CC | Encoding | Source |
|---|---|---|---|---|
| BROWSE rotary | `0xB6` | `0x40` | Clockwise: `0x01`, `0x02`, ... Counterclockwise: `0x7F`, `0x7E`, ... (signed 7-bit two's complement: delta = value if value < 0x40 else value - 0x80) | VERIFIED PDF p2 |
| BROWSE rotary + SHIFT | `0xB6` | `0x64` | Same encoding | VERIFIED PDF p2 |

Note the browse encoding is different from the jog encoding (browse is centred on 0, jog on `0x40`).

### 4.5 Mixer and global buttons (channel 7, status `0x96`)

| Control | Note | +SHIFT note | Notes | Source |
|---|---|---|---|---|
| MASTER CUE (headphones, master) | `0x63` | `0x78` | Mixxx manual: master-to-phones routing is hardware | VERIFIED PDF p2 |
| SMART CFX | `0x00` | `0x08` | MAN p.27: toggle Smart CFX / shift = change preset | VERIFIED PDF p2 |
| SMART FADER | `0x01` | `0x09` | | VERIFIED PDF p2 |
| BROWSE push | `0x41` | `0x42` | | VERIFIED PDF p2 |
| LOAD deck 1 | `0x46` | `0x68` | | VERIFIED PDF p2 |
| LOAD deck 2 | `0x47` | `0x7A` | | VERIFIED PDF p2 |

### 4.6 Fader start (note messages under SHIFT)

MAN p.27, p.67: SHIFT + moving a channel fader or the crossfader triggers fader start. These are the notes the unit sends (data2 `0x7F`/`0x00`):

| Trigger | Status | Note | Meaning | Source |
|---|---|---|---|---|
| Deck 1 channel fader, SHIFT, moved from the bottom to any other position | `0x90` | `0x66` | PLAY | VERIFIED PDF p2 |
| Deck 1 channel fader, SHIFT, moved from other position to the bottom | `0x90` | `0x52` | CUE (back cue) | VERIFIED PDF p2 |
| Deck 2 channel fader, same two cases | `0x91` | `0x66` / `0x52` | PLAY / CUE | VERIFIED PDF p2 |
| Crossfader, SHIFT, moved from right edge towards left | `0x90` | `0x66` | start deck 1 | VERIFIED PDF p2 |
| Crossfader, SHIFT, moved from other position to the right edge | `0x90` | `0x52` | cue deck 1 | VERIFIED PDF p2 |
| Crossfader, SHIFT, moved from left edge towards right | `0x91` | `0x66` | start deck 2 | VERIFIED PDF p2 |
| Crossfader, SHIFT, moved from other position to the left edge | `0x91` | `0x52` | cue deck 2 | VERIFIED PDF p2 |

Whether the normal fader/crossfader CC pair is still sent while SHIFT is held is UNKNOWN (PDF lists only the notes in the shifted rows). Verify with a MIDI monitor.

### 4.7 Beat FX section

| Control | Status | Data1 | Value | Source |
|---|---|---|---|---|
| BEAT FX SELECT | `0x94` | `0x63` | `7F`/`00` | VERIFIED PDF p2 |
| BEAT FX SELECT + SHIFT | `0x94` | `0x64` | `7F`/`00` | VERIFIED PDF p2 |
| BEAT left | `0x94` | `0x4A` | `7F`/`00` | VERIFIED PDF p2 |
| BEAT left + SHIFT | `0x94` | `0x66` | `7F`/`00` | VERIFIED PDF p2 |
| BEAT right | `0x94` | `0x4B` | `7F`/`00` | VERIFIED PDF p2 |
| BEAT right + SHIFT | `0x94` | `0x6B` | `7F`/`00` | VERIFIED PDF p2 |
| LEVEL/DEPTH | `0xB4` | CC `0x02` / `0x22` | 14-bit | VERIFIED PDF p2 |
| FX ON/OFF, CH1 selected | `0x94` | `0x47` | `7F`/`00` | VERIFIED PDF p2 |
| FX ON/OFF, CH2 selected | `0x95` | `0x47` | `7F`/`00` | VERIFIED PDF p2 |
| FX ON/OFF, CH1&2 selected | `0x94` | `0x47` | `7F`/`00` | VERIFIED PDF p2 |
| FX ON/OFF + SHIFT, CH1 / CH2 / CH1&2 | `0x94` / `0x95` / `0x94` | `0x43` | `7F`/`00` | VERIFIED PDF p2 |

BEAT FX CH SELECT (3-position slide switch) sends four Note messages on every change; the switch position is encoded by which two read `0x7F`:

| Switch position | `0x94 0x10` | `0x94 0x11` | `0x95 0x10` | `0x95 0x11` |
|---|---|---|---|---|
| CH1 | `7F` | `00` | `00` | `00` |
| CH2 | `00` | `00` | `00` | `7F` |
| CH1&CH2 | `7F` | `00` | `00` | `7F` |

So `0x94 0x10` = FX applied to channel 1, `0x95 0x11` = FX applied to channel 2; the other two notes are always `0x00` (VERIFIED PDF p2). MIXXX binds exactly those two notes.

### 4.8 Pad-mode buttons and performance pads

Pad mode buttons (channel 1 / 2, value `7F`/`00`; the shifted variants are what the hardware calls the secondary modes; MAN p.17-18):

| Physical button | Mode (unshifted) note | Mode with SHIFT (note) | Status D1 / D2 | Source |
|---|---|---|---|---|
| HOT CUE | `0x1B` = HOT CUE mode | `0x69` = KEYBOARD mode | `0x90` / `0x91` | VERIFIED PDF p3 (5-1), MAN p.17 |
| PAD FX 1 | `0x1E` = PAD FX 1 mode | `0x6B` = PAD FX 2 mode | `0x90` / `0x91` | VERIFIED PDF p3 (5-2), MAN p.18 |
| BEAT JUMP | `0x20` = BEAT JUMP mode | `0x6D` = BEAT LOOP mode | `0x90` / `0x91` | VERIFIED PDF p3 (5-3), MAN p.18 |
| SAMPLER | `0x22` = SAMPLER mode | `0x6F` = KEY SHIFT mode | `0x90` / `0x91` | VERIFIED PDF p3 (5-4), MAN p.18 |

Performance pads: 8 pads per deck. Note number = mode base + (pad number - 1), so pad n (1..8) = base + 0..7. Physical layout (MAN p.56): pads 1-4 top row, 5-8 bottom row.

| Mode | Base note | Pad 1 | Pad 8 | Default function (rekordbox, MAN p.56-65) |
|---|---|---|---|---|
| HOT CUE | `0x00` | `0x00` | `0x07` | Hot cue A-H; SHIFT+pad clears |
| PAD FX 1 | `0x10` | `0x10` | `0x17` | Effects A-H (SHIFT variant: I-P) |
| BEAT JUMP | `0x20` | `0x20` | `0x27` | 1,2,4,8 beats, odd pads backward, even pads forward; SHIFT+pad 7 / 8 = smaller / larger beat set |
| SAMPLER | `0x30` | `0x30` | `0x37` | Sampler slots 1-8 (deck 2: slots 9-16); SHIFT+pad = stop |
| KEYBOARD | `0x40` | `0x40` | `0x47` | Hot cue played at semitone offsets |
| PAD FX 2 | `0x50` | `0x50` | `0x57` | Effects Q-X (SHIFT variant: Y-AF) |
| BEAT LOOP | `0x60` | `0x60` | `0x67` | 1/4, 1/2, 1, 2, 4, 8, 16, 32 beats |
| KEY SHIFT | `0x70` | `0x70` | `0x77` | Key shift +4 +5 +6 +7 / 0 +1 +2 +3 semitones |

Status per deck and shift layer (VERIFIED PDF p3-5):

| | Unshifted | SHIFT held |
|---|---|---|
| Deck 1 pads | `0x97` | `0x98` |
| Deck 2 pads | `0x99` | `0x9A` |

Note numbers do not change when SHIFT is held; the channel changes (+1). Values `7F`/`00`. Every pad row has a matching MIDI-OUT row (LED), see section 5.

Notes collide numerically with deck-button notes (for example `0x20` is BEAT JUMP mode button on `0x90` and BEAT JUMP pad 1 on `0x97`). Always dispatch on the pair (status, data1).

### 4.9 Miscellaneous

| Control | Status | Data1 | Value | Source |
|---|---|---|---|---|
| Android MONO/STEREO rear switch | `0x96` | `0x6D` | `0x00` = STEREO (default), `0x7F` = MONO | VERIFIED PDF p2 (3-14). Sent on switch movement; whether also sent on connect is UNKNOWN |

No SysEx input is documented in the PDF.

---

## 5. OUTPUT messages (host to controller, LEDs and settings)

LED rule from PDF: output rows reuse the same status and data1 as the matching input row; value `hh` where `0x7F` = ON and `0x00` = OFF (PDF Details column). Nothing in the PDF lists colour values, dim levels or brightness steps. Pad LED colour is fixed per hardware mode.

Important: the PDF lists output rows for both the normal and the SHIFT variant of a button (and for both the normal and shifted pad channel). That means the unit shows the LED state keyed by the note number actually in effect (the shifted note when SHIFT is held, the shifted pad channel when SHIFT is held). To keep an LED correct whether or not SHIFT is held, drive both addresses with the same state. MIXXX does this too (sends both the base and shifted note, and both `0x97` and `0x98`, both `0x99` and `0x9A`).

### 5.1 Button LEDs (channel 1 / 2)

| LED | Status D1 / D2 | Note | Shifted-note LED | PDF listed? |
|---|---|---|---|---|
| PLAY/PAUSE | `0x90` / `0x91` | `0x0B` | `0x0E` | Yes, both (PDF p1) |
| CUE | `0x90` / `0x91` | `0x0C` | `0x48` | Yes, both |
| BEAT SYNC | `0x90` / `0x91` | `0x58` | `0x60` | Yes, both (not for long-press `0x5C`) |
| IN | `0x90` / `0x91` | `0x10` | `0x4C` | Yes, both |
| OUT | `0x90` / `0x91` | `0x11` | `0x4E` | Yes, both |
| 4 BEAT/EXIT | `0x90` / `0x91` | `0x4D` | `0x50` | **No** in PDF; MIXXX drives both (see D2) |
| CUE/LOOP CALL left/right | - | - | - | No LED in PDF |
| Headphone CUE (channel) | `0x90` / `0x91` | `0x54` | `0x68` | Yes, both |
| Pad mode buttons | `0x90` / `0x91` | `0x1B`, `0x1E`, `0x20`, `0x22` | `0x69`, `0x6B`, `0x6D`, `0x6F` | Yes, all eight (PDF p3) |
| SMART CFX | `0x96` | `0x00` | none | Yes (shift has no output) |
| SMART FADER | `0x96` | `0x01` | none | Yes (shift has no output) |
| MASTER CUE | - | - | - | No LED in PDF |
| Beat FX ON/OFF | `0x94` | `0x47` | `0x43` | Not in the table, see footnote *4 below and D3 |

PDF footnote *4: "The [FX ON/OFF] button blinks when NOTE ON is received, and the button lights up when NOTE OFF is received." So for FX ON/OFF, `0x7F` = blinking, `0x00` = steady lit, i.e. the LED is never fully dark. This is an unusual polarity; MIXXX sends `0x7F` when the focused effect is enabled. Verify on hardware.

### 5.2 Pad LEDs

Same status and note as the pad input row (4.8), value `7F` on / `00` off. Eight pads x eight modes x four statuses (`0x97`, `0x98`, `0x99`, `0x9A`). The hardware displays the LED row belonging to the active mode, so the host can write all modes' state ahead of time (INFERRED from MIXXX writing mode rows while another mode is active). A third level (`0x20` dim, "available but not selected") is reported by LSDJ as hardware-confirmed but not in the PDF: INFERRED.

### 5.3 Channel level meter (VU)

| Meter | Status | CC | Value |
|---|---|---|---|
| Deck 1 | `0xB0` | `0x02` | thresholds below |
| Deck 2 | `0xB1` | `0x02` | thresholds below |

PDF p2 (fig 3-15) lights segments from the bottom:

| Segment | Data2 range |
|---|---|
| Green 1 | `0x26` - `0x40` |
| Green 2 | `0x41` - `0x56` |
| Orange 1 | `0x57` - `0x64` |
| Orange 2 | `0x65` - `0x76` |
| Red | `0x77` - `0x7F` |

Below `0x26` presumably lights nothing: INFERRED. The PDF mislabels this row as "NOTE 2 D-1" in the data1 column, but the status column is `0xB0`/`0xB1`, so it is a CC. The meter shows the level before the channel fader (MAN p.26).

### 5.4 Settings / illumination (PDF p5)

| Name | Status | Data1 | Data2 | Meaning |
|---|---|---|---|---|
| LOADED, deck 1 | `0x9F` | `0x00` | `0x7F` | Track-load illumination (deck 1) |
| LOADED, deck 2 | `0x9F` | `0x01` | `0x7F` | Track-load illumination (deck 2) |
| Vinyl mode, deck 1 | `0x90` | `0x17` | `0x7F` ON (default) / `0x00` OFF | Selects which jog CC the platter emits (`0x22` vs `0x23`) |
| Vinyl mode, deck 2 | `0x91` | `0x17` | `0x7F` ON (default) / `0x00` OFF | Same |

VERIFIED PDF p5, footnote *2: vinyl mode cannot be changed from the unit; the host must send it. Effect of LOADED with `0x00` (clearing the animation/state) is UNKNOWN; MIXXX sends `0x7F` on load and `0x00` on eject. Exactly what the "illumination" looks like is UNKNOWN (likely a jog-ring animation).

---

## 6. SysEx, initialization, keepalive, refresh

Official position (PDF, MAN): **no SysEx and no handshake is documented.** The PDF has no SysEx section. MAN p.161 says only that the unit outputs button/knob data as MIDI.

Unofficial (INFERRED, from MIXXX and one LSDJ project):

| Item | Bytes | Behaviour | Tag |
|---|---|---|---|
| Keepalive / position query | `F0 00 40 05 00 00 04 05 00 50 02 F7` (12 bytes) | Host to unit. MIXXX sends it once at start ("query the controller for current control positions on startup... doubles as keep alive") and then repeats it every 200 ms from a timer. Its authors note it was reverse engineered with Wireshark (USB capture of rekordbox) | INFERRED. `00 40 05` is the manufacturer ID used by Pioneer DJ products. The remaining bytes `00 00 04 05 00 50 02` have no known decoded meaning |
| Effect on faders | After the query the unit reportedly sends the current position of knobs and faders (otherwise controls are silent until moved) | | INFERRED (LSDJ). No MIDI-spec documentation. The reply is plain CC messages, not SysEx. The MIXXX script has no SysEx input handler |
| Is the keepalive required? | Unknown whether the unit stops sending/lighting without it | | UNKNOWN |
| LED refresh | The PDF does not mention LED timeouts. MIXXX never re-sends LED state periodically (only the 200 ms SysEx timer exists), so LEDs are probably latched | | INFERRED |
| LED enable / mode SysEx | None found | | None known |
| Startup LED state | Pad mode defaults to HOT CUE at power-on per LSDJ | | INFERRED |

### What MIXXX sends on init (described in words, bytes only)

1. A Note-On with velocity `0x7F` on `0x9F` for data1 `0x00`, then for `0x01` (plays the "track loaded" illumination on both decks at startup).
2. For each deck (status `0x90`, then `0x91`): Note-On `0x7F` for notes `0x10`, `0x11`, `0x4E`, `0x4C` (turns on the IN and OUT LEDs and their shifted twins).
3. Level-meter reset: CC `0x02` value `0x00` on status `0xB0`, issued for both decks' meter entries, which both use status `0xB0` (see D6).
4. The 12-byte SysEx above once, immediately, then a 200 ms repeating timer.
5. Subsequent state callbacks light LEDs as needed (loaded track, loop state, pad modes, FX, sampler).
6. Pad-mode LEDs are lit when the user presses the mode button: the host echoes the received note back with `0x7F` on the same status and data1 (`0x90 0x1B 0x7F` etc.). This implies the host owns the mode-button LED. Mixxx does not turn the previous mode LED off, so either the hardware makes the lamps exclusive or Mixxx relies on firmware behaviour. UNKNOWN; LSDJ says the host must manage it. Test on hardware.

### What MIXXX sends on shutdown

1. Sampler pad LEDs off: value `0x00` on notes `0x30`-`0x37` for statuses `0x97`, `0x98`, `0x99`, `0x9A`.
2. Hot cue pad LEDs off: value `0x00` on notes `0x00`-`0x07` for the same four statuses.
3. IN/OUT LEDs off: `0x00` on notes `0x10`, `0x11`, `0x4E`, `0x4C` for `0x90` and `0x91`.
4. 4 BEAT/EXIT LEDs off: `0x00` on notes `0x4D` and `0x50` for `0x90` and `0x91`.
5. Beat FX LEDs off: `0x00` on `0x94 0x47` and `0x94 0x43`.
6. Level meter off on `0xB0 0x02`.
7. Stop the keepalive timer. No SysEx and no vinyl-mode reset on shutdown.

For deckforge: do our own independent equivalent; on clean exit send `0x00` to all LEDs we have lit. (Pad modes other than hot cue and sampler are not cleared by Mixxx; a robust implementation should clear all eight mode rows.)

---

## 7. Jog wheel notes

- Two physical surfaces: the platter top (touch-sensitive) and the outer wheel side (ring). They emit different CCs (platter `0x22`/`0x23`, ring `0x21`) so the host can bend with the ring and scratch with the top without extra state. VERIFIED PDF p1.
- Touch: Note On `0x36` (SHIFT: `0x67`) with `7F` when touched and `00` when released, per deck. The host should treat touch+platter-CC as scratch; platter CC without touch as nudge/bend. VERIFIED PDF p1.
- Platter CC number depends on vinyl mode (host-controlled, section 5.4): `0x22` when ON (default at power-up), `0x23` when OFF. SHIFT: `0x29`. VERIFIED PDF p1.
- Ticks per revolution: **UNKNOWN officially.** MIXXX uses 720 intervals per revolution with 33 1/3 rpm as its jog constant (reused from the DDJ-400 script), which suggests 720, but that is a Mixxx constant, not a datasheet value. To verify: monitor CC `0x22` while rotating the platter slowly through exactly one revolution and sum `(value - 0x40)`.
- Concept for scratch math (not code): when touched, convert accumulated ticks to a playback-position delta using ticks / ticks-per-rev x (60 / 33.33 rpm) seconds, i.e. one full turn = 1.8 s of audio at 33 1/3 rpm; feed that into the deck as a velocity (target rate = delta_audio_time / delta_wall_time) with smoothing so rapid reversals do not click. A fixed ramp on touch-down and release avoids pops. Without touch, add a small signed rate offset proportional to ticks per unit time (pitch bend), decaying back to zero.
- Message rate depends on rotation speed; high delta values mean fast spins, not 63 separate ticks, so never assume "one message = one tick".
- Shift+platter (`0x29`) is intended for fast seek; scale delta by a large factor.
- The ring (`0x21`) and the shifted ring (same `0x21`) are not distinguished by CC; use the SHIFT note state if shifted behaviour is wanted.

---

## 8. Hardware-analog vs MIDI controls

| Control | Sends MIDI? | Also hardware-analog? | Recommendation |
|---|---|---|---|
| MASTER LEVEL | **Yes**, `0xB6` CC `0x08`/`0x28` (PDF p2 fig 3-1) | UNKNOWN. MAN p.25 says it "adjusts the volume of the master output sound" without saying how. Mixxx manual says "hardware implemented and is not mapped to the software gain" | Do not apply to our software master gain by default (double attenuation risk). Surface as a read-only value. Verify (below) |
| HEADPHONES LEVEL | **Yes**, `0xB6` CC `0x0D`/`0x2D` | UNKNOWN, likely hardware (Mixxx manual: "adjusts output level from PHONES jack") | Same as above |
| HEADPHONES MIX | **Yes**, `0xB6` CC `0x0C`/`0x2C` | UNKNOWN. MIXXX maps it to its own software head-mix control, so it is treated as software-mixed | Our headphone bus is ours to build; map to our cue/master mix. Verify |
| MASTER CUE (master to phones) | **Yes**, `0x96` note `0x63` | Mixxx manual says "hardware implemented and not visible in the software" | Treat as a toggle we may mirror; verify if audio changes without software |
| MIC LEVEL / MIC ATT | `0xB6` CC `0x05`/`0x25` sent; ATT is purely analog (rear knob, MAN p.35) | Mic is mixed in hardware and not routed back over USB (Mixxx manual) | MIC audio is not available to the host; ignore MIDI except for UI |
| TRIM, EQ, CFX, channel faders, crossfader | Yes, 14-bit | No (digital in software) | Implement fully in software |
| TEMPO | Yes, 14-bit | No | Software |

Verification method for "does the knob act in hardware too": hold a steady 1 kHz tone in the deck, monitor MIDI, sweep the knob while measuring the analog output (or listening); if the level changes while software gain is untouched, the knob is also hardware-analog.

---

## 9. Behaviours around fader start, controller mode, initial positions

- **Fader start**: implemented as shifted notes `0x66` (play) / `0x52` (cue); see 4.6. MAN p.67 describes the user-level behaviour (cue point playback, back-cue on return). VERIFIED.
- **Controller mode**: the PDF defines no controller-mode or "enable MIDI" switch. The unit sends MIDI whenever it is powered and connected over USB; the Bluetooth indicator goes off once a USB DJ app is recognised (MAN p.25). VERIFIED, absence of anything more.
- **Request initial positions**: no official mechanism. The Mixxx SysEx (6) is the only known trigger; see its INFERRED status. Without it, treat knobs and faders as unknown until the first movement; do not assume positions on connect. The MIXXX script also uses soft-takeover for tempo and FX knobs, which implies the host cannot always know the physical positions.
- **Power-cycle after switching DJ software**: AlphaTheta guidance (section 1). VERIFIED.

---

## 10. Discrepancies between PDF and Mixxx (and PDF internal)

| # | Item | PDF | MIXXX | Resolution |
|---|---|---|---|---|
| D1 | BEAT FX LEVEL/DEPTH channel | Channel column says `6`, status column `B4`, and MIXXX uses `0xB4` | `0xB4 0x02` (MSB) | Use `0xB4`. Also listen on `0xB5 0x02/0x22` as a fallback; verify on hardware |
| D2 | 4 BEAT/EXIT LED | MIDI-OUT shown as "-" for notes `0x4D`/`0x50` | Sends `0x7F`/`0x00` to `0x4D` and `0x50` on `0x90`/`0x91` | Trust MIXXX (works on real hardware) but verify; may be an undocumented LED |
| D3 | FX ON/OFF LED | OUT "-" in table, but footnote *4 says NOTE ON blinks and NOTE OFF lights | Writes `0x94 0x47` and `0x94 0x43` with `0x7F` when effect enabled | Follow footnote *4 plus MIXXX; also try `0x95 0x47`/`0x95 0x43`. Verify which notes drive the LED |
| D4 | SHIFT+PLAY LED note | PDF: input and output `0x0E` | MIXXX input `0x0E` (matches), but its internal LED table has an unused entry `0x47` for shift-play | Use `0x0E`; the Mixxx `0x47` entry is unused and wrong per PDF |
| D5 | Level meter data1 label | "NOTE 2 D-1" but status `B0`/`B1` | `0xB0/0xB1 0x02` as CC | It is a CC (typo in PDF label) |
| D6 | Deck 2 meter reset | `0xB1 0x02` for deck 2 | Init/shutdown clear uses `0xB0 0x02` for both lights tables (deck 2 clear is aimed at the wrong status); runtime meter updates correctly use `0xB1` | Use `0xB1` for deck 2 |
| D7 | Tempo +SHIFT | Same CC `0x00`/`0x20` as unshifted | Binds only unshifted | Same CC; no shift handling needed |
| D8 | "Reloop/exit" naming | Only "4 BEAT / EXIT" | Calls `0x4D` reloop | Same byte, different label |
| D9 | Pad modes 5-8 | Present (KEYBOARD, PAD FX 2, BEAT LOOP, KEY SHIFT) with LED rows | Uses KEYBOARD for stems, KEY SHIFT for pitch; PAD FX not implemented | PDF is authoritative |
| D10 | Crossfader/channel fader values | Min "bottom/left", max "top/right" | Same | No conflict |

Tempo orientation note: PDF defines `0` as the "-" end and `0x3FFF` as the "+" end. MIXXX inverts the value because Mixxx's rate convention is configurable. LSDJ reports the low end is the slow end on hardware. Which physical end (top or bottom of the slider) corresponds to "-" is not stated in the PDF; verify.

---

## 11. UNKNOWN items and how to verify on hardware

| Item | Verify by |
|---|---|
| Exact CoreMIDI port/entity name | Audio MIDI Setup > MIDI Studio, or a CoreMIDI enumeration; record both source and destination names |
| USB VID/PID | `system_profiler SPUSBDataType` |
| Audio channel count/order, 0 inputs | `system_profiler SPAudioDataType`; play per-channel test tones |
| Whether MSB or LSB is sent first | MIDI monitor while moving a fader slowly and quickly |
| Whether `0x00` is sent after BEAT SYNC `0x58` and `0x5C` | MIDI monitor; quick tap vs long hold |
| Whether a long press also emits `0x58` | MIDI monitor |
| Whether shifted fader moves still emit the CC pair | MIDI monitor with SHIFT held |
| Jog ticks per revolution, max delta values | Sum `(v - 0x40)` for one full revolution at different speeds |
| Whether the SysEx query is required, what it triggers, whether it must repeat | Connect, observe CC burst; disable repetition and watch LEDs and controls for several minutes |
| Mode-button LED ownership and exclusivity | Echo `0x90 0x1B 0x7F` then `0x90 0x1E 0x7F`; see if the first goes off |
| Pad LED dim/colour values beyond on/off | Send `0x01`..`0x7F` to a pad LED |
| MASTER LEVEL, HEADPHONES LEVEL analog behaviour | Section 8 test |
| LOADED illumination off value and look | Send `0x9F 0x00 0x00` and `0x7F` |
| FX ON/OFF LED notes and blinking polarity | Send `0x94 0x47`, `0x94 0x43`, `0x95 0x47` with `0x7F` and `0x00` |
| Power-on default pad mode | Power-cycle and press pads |

---

## 12. Ten byte-level facts to build the mapping on

1. PLAY `0x90/0x91 0x0B`, CUE `0x90/0x91 0x0C`, SHIFT `0x90/0x91 0x3F` (`7F` press, `00` release); shifted PLAY `0x0E`, CUE `0x48`.
2. Platter rotate: `0xB0/0xB1` CC `0x22` (vinyl ON, default) or `0x23` (vinyl OFF); delta = `value - 0x40` (`0x41` = +1, `0x3F` = -1). Platter touch: Note `0x36`. Wheel side: CC `0x21`. SHIFT+platter: CC `0x29`.
3. TEMPO: `0xB0/0xB1`, MSB CC `0x00`, LSB CC `0x20`, 14-bit.
4. CHANNEL FADER: `0xB0/0xB1`, MSB `0x13`, LSB `0x33`; CROSSFADER: `0xB6`, MSB `0x1F`, LSB `0x3F`.
5. EQ HI/MID/LOW: MSB `0x07`/`0x0B`/`0x0F`, LSB +`0x20`; TRIM `0x04`/`0x24`; CFX `0xB6` `0x17`/`0x37` (ch1), `0x18`/`0x38` (ch2).
6. Pads: deck 1 `0x97` (SHIFT `0x98`), deck 2 `0x99` (SHIFT `0x9A`); note = mode base (`0x00` hot cue, `0x10` pad FX 1, `0x20` beat jump, `0x30` sampler, `0x40` keyboard, `0x50` pad FX 2, `0x60` beat loop, `0x70` key shift) + pad index (0-7).
7. Pad mode buttons: `0x90/0x91` notes `0x1B`, `0x1E`, `0x20`, `0x22`; with SHIFT `0x69`, `0x6B`, `0x6D`, `0x6F` (keyboard, pad FX 2, beat loop, key shift).
8. LEDs: echo the input note back with `0x7F`/`0x00` on the same status; drive both the normal and shifted address; meters on `0xB0/0xB1` CC `0x02`; vinyl mode set with `0x90/0x91 0x17`.
9. BROWSE: `0xB6` CC `0x40`, signed 7-bit (`0x01` = +1 CW, `0x7F` = -1 CCW); push `0x96 0x41`; LOAD deck 1 `0x96 0x46`, deck 2 `0x96 0x47`.
10. There is no official handshake. The only known init is an unofficial 12-byte SysEx (`F0 00 40 05 00 00 04 05 00 50 02 F7`), sent once and repeated every 200 ms by Mixxx; whether it is required is unknown.

---

## 13. Licensing: what we may and may not reuse from Mixxx

Mixxx is licensed under GPL v2 (`LICENSE` in the repository, "General Public License version 2"). The controller XML and JavaScript files are part of that work.

Allowed:

- Using the MIDI numbers as facts. Note numbers, CC numbers and byte values originate from Pioneer's own MIDI list and are factual data about a device, not creative expression. Every value in this document that is VERIFIED came from the PDF/MAN, not from Mixxx.
- Using Mixxx as a behavioural reference: what a button does, which messages the controller needs on startup, observed hardware behaviour. This document describes those in our own words (including the keepalive SysEx bytes, which are a functional interface fact reverse engineered from USB traffic).
- Writing our own independent implementation from this document and the PDF.

Not allowed (unless deckforge is deliberately released under a GPL-2.0-compatible licence; this is a licensing note, not legal advice):

- Copying any Mixxx source: JavaScript functions, XML, comments, structure or naming, even translated line by line to Rust.
- Copying Mixxx's tables or constants verbatim as a block (the pad-to-beat-jump table, the pitch-pad lookup, the `lights` object).
- Linking against or embedding Mixxx code.

Process recommendation: keep this document as the single spec. Mixxx-derived items are tagged INFERRED here so they are confirmed on hardware rather than copied. Do not paste Mixxx files into the repository, and do not keep them in a clean-room contributor's working tree. The AlphaTheta PDF is a copyrighted manual; we cite and summarise it, and should link rather than redistribute it.

General techniques (14-bit CC pairing, soft-takeover, touch-gated scratching) are ideas we may implement independently; only Mixxx's concrete code and expression must not be copied.

## 14. Hardware check, 2026-10-14 (Phase 3 FX control)

Captured with `flx4_monitor` on the user's DDJ-FLX4 (logs `target/flx4-phase3-run{1,3}.log`). All items below are VERIFIED on hardware.

| Question | Observed |
|---|---|
| LEVEL/DEPTH with SHIFT held | Same CC (`B4 02`/`22`) as unshifted; SHIFT is only visible as deck SHIFT `90 3F`. The knob is also mirrored on `B5 02`/`22` every time. |
| BEAT ◀ / ▶ | `94 4A` / `94 4B`; with SHIFT `66` / `6B`, sent on both `94` and `95` |
| FX SELECT | `94 63`; with SHIFT `94 64` |
| FX ON/OFF | `94 47`; with SHIFT `43`, sent on both `94` and `95` (CH SELECT at CH1&2) |
| CFX 1 / 2 | `B6 17`/`37`, `B6 18`/`38` |
| PAD FX 2 | SHIFT + PAD FX 1 sends `90 6B` (no separate button) |
| Pad mode switching | **Host-driven.** The pad-mode button press is reported, but pads keep sending the previous mode's notes until the host lights that mode's button LED. With the LED driven, PAD FX 1 pads send `0x10..0x17`, PAD FX 2 `0x50..0x57`, BEAT JUMP `0x20..`, HOT CUE `0x00..`; deck 2 the same on `99`. |
| Pad and FX ON/OFF LEDs | Dark unless the host drives them (expected; no host in the bare monitor). |
