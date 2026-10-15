# FX v2 — design

Synthesis of `docs/research/fx-pro-sound.md`, `fx-tension.md` and `fx-flx4-control.md`, driven by user feedback after DJing on the DDJ-FLX4:
echo pitches on beat change; Space thins the lows instead of shimmering the highs; Reverb, Echo, Space and Noise don't build tension and sound "goofy".

## Phase 1 — make the existing effects sound professional

| # | Change | Acceptance |
|---|---|---|
| 1 | **Pitchless division change** for Echo, Delay, Ping Pong, Up/Down Echo: dual-tap 40 ms Hann crossfade, feedback taken from the crossfaded output. Tape-style glide kept only for Spiral. | 1 kHz sine through echo stays within ±5 cents across any division change; no click |
| 2 | **Echo loop character**: 2-pole HPF ~200 Hz + LPF ~4.5 kHz in the loop, subtle tanh saturation, small L/R offset, feedback `0.40 + 0.35·level^1.2`. Up/Down Echo step reduced to ±1 semitone per repeat. | repeats darken and thin naturally; loudness stays within +1.5 dB of dry |
| 3 | **Space**: dry path untouched. Wet-only HPF 500 Hz–2.5 kHz (follows knob), +4 dB shelf at 6 kHz, +12 st shimmer voice in the reverb loop, loop limiter. Knob left = darker wash, right = bright shimmer. | <0.5 dB change below 150 Hz at any knob position; shimmer partials present an octave up |
| 4 | **Reverb quality pass**: frequency-dependent decay (highs decay ~0.55× lows), wet HPF 250–350 Hz, per-line modulation scaled by RT60, dry-keyed ducking (3–6 dB, 8 ms / 200 ms) so the tail blooms in the gaps. 1–100 % size retained. | lows below 150 Hz clean; no metallic ring in tail |
| 5 | **Noise**: pink/white blend, 150 Hz HPF, swept band-pass 200 Hz–8 kHz with t² level curve, ducked by the music, falling-cutoff tail on release. | riser audibly sweeps up; never louder than the music |
| 6 | **FX frequency (LOW/MID/HI)**: Beat FX applied per band via the existing LR4 split (250 Hz / 3.5 kHz). LOW off by default for Echo-family and Reverb. | kick and bass stay dry with LOW off |

## Phase 2 — tension effects (new Beat FX kinds)

| Effect | What it is | How you perform it |
|---|---|---|
| **Pulse Verb** | Reverb + beat-locked tremolo + noise bed (Revolo-inspired). Tremolo can accelerate through a build. | ride depth up over 8–16 bars, cut on the drop |
| **Dub Tape** | Deep dub delay: HPF/LPF + tape drive in the loop, wow/flutter, controlled self-oscillation up to fb 1.05 with limiter. Time change: Clean / Tape / Reset. Throw and Freeze. | throw a vocal, ride feedback into oscillation, cut |
| **Halo** | Shimmer / freeze wash, +12 st feedback, infinite hold. | freeze a chord under a breakdown |
| **Ascend** | One-button build: ramps noise, HPF, echo division, reverb send and tremolo rate over 4/8/16/32 bars, landing on the next phrase boundary; one press releases with a hard cut. | press 16 bars before the drop |
| **Backwash** | Reverse-reverb swell that reads the track ahead of the playhead so it peaks on the next downbeat. | trigger one bar before a drop or vocal |

## Phase 3 — FLX4 control (needs a short hardware check first)

- LEVEL/DEPTH = depth; **SHIFT + LEVEL/DEPTH** = second parameter (feedback / tone / size per effect); **SHIFT + BEAT ◀▶** pages that parameter. Soft pickup with on-screen ghost marker.
- **SHIFT + FX ON/OFF = Release FX** (Echo Out / Backspin / Vinyl Brake, chosen on screen).
- **PAD FX 1** (per deck): pads 1–4 momentary Echo, Ping Pong, Reverb, Roll; pads 5–8 one-shots Echo Out, Backspin, Vinyl Brake, Roll Out; SHIFT = the other kinds. Hold pad + BEAT ◀▶ / LEVEL to tweak it live.
- **PAD FX 2**: pads set beat division instantly (1/8 … 8, or reverb %); SHIFT + pad = favourite effect quick-pick.
- **SMART CFX**: CFX knobs become one-knob macros (Echo Wash, Riser, Space, Crush & Chop).
- UI: parameter strip with focused parameter, 1.2 s popover on any hardware change, per-deck pad grid in FX pad modes.

Hardware checks before Phase 3 (via a live MIDI monitor): LEVEL/DEPTH message with SHIFT held; SHIFT variants of BEAT ◀▶ / SELECT / ON-OFF; PAD FX 1/2 mode notes and pad release notes; pad and FX ON/OFF LED behaviour; LEVEL/DEPTH status byte (B4 vs B5).

### Phase 3 as built (2026-10-14)

Byte facts from the hardware check (`docs/research/ddj-flx4-midi.md` §14); behaviour from `docs/research/fx-flx4-control.md` §4. Choices where no rekordbox data exists:

- **Pad FX run per deck.** Each channel has its own Pad FX unit (a Beat FX voice plus a Release FX), independent of the shared Beat FX unit and of the other deck. One voice per deck: the most recently pressed held pad sounds and letting go falls back to a pad still held. rekordbox stacks several pad effects at once; that is not replicated.
- **Release FX** (SHIFT + FX ON/OFF, or PAD FX 1 pads 5-8): cancels the Beat FX including its tail and plays the release on the Beat FX target (pads: on their own deck), then the signal returns, as the rekordbox manual describes. No release DSP was decoded, so the shapes are ours: Echo Out repeats the last 1/8 of the length (1/8 to 1 beat) with feedback set so the tail is 36 dB down at the end, loop band-limited to 200 Hz-4.5 kHz; Backspin spins back up to 3x and slows to a stop; Vinyl Brake slows linearly to a stop; Roll Out (pad only) loops 1/4 beat and fades over the length. Lengths 1/2, 1, 2, 4, 8 beats (default 2), capped at 6 s.
- **SHIFT + BEAT ◀▶ pages the parameter** that SHIFT + LEVEL/DEPTH drives (continuous parameters only, remembered per kind); rekordbox uses these buttons for BPM Auto/Tap.
- **Soft pickup** on LEVEL/DEPTH whenever its target changes: engages within 3 % or when the knob sweeps through the stored value.
- **Smart CFX presets** (Echo Wash, Riser, Space, Crush & Chop) are deckforge recipes of the existing Color FX blocks, a side-dependent filter amount and a beat chop; rekordbox's own presets were not reproduced. Each switch or preset change re-arms the CFX knobs from the centre.
- **FX ON/OFF LED** follows the official list's footnote (NOTE ON blinks, NOTE OFF lights steadily), written on `0x94` and `0x95`; polarity still needs a look on the hardware.

Hot cues, loops, beat jump and jog behaviour are unchanged in every phase.
