# DDJ-FLX4 effects control for deckforge

Research and design date: 2026-10-13. Status: proposal, nothing implemented, nothing tested on the device.

Goal: keep the FLX4 as the primary input surface, but give the user finer, more intuitive control of effects and their parameters than "cycle kind, step beats, one LEVEL knob". The FLX4 has no screens and no LED rings, so every proposal below pairs a hardware gesture with on-screen feedback.

Byte-level facts come from `docs/research/ddj-flx4-midi.md` (called "the MIDI doc" below). Only rows tagged VERIFIED there are used. Anything that depends on unverified behaviour is listed in section 7.

---

## 1. Where deckforge is today

From `crates/controller/src/flx4.rs`, `BeatFx.svelte`, `Mixer.svelte` and `api/types.ts`:

| Area | Current behaviour |
|---|---|
| BEAT FX SELECT | `0x94 0x63` next kind, `0x94 0x64` (SHIFT) previous kind. 12 kinds (delay, echo, upEcho, downEcho, pingPong, spiral, reverb, flanger, phaser, roll, trans, filterSweep). |
| BEAT left/right | `0x94 0x4A` / `0x4B` step the beat division (`BEAT_FX_BEATS`: 1/8, 1/4, 1/2, 3/4, 1, 2, 4, 8, 16); for reverb they step `percent` instead. The SHIFT variants (`0x66`, `0x6B`) are not handled. |
| LEVEL/DEPTH | `0xB4 0x02/0x22`, 14-bit, sets `beatFx.level` only. No SHIFT handling, so the host cannot reach a second parameter. In the DSP, echo-family feedback is derived from this one value. |
| FX ON/OFF | `0x94 0x47` (and `0x95 0x47`) toggles on/off. SHIFT variant `0x43` is only used as an LED twin; SHIFT+ON/OFF does nothing. |
| CH SELECT | `0x94 0x10` / `0x95 0x11` -> target A, B or Master. |
| CFX knobs | `0xB6 0x17/0x37` and `0x18/0x38` -> per-channel Color FX amount (centred = off, one `colorFx` type chosen on screen). |
| SMART CFX / SMART FADER | Not mapped, no LED. |
| Pad modes | HOT CUE, BEAT JUMP and BEAT LOOP are wired. PAD FX1, PAD FX2, SAMPLER, KEYBOARD and KEY SHIFT pads do nothing. |
| LEDs | FX ON/OFF (steady on when enabled), pad-mode buttons, transport, loops, hot cue and beat-loop pads. |

Free real estate: every FX-related FLX4 gesture that rekordbox uses beyond the basics (pad FX, release FX, Smart CFX, SHIFT+BEAT, SHIFT+ON/OFF) is currently unmapped, so a richer scheme does not displace anything that already works.

---

## 2. How rekordbox uses the FLX4 for effects

### 2.1 Beat FX section

| Control | Behaviour | Source |
|---|---|---|
| BEAT FX CH SELECT (1 / 2 / 1&2) | Chooses which channel the effect hits. With 1&2 the beat and effect type are synchronised between rekordbox's FX unit 1 and unit 2 (beat settings may still drift because the decks' BPMs differ); sync does not work in single mode. | FLX4 manual p.68 |
| BEAT FX SELECT | Next effect; with SHIFT, previous effect. | FLX4 manual p.31, p.68 |
| BEAT left / right | Fewer / more beats of synchronisation; for parameter-based effects such as reverb they set the parameter instead of beats. | FLX4 manual p.31-32, p.69; rekordbox 5.5 manual p.151 |
| SHIFT + BEAT left | Beat FX tempo = BPM Auto (track BPM is the base). | FLX4 manual p.31, p.69; hardware diagram |
| SHIFT + BEAT right | Beat FX tempo = BPM Tap (tap the button in rhythm to set the BPM). | FLX4 manual p.32, p.69; hardware diagram |
| LEVEL/DEPTH | "Adjusts the parameter of Beat FX". Its meaning differs per effect. The wet/dry balance and per-effect parameters live on the on-screen FX panel, not on the hardware. | FLX4 manual p.32, p.69; rekordbox 5.5 manual p.114-115, p.151-152 |
| BEAT FX ON/OFF | Toggle; the button flashes while the effect is on. | FLX4 manual p.32, p.69 |
| SHIFT + BEAT FX ON/OFF | Release FX: turns Beat FX off and plays the selected Release FX. | FLX4 manual p.32, p.69 |

rekordbox's on-screen FX panel has a single mode (one effect per unit, several parameters, parameter snapshots per effect, the L/D position is not saved) and a multi mode (up to three effects per unit, each with a balance knob and a beat selector), plus a BPM AUTO/TAP selector and a Release FX selector with its own beat length (rekordbox 5.5 manual p.114-115, p.151-153).

### 2.2 Release FX

- Three types: Echo Out, Vinyl Brake, Backspin. The type and length (beats) are picked on the software FX panel, not on the controller. ([We Are Crossfader](https://wearecrossfader.co.uk/blog/ddj-400-hidden-features/); rekordbox 5.5 manual p.153)
- On the FLX4 it is fired from the Beat FX section with SHIFT+ON/OFF, or from a pad that has a Release FX assigned (see 2.3).
- It is a "smooth exit": the running Beat FX is cancelled and the release effect plays, after which sound returns to normal (rekordbox 5.5 manual p.150, p.153). The Preferences > Controller > Effect tab can also tie Release FX to turn Sound Color FX off at the same time (rekordbox 5.5 manual p.156).

### 2.3 PAD FX 1 / PAD FX 2

| Aspect | Behaviour | Source |
|---|---|---|
| Entering | PAD FX1 button = Pad FX mode 1; SHIFT + PAD FX1 = Pad FX mode 2. | FLX4 manual p.58; hardware diagram |
| Pad count | 8 pads per deck, 4 banks: mode 1 plain = effects A-H, mode 1 with SHIFT held = I-P, mode 2 plain = Q-X, mode 2 with SHIFT held = Y-AF (32 slots). | FLX4 manual p.58 |
| Momentary | Press and hold turns the effect on, release turns it off. The effect uses the effect type and beat count assigned to that pad. | FLX4 manual p.59; rekordbox 5.5 manual p.156 |
| Parameters per pad | Each pad stores its effect type, beats and parameter values (edited on screen via the Pad FX editor). Release FX pads additionally have a Hold toggle: on = press toggles, off = hold-to-run. | rekordbox 5.5 manual p.157; [Pad Editor guide](https://cdn.rekordbox.com/files/20210119132842/rekordbox6.5.0_pad_editor_operation_guide_EN.pdf) |
| Multiple pads | Pressing several pads with different effects runs them all at once; if two have different beats, only the last one pressed is applied. | rekordbox 5.5 manual p.156-157 |
| Release FX on a pad | Hold a normal effect pad, then press the Release FX pad: the other pad FX switch off and the release effect plays. | FLX4 manual p.59; rekordbox 5.5 manual p.157 |
| Beats while holding | While a pad is held, the BEAT left/right buttons temporarily change that pad effect's beats (documented in the rekordbox 5.5 manual, generic controller text; not repeated in the FLX4 manual, so treat as to-verify on the FLX4). | rekordbox 5.5 manual p.157 |
| Lock trick | Hold the pad FX pads and then switch pad mode: the effects stay on without holding the pads; return to the Pad FX mode and tap the lit pads to switch them off. Community-documented for the DDJ-400, same pad architecture. | [We Are Crossfader](https://wearecrossfader.co.uk/blog/ddj-400-hidden-features/) |

### 2.4 SMART CFX and SMART FADER

- With SMART CFX off, each CFX knob is the channel's Sound Color FX (default filter); centre = unchanged. With SMART CFX on, the same knob drives a Smart CFX combo for that channel; centre = unchanged. (FLX4 manual p.26)
- Press toggles Smart CFX; SHIFT + press changes the preset and opens a preset screen on rekordbox. Some presets also change loops and key in addition to the effect; while Smart CFX is on, some effects and trim cannot be controlled from the rekordbox screen. (FLX4 manual p.27, p.70; hardware diagram)
- The presets reported for rekordbox are Phantom Echo, Reflect Echo, Mobius Echo, Vaporize, Noise Chopper, Cyber Jet, Cyber Pitch and Twister (they combine echo, reverse, oscillator, reverb, chopping, delay and pitch blocks). ([Pro Mobile review](https://m.promobile.org.uk/issues/review/196/REVIEWED-Pioneer-DJ-DDJFLX4), [DJGear2K](https://djgear2k.com/pioneer-ddj-flx4-vs-ddj-flx6/); the blocks inside each come from an AlphaTheta support article about a bug where some of them stop working after switching CH SELECT, [support.alphatheta.com/en-us/articles/12185533301913](https://support.alphatheta.com/en-us/articles/12185533301913), seen through search results only because the page returned 403 to our fetcher.)
- Serato flavour: SMART CFX gives preset 1 (echo type); SHIFT + SMART CFX gives preset 2 (reverb type). (FLX4 manual p.103)
- SMART FADER: when on, moving the channel faders or crossfader automatically adjusts BPM, EQ low and an echo during the transition, and forces Beat Sync on. (FLX4 manual p.71)

### 2.5 What rekordbox shows

Per the manuals: a preset screen for Smart CFX when the preset is changed; the FX panel shows the active effect, beat or parameter, wet/dry, a BPM auto/tap indicator, and a Release FX selector with its beat length; Pad FX shows the preset effect on each pad with lit pads while active. rekordbox does not show hardware knob positions or pickup state, and the hardware itself has no display, which is the gap deckforge's UI can fill better.

---

## 3. Patterns from other software

Behavioural references only; nothing is copied from any GPL code.

| Software / hardware | Pattern | What deckforge can borrow |
|---|---|---|
| Traktor FX unit, Single mode | One effect, one dry/wet knob, three parameter knobs, four buttons; the first button is on/off, the others toggle parameter-specific functions (for Delay: filter reset, feedback on/off, freeze). ([Traktor X1 MK3 manual](https://docs.native-instruments.com/online-guides/traktor-x1-mk3-manual/en/effect-control); [Effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference)) | The parameter model: a small fixed set of named parameters per effect, plus a dedicated wet/dry. |
| Traktor FX unit, Group mode | Three effects in series; each knob is a "one-knob" version of that effect's key Single-mode parameter (Delay -> rate at medium feedback, Reverb -> size). Same dry/wet for the chain. ([Effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference); [DJ TechTools](https://djtechtools.com/2015/04/19/serato-dj-fx-vs-traktor-fx/)) | Smart CFX presets as macros: one knob, several parameters moving along fixed curves. |
| Traktor shift layer, pickup | SHIFT on a hardware knob switches which parameter it controls; if the knob position does not match, the parameter does not jump and arrows on screen show which way to turn until the knob "picks up" the value. ([X1 MK3 manual](https://docs.native-instruments.com/online-guides/traktor-x1-mk3-manual/en/effect-control)) | The exact scheme for SHIFT+LEVEL: pickup with on-screen arrows. |
| Traktor Tweaker | Encoders act as button when pressed and knob when turned; a separate FX HOLD is momentary and reverses the on state. ([Traktor reference guide](https://cf3.zzounds.com/media/guide2-3d1923db7a695b40305d45508fdb02a7.pdf), via search results) | Press-and-turn, and "momentary reverses state" as a lock-free way to perform. |
| Traktor one-button builds | A single held button loads an effect, snaps several knobs to preset values (snapshots) and auto-ramps one knob while held; release turns the unit off, leaving a tail. ([DJ TechTools](https://djtechtools.com/2014/02/10/creating-one-button-fx-builds-in-traktor/)) | Pad FX presets as full snapshots with an optional ramp. |
| Serato DJ-FX | Single FX: dry/wet first, then two parameter knobs and buttons. Multi FX: up to three effects, no unit wet/dry, each knob is that effect's depth, other parameters fixed to sensible presets; both include a beat multiplier. ([DJ TechTools](https://djtechtools.com/2015/04/19/serato-dj-fx-vs-traktor-fx/); [Serato support](https://support.serato.com/hc/en-us/articles/224970027-Using-DJ-FX), Serato snippets only because the page returned 403) | Same Single/Multi split; fixed good presets when there is no room for knobs. |
| Serato on the FLX4 pads | Pad FX mode: top-row pads 1-4 held-while-pressed (reported as Echo, Flanger, Reverb, Repeater); bottom-row pads 5-8 are latching transition effects (Echo Out, Backspin, Braker, Roll Out). The DDJ-400 layout is reported to match the FLX4. (Serato DDJ-400 quickstart via search results, unconfirmed; FLX4 manual p.93 gives the pad slot count, Serato DJ Pro slots A-H.) | A momentary row and a transition row on the same bank. |
| djay (unofficial FLX4 mapping v3.1) | SELECT cycles the effect of the chosen deck; the BEAT arrows adjust the main effect parameter; SHIFT+BEAT right = BPM tap, SHIFT+BEAT left = restore analysed BPM; LEVEL/DEPTH = dry/wet; SHIFT+ON/OFF toggles a deck-level stem/EQ effect. ([Algoriddim community](https://community.algoriddim.com/t/ddj-flx4-unofficial3-0-midi-mapping/24144)) | BEAT arrows as parameter controls; SHIFT on ON/OFF for a second function. |
| djay Instant FX | Eight fixed one-tap effects with a Hold mode that latches an effect; Instant FX parameters cannot be adjusted (so a 1-beat echo out is impossible). ([Algoriddim help](https://help.algoriddim.com/user-manual/djay-pro-mac/dj-tools/effects/effects-section), via search results; [community thread](https://community.algoriddim.com/t/echo-out-drums-or-instrumentals-as-instant-fx-instead-of-manual-fx/18189)) | Warns against making pad FX presets fixed: they must be editable and expose a beat length. |
| Mixxx FLX4 (behavioural) | Mixxx maps only the FX unit 1: BEAT left/right move focus between effects in the chain, LEVEL/DEPTH = unit mix, SHIFT+LEVEL/DEPTH = meta-knob of the focused effect, SHIFT+ON/OFF = disable all effects and kill wet/dry. Soft takeover is enabled on LEVEL/DEPTH. Pad FX and Smart CFX are not implemented. ([Mixxx manual 2.4](https://manual.mixxx.org/2.4/en/hardware/controllers/pioneer_ddj_flx4)) | Soft takeover on the shared knob; "SHIFT+LEVEL = second parameter" is an accepted convention on this hardware. |

Recurring ideas, summarised:

1. One shared knob + SHIFT layer + pickup is the standard way to double a control's reach.
2. Single (several parameters) vs Group (macro per knob) is the standard way to trade depth for speed; Smart CFX is Group-style.
3. Momentary press-and-hold beats toggles for performance, but transitions (echo out, brake) are better as one-shot or latched.
4. Pads that carry a full parameter snapshot (kind, beats, wet, a second parameter) are faster than "select, then turn".
5. Hold-a-pad + a nearby control modifies that pad's voice (rekordbox: BEAT left/right changes its beats).

---

## 4. Proposed deckforge scheme

### 4.1 Principles

- Additive. Everything that works today keeps working with the same bytes. SHIFT+BEAT left/right and SHIFT+ON/OFF are currently ignored, so reassigning them breaks nothing.
- Stay close to rekordbox where users already have muscle memory: SELECT, SHIFT+SELECT, BEAT left/right, ON/OFF, SHIFT+ON/OFF = Release FX, SMART CFX toggle, SHIFT+SMART CFX = next preset, PAD FX1 / SHIFT+PAD FX1 for the two FX pad banks.
- No hidden state: anything held, latched or picked up is visible on screen; anything the hardware can do the mouse can do.
- The layer is "any SHIFT": either deck's SHIFT (`0x90 0x3F` or `0x91 0x3F`) counts, because the FX section is shared.

### 4.2 Parameter model

Each Beat FX kind exposes:

- `time`: beat division, or percent for reverb (existing, set by BEAT left/right and by PAD FX2 pads).
- `p0` Level/Depth (0..1). The hardware LEVEL/DEPTH knob without SHIFT. Same name as the knob, same as today.
- `p1`, `p2`: named sub-parameters, reached with SHIFT. Not every kind has `p2`.

| Kind | `time` | `p0` (LEVEL) | `p1` | `p2` |
|---|---|---|---|---|
| delay, echo | beats | Wet | Feedback | Tone (high-pass/low-pass in the loop) |
| upEcho, downEcho | beats | Wet | Feedback | Pitch drift per repeat |
| pingPong | beats | Wet | Feedback | Tone |
| spiral | beats | Wet | Feedback | Tone |
| reverb | percent | Wet | Tone (high-pass/low-pass tilt) | none |
| flanger | beats (LFO period) | Depth | Feedback | none |
| phaser | beats | Depth | Feedback | none |
| roll | beats (roll length) | Wet | Decay | none |
| trans | beats (gate rate) | Depth | Gate shape (duty) | none |
| filterSweep | beats (sweep period) | Depth | Resonance | none |

Assumption: the DSP can expose these. Today `feedback_at(level)` in `crates/dsp/src/fx/beat.rs` derives echo feedback from the single level value, and the loop has `feedback_lp/hp` filters, so Feedback and Tone are separable cheaply for the echo family. Names and ranges for the other kinds are proposals for the DSP owner to confirm; a first release can ship only `p1`.

### 4.3 Beat FX section

Message bytes in column 3 are all VERIFIED rows in the MIDI doc (sections 4.7, 4.1).

| Gesture | Action | Bytes (data2 `7F` press / `00` release) |
|---|---|---|
| BEAT FX SELECT | Next kind. Focus resets to `p1`. | `94 63` |
| SHIFT + SELECT | Previous kind. | `94 64` |
| BEAT left / right | `time` down / up (beats or percent). While a PAD FX1 pad is held, adjusts that pad's voice instead (see 4.5). | `94 4A` / `94 4B` |
| SHIFT + BEAT left / right | Focus the previous / next sub-parameter page (`p1` -> `p2` -> wraps; kinds with only `p1` do nothing). Replaces rekordbox's BPM Auto/Tap on those two buttons; see trade-off below. | `94 66` / `94 6B` |
| LEVEL/DEPTH, SHIFT not held | `p0` (Level/Depth), with pickup after any layer change. | `B4 02` + `B4 22` (14-bit) |
| LEVEL/DEPTH, SHIFT held | Focused sub-parameter (`p1`/`p2`), with pickup. The knob has no separate SHIFT message, so the host decides by the tracked SHIFT state. | `B4 02` + `B4 22` |
| FX ON/OFF | Toggle the Beat FX unit. | `94 47` (CH1, CH1&2) or `95 47` (CH2) |
| SHIFT + FX ON/OFF | Release FX: turns the unit off, fires the selected Release FX (type chosen on screen, length in beats). Fires on press, release ignored. Fires even if Beat FX was already off, so it doubles as a stand-alone transition. | `94 43` or `95 43` |
| CH SELECT switch | Target A / B / Master (as today). 1&2 maps to Master, not "both channels", which differs slightly from rekordbox. | `94 10`, `95 11` |

Trade-off recorded: rekordbox uses SHIFT+BEAT left/right for BPM Auto/Tap on the Beat FX clock. deckforge has no FX tap-BPM today (`beatFx.bpm` is derived from the target deck, else the master deck, else 120). Parameter paging is worth more than tap tempo for this app. If tap BPM is wanted later, candidates are an on-screen TAP button or a hold-time split on SHIFT+BEAT right (tap = page, hold = tap-tempo); I recommend deferring.

#### Pickup (knob catch-up)

The LEVEL/DEPTH knob is absolute and unmotorised, and the host cannot move it. Whenever the knob's controlled target changes (SHIFT pressed or released, focus page changed, kind changed, pad held or released), the new target is ignored until the physical value reaches the target's stored value:

- Engage when `|knob - stored| <= 3%` (same threshold as `TAKEOVER_THRESHOLD` already used for the tempo fader), or when two consecutive 14-bit samples bracket the stored value (so fast sweeps do not miss it).
- While waiting, the UI shows the stored value as a ghost marker and an arrow telling which way to turn (Traktor-style).
- On kind change, per-kind stored values are restored (per-kind parameter memory, like rekordbox single mode snapshots).
- On connect the knob position is unknown until its first message; the existing keepalive SysEx may make the unit report positions (INFERRED in the MIDI doc), which would seed pickup.

### 4.4 Pad mode map

No new "FX mode". The mode buttons the FLX4 already has are the switch, per deck, exactly as rekordbox does it. Deck 1 can be in PAD FX1 while deck 2 stays in HOT CUE.

| Pad mode | Button bytes (deck 1 / deck 2) | Pad notes | deckforge function |
|---|---|---|---|
| HOT CUE | `90 1B` / `91 1B` | `00..07` | unchanged |
| BEAT JUMP | `90 20` / `91 20` | `20..27` | unchanged |
| BEAT LOOP (SHIFT+BEAT JUMP) | `90 6D` / `91 6D` | `60..67` | unchanged |
| PAD FX1 | `90 1E` / `91 1E` | `10..17` | new: Performance FX (4.5) |
| PAD FX2 (SHIFT+PAD FX1) | `90 6B` / `91 6B` | `50..57` | new: Beat FX gate and quick-pick (4.6) |
| SAMPLER, KEYBOARD, KEY SHIFT | `22`, `69`, `6F` | `30..37`, `40..47`, `70..77` | untouched (reserved, not used by FX) |

Pad message addressing (VERIFIED, MIDI doc 4.8): deck 1 pads `0x97` (SHIFT held: `0x98`), deck 2 pads `0x99` (SHIFT held: `0x9A`); note = mode base + pad index (pad 1 = base + 0, pad 8 = base + 7); `7F` press, `00` release.

### 4.5 PAD FX1: Performance FX (deck-local)

Scope: acts on the pad's own deck channel (deck 1 pads -> channel A, deck 2 pads -> channel B), independent of the Beat FX unit's target. Per-pad data is a full snapshot: kind, `time`, `p0`, `p1`, plus pad behaviour. Presets are starting points, tuned by ear and editable on screen.

Bank A (SHIFT not held, pad notes `10..17` on `0x97` / `0x99`):

| Pad | Notes (D1 / D2 pad status) | Kind and preset | Behaviour | Needs |
|---|---|---|---|---|
| 1 | `97 10` / `99 10` | Echo, 1/2 beat, level 70%, feedback 55% | Momentary | exists |
| 2 | `97 11` / `99 11` | Ping Pong, 1/4 beat, level 70%, feedback 50% | Momentary | exists |
| 3 | `97 12` / `99 12` | Reverb, 60%, level 60% | Momentary, tail rings out after release | exists |
| 4 | `97 13` / `99 13` | Roll, 1/2 beat, level 100% | Momentary | exists |
| 5 | `97 14` / `99 14` | Echo Out, release length 2 beats | One-shot transition | new behaviour on existing echo (cut dry, ring the tail) |
| 6 | `97 15` / `99 15` | Backspin, 1 beat | One-shot transition | new DSP |
| 7 | `97 16` / `99 16` | Vinyl Brake, 1 beat | One-shot transition | new DSP |
| 8 | `97 17` / `99 17` | Roll Out, roll 1/4 with level ramp-down over 2 beats | One-shot transition | exists, plus a level ramp |

Bank B (SHIFT held, pad notes `10..17` on `0x98` / `0x9A`): the remaining Beat FX kinds, momentary.

| Pad | Notes (D1 / D2 shifted pad status) | Kind and preset |
|---|---|---|
| 1 | `98 10` / `9A 10` | Flanger, 4 beats, depth 80% |
| 2 | `98 11` / `9A 11` | Phaser, 4 beats, depth 80% |
| 3 | `98 12` / `9A 12` | Trans, 1/4 beat, depth 100% |
| 4 | `98 13` / `9A 13` | Filter Sweep, 4 beats, depth 80% |
| 5 | `98 14` / `9A 14` | Down Echo, 1 beat |
| 6 | `98 15` / `9A 15` | Up Echo, 1 beat |
| 7 | `98 16` / `9A 16` | Spiral, 1 beat |
| 8 | `98 17` / `9A 17` | Delay, 3/4 beat |

All 12 Beat FX kinds are reachable from pads across the two banks. This takes the place of rekordbox's four banks (A-AF) with a simpler 16-slot model.

Rules:

1. Momentary: effect on at press, off at release. The release is matched by pad note and deck, ignoring the shift channel, so pressing a pad and then pressing SHIFT before letting go cannot strand the effect.
2. Several pads together: different kinds stack; two pads of the same kind share one voice and the most recent press wins for `time` (matches rekordbox's "only the last one with different beats").
3. Hold a pad and press BEAT left/right: that pad's `time` steps for as long as it is held (temporary, not saved). While a pad is held the Beat FX section's BEAT buttons are captured by it; the Beat FX unit's `time` does not change.
4. Hold a pad and turn LEVEL/DEPTH: scales that pad's `p0` live, with the same pickup rule. Release restores the unit's own knob target.
5. Transition pads (5-8 in bank A) are one-shots: press fires, release ignored, no hold needed. They play at the selected pad length and the deck returns to normal sound afterwards; they act on the live signal, not on transport position, so playback and sync are not disturbed.
6. Optional lock (to match the known Pioneer trick): hold a momentary pad and press another pad-mode button; the pad effect latches. Return to PAD FX1 and tap a lit pad to release. This depends on open question Q4 below.
7. Safety: on pad-mode change, SHIFT-state loss, controller disconnect, or app focus loss, all non-latched pad voices are released; latched voices are visible in the UI and can be cleared with one click.

### 4.6 PAD FX2: Beat FX gate and quick-pick (drives the shared Beat FX unit)

Scope: the one global Beat FX unit, with the target taken from the CH SELECT switch. This is the "pick the beat division instantly" mode.

Unshifted pads (notes `50..57`, statuses `0x97` / `0x99`): division gate.

| Pad | Notes (D1 / D2) | Beat kinds | Reverb |
|---|---|---|---|
| 1 | `97 50` / `99 50` | 1/8 | 10% |
| 2 | `97 51` / `99 51` | 1/4 | 20% |
| 3 | `97 52` / `99 52` | 1/2 | 35% |
| 4 | `97 53` / `99 53` | 3/4 | 50% |
| 5 | `97 54` / `99 54` | 1 | 65% |
| 6 | `97 55` / `99 55` | 2 | 80% |
| 7 | `97 56` / `99 56` | 4 | 90% |
| 8 | `97 57` / `99 57` | 8 | 100% |

Behaviour ("FX gate"): pressing a pad sets `time` immediately and, if Beat FX is currently off, turns it on while the pad is held; releasing returns it to off. If Beat FX was already on (latched with FX ON/OFF) the pad only changes `time` and release does nothing. 16 beats stays reachable with BEAT right. If both decks' pads are used, last press wins.

Shifted pads (notes `50..57`, statuses `0x98` / `0x9A`): kind quick-pick, one press, no gate.

| Pad | Notes (D1 / D2 shifted) | Kind |
|---|---|---|
| 1 | `98 50` / `9A 50` | Echo |
| 2 | `98 51` / `9A 51` | Ping Pong |
| 3 | `98 52` / `9A 52` | Reverb |
| 4 | `98 53` / `9A 53` | Roll |
| 5 | `98 54` / `9A 54` | Flanger |
| 6 | `98 55` / `9A 55` | Phaser |
| 7 | `98 56` / `9A 56` | Trans |
| 8 | `98 57` / `9A 57` | Filter Sweep |

The list is a default "favourites" set, user-editable. Selecting a kind restores that kind's remembered parameters.

### 4.7 SMART CFX

| Gesture | Action | Bytes |
|---|---|---|
| SMART CFX press | Toggle Smart CFX (both channels, global like rekordbox). | `96 00` |
| SHIFT + SMART CFX | Next Smart CFX preset (wraps). | `96 08` |
| CFX knob, channel 1 / 2 | With Smart CFX off: existing Color FX. With it on: the preset's macro, driven by the knob's position. | `B6 17` + `B6 37` / `B6 18` + `B6 38` |
| SMART FADER | Left unmapped in this proposal. | `96 01`, `96 09` |

Macro behaviour: the knob position is bipolar `b` in `-1..1` with a 2% dead zone around centre (same as the existing `0.02` bypass in the UI). Each preset defines a left curve and a right curve; several DSP parameters move together.

| Preset (deckforge-original, built from the existing Color FX blocks) | Knob left (counter-clockwise) | Knob right (clockwise) |
|---|---|---|
| 1 Echo Wash | Low-pass closes, echo send (1/2 beat) rises | High-pass opens, echo send and feedback rise together |
| 2 Riser | Low-pass closes plus reverb tail | High-pass opens, noise bed rises, gate rate tightens 1/4 -> 1/16 (build-up) |
| 3 Space | Reverb send rises with low-pass | Reverb send rises with high-pass (washed-out) |
| 4 Crush and Chop | Gate chop rate rises with low-pass | Bit-crush depth rises |

Rationale: rekordbox's presets are named recipes of echo, reverb, chopper, pitch and oscillator blocks; our four are simpler recipes built from effects deckforge already has (`filter`, `space`, `dubEcho`, `crush`, `noise` Color FX plus the Beat FX DSP), aimed at transitions and builds. More can follow once the DSP supports reverse and pitch blocks.

Safe-arm: when Smart CFX is turned on (or preset changed) with a knob off-centre, that knob is ignored until it passes through the dead zone, so the sound does not lurch. The SMART CFX LED blinks while any knob is waiting to arm. This is the centre-catch version of pickup.

Interaction with Beat FX: Smart CFX is a separate DSP path from the Beat FX unit, so there is no equivalent of rekordbox's known bug where switching CH SELECT after enabling Smart CFX switches some effects off.

### 4.8 LED feedback (only LEDs the MIDI doc says can be driven)

All values on the same status/data1 as the matching input row; `7F` on, `00` off; host-generated blinking by toggling in the feedback tick (the existing blink logic). Pad LEDs are only visible in the active mode; write both the unshifted and shifted status for each pad (MIDI doc section 5).

| LED | Address | State -> value |
|---|---|---|
| PAD FX1 / PAD FX2 mode buttons | `90 1E`, `90 6B` / `91 1E`, `91 6B` (existing mode-button logic) | Lit while that mode is active on the deck. |
| PAD FX1 pads | `97/98/99/9A 10..17` | Lit = slot assigned and idle. Fast blink (about 4 Hz) = its voice is running (held, latched or tail ringing). Off = empty slot. |
| PAD FX2 division pads | `97/99 50..57` (write `98/9A` too) | Lit = the current division (or nearest reverb percent); others off. Blink while the gate holds the FX on. |
| PAD FX2 quick-pick pads | `98/9A 50..57` (write `97/99` too) | Lit = the current Beat FX kind, if it is a favourite. |
| BEAT FX ON/OFF | `94 47` and `94 43` (existing twin write) | Per PDF footnote *4: a Note On blinks and a Note Off gives a steady light. So `7F` = FX running (blink) and `00` = armed (steady). The button can never be fully dark; confirm on hardware (Q1). |
| SMART CFX | `96 00` | Lit = Smart CFX on; blink = a knob is waiting to arm; off = Color FX. No shifted LED exists. |
| Everything else in the Beat FX section (SELECT, BEAT left/right, LEVEL/DEPTH, CH SELECT) | none | No documented LED; feedback is on screen only. |

No dim or brightness levels are used; the dim pad value reported by a third-party doc is INFERRED and not relied on.

### 4.9 Mode switching and what stays untouched

- Switching: press the pad-mode button on that deck (PAD FX1, or SHIFT+PAD FX1 for PAD FX2). Press HOT CUE / BEAT JUMP / SHIFT+BEAT JUMP to return. Nothing global changes; each deck keeps its own pad mode.
- Not touched: hot cue pads (`00..07`), beat jump pads (`20..27`), beat loop pads (`60..67`), IN/OUT/4 BEAT/CUE-LOOP CALL buttons, loop halve/double, jog and tempo. The BEAT FX BEAT buttons live on `0x94` and are distinct from the CUE/LOOP CALL buttons on `0x90/0x91`.
- Collision check: PAD FX1 pad notes `10..17` on `0x97..0x9A` do not collide with IN `0x10` and OUT `0x11` on `0x90/0x91`; dispatch is always on the (status, note) pair, as the existing code already does.
- A SHIFT+LEVEL gesture requires nothing from the pads; they are independent, so the user can be in HOT CUE on both decks and still use SHIFT+LEVEL and SHIFT+BEAT.

---

## 5. UI behaviour

Principle: every hardware parameter change must be readable from across the booth, and every hardware gesture must be reproducible with the mouse. The UI keeps a single source of truth in the engine snapshot; the controller reports intent via `ControllerAppAction`-style events for transient items (pickup, SHIFT state).

### 5.1 Beat FX bar (`BeatFx.svelte`)

- A parameter strip next to the Level knob: one slim bar per parameter of the current kind (`Level`, `Feedback`, `Tone` ...). The one the hardware would currently move is highlighted with the FX accent; with SHIFT held the strip also shows an arrow on the focused page.
- A SHIFT chip appears while the controller SHIFT is held, so the user sees which layer the knob is in.
- Pickup: if the knob target changed and the knob does not match, the bar shows a ghost marker at the stored value and a left/right chevron until picked up.
- The beat stepper (or percent) stays as today.
- A Release FX control beside ON: type (Echo Out / Backspin / Vinyl Brake) and length (beats); it flashes when SHIFT+ON/OFF or a transition pad fires it.

### 5.2 Transient popover (any hardware change)

When LEVEL/DEPTH, BEAT left/right, SELECT or a pad parameter changes from the hardware, a compact popover anchored to the Beat FX bar (or the deck for pad FX) shows for about 1.2 s after the last change: "Echo / Feedback / 62%" with a larger value and a mini bar. While SHIFT is held it stays open and shows all pages (Level, Feedback, Tone) with the focused one highlighted. During pickup it shows "turn up to 62%".

### 5.3 Pad FX panel per deck

When a deck is in PAD FX1 or PAD FX2, show a 4x2 grid mirroring the physical pad layout under that deck (or in the deck header): each pad shows kind icon, name and `time`; held and latched pads are highlighted; transition pads have a distinct tint; the SHIFT bank is peeked on a hold of the SHIFT chip. Each pad is clickable (momentary press) and editable (kind, time, p0, p1, hold behaviour). A "save current Beat FX unit to this pad" action is on the editor (no hardware gesture proposed for it).

Active pad voices appear as small chips on the corresponding channel strip in the mixer (for example "Echo 1/2") with a click-to-clear for latched ones.

### 5.4 Mixer Smart CFX (`Mixer.svelte`)

- A Smart CFX switch next to the Color FX type selector. When on, the selector becomes a preset selector and each CFX knob shows its preset-specific left/right labels (for example "Wash <- | -> Rise").
- A bipolar macro bar per channel shows which blocks are being driven and by how much.
- While safe-arm is waiting, the knob shows an "arm: turn to centre" hint.
- A SHIFT+SMART CFX change shows the preset name as a toast, like rekordbox's preset screen.

### 5.5 Mode toasts

Pad mode changes show a one-line toast ("Deck A: PAD FX 1"). If a pad voice is latched when the mode changes, a small badge persists on the deck.

---

## 6. Implications for engine and contract (design only)

These follow from the proposal; none exist today.

| Area | Needed |
|---|---|
| Beat FX snapshot | Parameter list per kind (name, value 0..1, focused), `p1`/`p2` stored per kind; commands to set a sub-parameter and to move focus. |
| Pad FX | Per-deck pad voice bank (several voices, kinds stack, same kind last-press-wins), commands for press/release with per-pad snapshot, temporary `time` override, per-pad `p0` override, latch/clear; a pad preset table with defaults and persistence. |
| Release FX | A release engine on the live signal: echo out, backspin, vinyl brake with a beat length; a selectable type for SHIFT+ON/OFF. |
| Smart CFX | Preset enum, global on/off, per-channel macro amount; sits beside the existing `colorFx` path. |
| Controller to UI | Events for SHIFT state, pickup state (target, stored value, direction), parameter-change HUD, mode toasts. |
| Controller state | Track SHIFT per deck; pad voices held by (deck, note); pickup per target; Smart CFX arm state. |

Suggested phasing so the first release is already useful:

1. Phase 1: SHIFT+LEVEL second parameter (feedback), SHIFT+BEAT paging, PAD FX2 gate and quick-pick pads, SHIFT+FX ON/OFF as Echo Out, on-screen parameter strip and popover.
2. Phase 2: PAD FX1 performance bank A and B with per-pad snapshots, transition pads, pad panel UI, press-and-hold BEAT / LEVEL on pad voices.
3. Phase 3: Backspin and Vinyl Brake DSP; Smart CFX presets with safe-arm and LED.

---

## 7. Open questions and items needing physical verification

The controller has barely been used, so each of these is an assumption.

| # | Item | Why it matters | How to verify |
|---|---|---|---|
| Q1 | FX ON/OFF LED polarity and which addresses drive it (`94 47`, `94 43`, `95 47`, `95 43`). Footnote *4 of the PDF says Note On blinks and Note Off gives steady light; the PDF table shows no LED row. | Defines the "running" vs "armed" display. | Send both values to each address and watch. |
| Q2 | Do pad LEDs accept values other than `00` and `7F` (dim at `0x20`)? Is host-generated blinking at 4 Hz clean over USB MIDI? | LED language for pads (lit vs blink) and the optional dim level. | Send `0x01..0x7F` to a pad LED; run a 4 Hz toggle. |
| Q3 | Does LEVEL/DEPTH send the same CC (`B4 02/22`) while SHIFT is held, and in what MSB/LSB order? Are SHIFT-state notes reliable (including release while held) if the user lets go of one deck's SHIFT while holding the other? | The entire SHIFT+LEVEL layer relies on host-tracked SHIFT. | MIDI monitor with SHIFT held on each deck separately and together. |
| Q4 | If the pad mode is switched while a pad is held, which note and status does the later release carry? What happens if SHIFT is pressed or released between press and release of a pad? | Determines both the lock trick and the stuck-effect safety rule. | MIDI monitor: hold pad, press another mode button, release pad; repeat with SHIFT. |
| Q5 | Pad release encoding: Note On velocity 0, or Note Off (`0x8n`)? The existing code accepts both. | Momentary FX depend on a reliable release. | MIDI monitor. |
| Q6 | Do the SHIFT variants arrive exactly as in the PDF: SELECT `94 64`, BEAT left `94 66`, BEAT right `94 6B`, ON/OFF `94 43` / `95 43` depending on CH SELECT? | These are new bindings. | MIDI monitor. |
| Q7 | The discrepancy D1 in the MIDI doc: LEVEL/DEPTH channel (PDF says 6, status says `B4`). | If status is `B5`, the knob is silent. | Already listed in the MIDI doc; also listen on `B5`. |
| Q8 | The mode-button LED ownership and exclusivity (MIDI doc section 6 item 6): does the host need to turn the previous mode LED off? Does PAD FX1 / `0x6B` behave like the other modes? | Mode-button LED correctness. | Echo `90 1E 7F` then `90 6B 7F`, then `90 1B 7F`. |
| Q9 | Does the CFX knob's centre read near `0x2000`, with what dead-zone width? Is the knob centred in the physical detent? | Safe-arm dead zone width and macro centre. | MIDI monitor, centred knob, repeat several times. |
| Q10 | Does rekordbox's "hold pad + BEAT left/right changes that pad's beats" exist on the FLX4 or only in older generic text? | Rule 3 in 4.5 is a deckforge design choice either way; parity claim only. | Try it in rekordbox with the FLX4 if available. |
| Q11 | LEVEL/DEPTH initial position at connect: does the keepalive SysEx cause the unit to report it? | Pickup seed and first-touch jump. | Observe the first CC burst after connect (MIDI doc section 6). |
| Q12 | Release FX when the Beat FX is already off: fire anyway, or ignore? | Design decision: this proposal fires it. | User testing; low risk either way. |
| Q13 | 1&2 maps to Master, rekordbox maps it to both channels. Is Master the right default for deckforge's CH SELECT? | Release FX and pad gate target. | A decision, not a verification. |
| Q14 | The DSP can expose the proposed sub-parameters per kind, and Backspin / Vinyl Brake are implementable as signal effects without disturbing transport. | Scope and phasing in section 6. | DSP review of `crates/dsp/src/fx/beat.rs`, `pitch.rs`, `ring.rs`. |
| Q15 | Is the Serato pad layout (Echo, Flanger, Reverb, Repeater on top; Echo Out, Backspin, Braker, Roll Out on bottom) accurate for the FLX4? | Only used as inspiration for the top/bottom row split. | Serato FLX4 quickstart; the page returned 403 to the fetcher. |

---

## 8. Sources

Primary hardware and software documentation:

- DDJ-FLX4 Operating Instructions (English): https://assets.ctfassets.net/q602vtcuu3w3/2vz7GMLHawLZBK6gWpCP9V/26bd62eef3eca3714ddb1d3396d30f7b/DDJ-FLX4_manual_EN.pdf (pages cited: 26-27 CFX and SMART CFX/FADER, 31-32 Beat FX section, 58-59 Pad FX and Release FX of Pad FX, 68-71 Beat FX, Smart CFX, Smart Fader in rekordbox, 93 and 103 Serato pads and Smart CFX).
- DDJ-FLX4 Hardware Diagram for rekordbox: https://cdn.rekordbox.com/files/20221108163821/DDJ-FLX4_Hardware_Diagram_en.pdf
- DDJ-FLX4 List of MIDI messages (AlphaTheta), summarised in `docs/research/ddj-flx4-midi.md`.
- rekordbox 5.5.0 manual (effects, Pad FX, Release FX, effect panel): https://cdn.rekordbox.com/files/20200214194946/rekordbox5.5.0_manual_EN.pdf
- rekordbox 6.5.0 Pad Editor operation guide: https://cdn.rekordbox.com/files/20210119132842/rekordbox6.5.0_pad_editor_operation_guide_EN.pdf

Reviews, tutorials and community:

- We Are Crossfader, DDJ-400 hidden features (pad FX lock trick, Release FX): https://wearecrossfader.co.uk/blog/ddj-400-hidden-features/
- Pro Mobile, DDJ-FLX4 review (Smart CFX preset names): https://m.promobile.org.uk/issues/review/196/REVIEWED-Pioneer-DJ-DDJFLX4
- DJGear2K, FLX4 vs FLX6 (Smart CFX preset names): https://djgear2k.com/pioneer-ddj-flx4-vs-ddj-flx6/
- AlphaTheta support, Smart CFX effects turning off (blocks inside presets; seen via search results, page returned 403): https://support.alphatheta.com/en-us/articles/12185533301913
- The DJ Hook Up and DJ TechTools launch articles on the FLX4 (Smart CFX and Smart Fader described only in general terms): https://blog.thedjhookup.com/reviews/pioneer-ddj-flx4-review/ and https://djtechtools.com/2022/11/09/pioneer-djs-new-ddj-fx4-a-beginners-controller-with-smart-fading-effects/
- Mixxx manual 2.4, Pioneer DDJ-FLX4 (behavioural reference only, GPL project, nothing copied): https://manual.mixxx.org/2.4/en/hardware/controllers/pioneer_ddj_flx4
- Algoriddim community, DDJ-FLX4 unofficial 3.x mapping and built-in Smart CFX behaviour: https://community.algoriddim.com/t/ddj-flx4-unofficial3-0-midi-mapping/24144 and https://community.algoriddim.com/t/pioneer-ddj-flx4-cfx-button/24601
- Algoriddim help, djay effects section and Instant FX (via search results): https://help.algoriddim.com/user-manual/djay-pro-mac/dj-tools/effects/effects-section ; community thread on Instant FX limits: https://community.algoriddim.com/t/echo-out-drums-or-instrumentals-as-instant-fx-instead-of-manual-fx/18189
- Native Instruments, Traktor X1 MK3 manual, effect control: https://docs.native-instruments.com/online-guides/traktor-x1-mk3-manual/en/effect-control
- Native Instruments, Traktor Pro effect reference: https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference
- DJ TechTools, Serato DJ FX vs Traktor FX: https://djtechtools.com/2015/04/19/serato-dj-fx-vs-traktor-fx/
- DJ TechTools, one-button FX builds in Traktor: https://djtechtools.com/2014/02/10/creating-one-button-fx-builds-in-traktor/
- Serato support, Using DJ-FX (403 to fetcher; facts taken from search-result snippets, treat as secondary): https://support.serato.com/hc/en-us/articles/224970027-Using-DJ-FX
- Serato DDJ-SP1, DDJ-ERGO and DDJ-400 quickstarts (SHIFT+knob slot selection, pad FX layout; snippets only): https://support.serato.com/hc/en-us/articles/10173705065871-Pioneer-DJ-DDJ-SP1-Quickstart-Guide and https://support.serato.com/hc/en-us/articles/10418841247503-Pioneer-DJ-DDJ-400-Quickstart-Guide
