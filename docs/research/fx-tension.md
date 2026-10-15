# Tension-building DJ effects: identification, technique catalogue, deckforge proposals

Research date: 2026-10-13. Research only, no code. Quotes are kept short; everything else is paraphrased.

## 1. Identification: "Tila / Revolo / Khana delay"

The speech-to-text spellings resolve to one small Berlin hardware company and two of its products.

| Heard as | Actual name | Confidence |
|---|---|---|
| "Tyler" / "Tila" | **TEILE Elektronik** (brand "TEILE", German for "parts"; sold at teile.life) | High |
| "Revolo" | **TEIL2 "Revolo"**, reverb + tremolo box | Certain (exact name, and the maker's own definition matches) |
| "Khana / Kana / Canna delay" | **TEIL1 "the keinedelay"** (written "Keinedelay") | High. Phonetic match to the label name Keinemusik; matches the "build tension" use. |

Notes on the evidence:

- TEILE was started by Rampa of Keinemusik to build hands-on effects for live DJ sets ([Sinesquares review](https://www.sinesquares.net/musicgear/teile-revolo-review)). The Revolo was co-developed with Rampa and Keinemusik ([TEILE product page](https://teile.life/products/teil2-reverb)); the Keinedelay's prototypes were tested directly by Rampa ([MATRIXSYNTH](https://www.matrixsynth.com/2018/10/teil1-keinedelay-playable-delay.html), [Ivan Minuti](http://ivanminuti.com/journal/music/teil1-keinedelay-ckdlc)).
- There is no plugin from this company. Both products are hardware boxes for a mixer send/return. Web searches for "Kana / Kanna delay plugin" returned nothing relevant, which supports the hardware reading. The user's quote about going "really deep" into effects "to build some tension" did not turn up on any page I could fetch; it is probably from a video review or a DJ's comments. I could not verify the exact wording.
- Not an in-software effect pack: these are not rekordbox, Serato or djay add-ons. They are physical units, so "how they work" is known from the maker's text, reviews and demos, not from code.
- Both are currently out of stock at the maker (waitlist) and trade used ([TEIL1](https://teile.life/products/teil1), [TEIL2](https://teile.life/products/teil2-reverb)).
- Limitation: the official Revolo manual ([PDF](https://cdn.shopify.com/s/files/1/0949/7483/7078/files/TEIL2-Revolo-Manual.pdf)) is a one-page A3 poster with outlined text; I could not extract its text, so parameter ranges below come from the product page, the Sinesquares review and a retailer listing, and should be treated as secondary.

## 2. Product analyses

### 2.1 TEILE TEIL2 "Revolo" (reverb + tremolo)

Name: REVerb + tremOLO. A DJ-sized box (about 120 x 110 x 55 mm, USB-C powered) meant for a mixer send/return or inline use. Sources: [TEILE](https://teile.life/products/teil2-reverb), [Sinesquares](https://www.sinesquares.net/musicgear/teile-revolo-review), [Steffen Sennert (DSP developer)](https://steffensennert.de/teile-elektronik-teil2-revolo/), [Reverb.com listing](https://reverb.com/en-pl/item/93647465-teile-elektronik-revolo-dj-reverb-and-tremolo-effect).

**Sonic character.** A large, airy reverb that "sits above" the mix rather than clouding it, with a tremolo placed after the reverb so the tail pulses. Reviewers describe the result as rhythmic pumping, like the reverb being sidechained to a kick. The noise generator adds a wash that can be played on its own or into the reverb. The DSP is a port of a Valdemar Orn reverb algorithm, with a lighter "ECO" variant to fit a microcontroller, tuned to stay lush without muddying the mix (Sennert).

**Controls** (8 knobs, 2 switches, 2 arcade buttons per Sinesquares; knob names per the retailer text):

| Control | Function |
|---|---|
| REVERB | Wet level of the reverb |
| CHARACTER | Early reflections / room size; thickens the sound (Sinesquares) |
| LENGTH | Decay time |
| AIR | High-frequency tone of the tail |
| TREMOLO | Depth |
| TEMPO | Tremolo rate, reported as roughly 0.2 Hz to 40 Hz; overridden by TAP |
| NOISE | White-noise amount fed into the reverb |
| FILTER | Low-cut on the wet path, reported 20 Hz to 20 kHz |
| MIX / WET switch | Inline (dry + wet) versus wet-only for send/return |
| Bypass mode switch | Latching versus momentary |
| ON/OFF + TAP buttons | Bypass, and tap tempo (there is no sync input) |

Also: stereo in/out, and a send/return jack that replaces the internal reverb with an external spring tank.

**Signal flow** (as described by the Sinesquares review, confirmed in outline by the maker):

```
stereo in -> noise generator (added) -> low-cut filter (wet only) -> reverb (Volume, Character, Length, Air)
          -> tremolo (Depth, Tempo/Tap) -> out
dry path stays analogue; only the wet path is digitised (24-bit AD/DA)
```

The tremolo hits the reverb and the noise together, like a gater (per a [DJ demo on TikTok](https://www.tiktok.com/@sounds_market/video/7572151017052130582)). The noise passes through the filter before the reverb, so the filter also shapes the riser colour.

**Why DJs like it for tension.**

- Three independent "ride" axes in the wet path: noise level, tremolo depth/speed, and filter. A build is just turning them up in sequence.
- Tremolo after the reverb turns a static wash into a pulse, which implies motion and forward drive without adding any new rhythmic source.
- The wet path is filtered, so the wash does not fight the bass and kick.
- Momentary bypass and wet-only mode make it a "throw" effect: hit it on the last bar, cut it on the one.
- The reviewer found playing it by hand faster and more musical than automating it, and used noise to build into a chorus.

### 2.2 TEILE TEIL1 "the keinedelay"

A stereo tape-style delay for DJs; an early model was reportedly called KD7, a nod to the Boss DD-7 (seen in a search summary; not confirmed on a page I fetched). Sources: [TEILE](https://teile.life/products/teil1), [Perfect Circuit listing text via search](https://www.perfectcircuit.com/teile-elektronik-teil1-keinedelay.html), [MATRIXSYNTH](https://www.matrixsynth.com/2018/10/teil1-keinedelay-playable-delay.html).

**Sonic character.** A tape-delay emulation where the TIME control behaves like tape speed. Switching delay-time steps while repeats are still in the loop re-times and re-pitches those repeats, "as if you were changing the speed rapidly on a tape delay" (maker's wording). That is the effect the maker advertises for rhythmic patterns. In ANALOG mode a high-cut filter and mild wobble make repeats darker and less stable.

**Controls.**

| Control | Function |
|---|---|
| TIME | Delay time/tape speed within the selected range |
| Time range switch (rotary) | Steps from 3200 ms down to 400 ms; each step halves or doubles the time. Also FAST, FULL and LOOP positions |
| FEEDBACK | Repeat count; in LOOP mode it sets grain size |
| FX / MIX | Wet amount |
| SPEED | Listed on retail pages as one of the four knobs; not described by the maker |
| ANALOG / DIGITAL | ANALOG adds a high-cut and mild tempo/pitch modulation |
| MIX / WET | Wet-only output for mixer returns |
| MOM | Audio enters the delay loop only while the button is held |
| TAP, send, ONF buttons | Tap tempo, momentary FX send, output mute |

**Modes.**

- Stepping the time range changes delay time by 2x or 0.5x, and the repeats still circulating are sped up or slowed down with it. Switching into FAST, FULL or LOOP clears the feedback path.
- LOOP: records up to 12 seconds, then lets you change loop speed with TIME (an "old school time stretching algorithm", so it is not pure varispeed) and grain size with FEEDBACK. This is a freeze/granular-style tool.

**Signal flow.** Dry stays analogue; only repeats are digital (24-bit). With MIX/WET set to wet-only the box returns repeats alone, so a mixer FX return does not double the dry.

**Why DJs like it for tension.**

- The MOM send is how you "throw" a word, snare or hat into the loop and then let it ring, which is the core dub move. The ONF mute is the cut.
- The pitch/time step is a built-in riser or falling-tape move: stepping the time range during a tail raises or lowers the repeats in pitch and rate.
- Wet-only return plus filtered/dark ANALOG repeats keep the bass clear.
- LOOP freeze with grain control gives a sustained texture that can be stuttered or stretched.
- Caveat for deckforge: the pitch bend on time change is a feature on the Keinedelay, but the user explicitly dislikes the "goofy" version. So deckforge should offer it as an optional, deliberate gesture, not as the default time-change behaviour (see section 3.5).

## 3. Catalogue of tension-building effects

For each: what it does, typical controls, and how it is used. Sources are given where I read them; items marked "(general)" are standard practice summarised from the listed references rather than a single quote.

### 3.1 Tremolo + reverb (Revolo type)

- **What:** Reverb followed by amplitude modulation, so the tail pulses in time. At slow, synced rates it is a sidechain-style pump; at fast rates it is a rhythmic gate or shimmer; with noise in the reverb it becomes a pulsing riser (Revolo, section 2.1).
- **Controls:** reverb size/decay, tone, low-cut, tremolo depth, tremolo rate (free, tap or beat-synced), LFO shape (sine, square, ramp), optional noise level.
- **Use:** raise depth and rate across a build; filter the wet so it stays above the bass; pull everything back on the drop. Serato's Tremolo and Transform, Pioneer's Trans, and djay's Tremolos in Crossfader FX are related ([Serato FX list](https://support.serato.com/hc/en-us/articles/10271055029263-DJ-FX-Channel-FX-list), [Pioneer DJM-A9 effect descriptions via search](https://www.manualslib.com/manual/3001872/Pioneer-Dj-Djm-A9.html?page=52), [djay Crossfader FX](https://help.algoriddim.com/user-manual/djay-pro-mac/dj-tools/effects/crossfader-fx)).

### 3.2 Gated reverb

- **What:** A large reverb cut abruptly by a gate, so only the first part of the tail is heard. Classic 1980s drum sound ([Wikipedia](https://en.wikipedia.org/wiki/Gated_reverb)). Typical hold is around half a second followed by a fast release.
- **Controls:** threshold or trigger, hold, release, decay length, pre-delay.
- **DJ use:** a gated tail on a snare or clap gives a big impact without lingering wash. On a DJ deck the key can be a beat-grid trigger rather than an input level.

### 3.3 Rhythmic, beat-synced reverb gating

- **What:** The reverb send or return is chopped on a beat division (1/8, 1/16, triplets). Similar in spirit to Pioneer's Trans, which cuts the signal cyclically on a beat fraction (1/16 to 16 beats), and Traktor's Gater (rate off to 1/32, shape, mute) ([Pioneer DJM-A9 effect descriptions](https://www.manualslib.com/manual/3001872/Pioneer-Dj-Djm-A9.html?page=52), [Traktor effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference)).
- **Controls:** division, gate shape (hold/decay), depth, optional swing/offset.
- **Use:** speed the division up through a build (1/4, 1/8, 1/16, 1/32); this is the "accelerating tremolo" idea in Revolo form.

### 3.4 Reverse reverb swells

- **What:** Reverb tail rises into a downbeat instead of decaying from it. Made by reversing the source, reverbing it, reversing the result, then aligning it so the peak lands on the hit ([Kern Audio](https://kernaudio.io/guides/reverb/reverse-reverb), the 2026 [VAS Reverse Reverb write-up](https://bedroomproducersblog.com/2026/09/13/vas-reverse-reverb/)).
- **Controls:** swell length (1, 2, 4, 8 bars is typical), source, pre-delay, mirror/symmetry.
- **DJ constraint:** it normally needs the audio ahead of the playhead. On a mixer's send/return you cannot do it live; you must pre-render. A DJ app that already holds both decks in memory can do this live because it can read ahead (see proposal 3.5.4). Guidance on timing: finish the swell exactly on the downbeat, in 2, 4 or 8 bar phrases; take something away one or two bars before.

### 3.5 Shimmer

- **What:** Reverb with pitch shifting in the feedback loop, typically +12 semitones, so each pass climbs and the tail glows upward. In ValhallaShimmer, the Shift control has an effect only when Feedback is above zero; +12 with feedback 0.5 or more gives the classic sound ([Valhalla DSP](https://valhalladsp.com/2010/11/27/valhallashimmer-the-controls/), [Valhalla product page](https://valhalladsp.com/shop/reverb/valhalla-shimmer/)).
- **Controls:** shift amount (semitones), feedback, mix, damping, mode (single up, dual up/down, reverse).
- **Use:** automate feedback and mix up over a build, band-limit the send and return, put a limiter after it. A rising pitch is a standard tension device ([Point Blank](https://www.pointblankmusicschool.com/blog/designing-unique-risers-and-fx-for-transitions-to-level-up-your-tracks/)).

### 3.6 Multi-tap and rhythmic delays with per-tap filters

- **What:** Several taps at different beat divisions (for example 1/4, dotted 1/8, 1/16), each with its own level, pan and filter, giving a rhythm rather than a single echo. Pioneer's Ping Pong and Traktor's Delay (with FILTER, FEEDB, RATE 1/32 to 4/4, SPRD stereo offset) are the single-tap relatives ([Traktor effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference)).
- **Controls:** per-tap time (beat division), level, pan, high-pass/low-pass, plus global feedback.
- **Use:** darker, lower taps behind brighter, higher ones create depth; dotted values add rhythmic tension ([Klementz](https://www.klementz.fr/en/dj-delay-for-live-mixing-the-right-technique/) covers tempo-synced delay choices).

### 3.7 Granular and freeze delays

- **What:** Capture a short buffer and loop or stretch it at a chosen grain size; freeze holds the buffer indefinitely. Traktor's Delay, Tape Delay, Reverb and Iceverb all have a FRZ control; Reverse Grain captures a loop and plays it backwards; Transpose Stretch has a grain size from 333 ms to 5 ms ([Traktor effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference)). The Keinedelay's LOOP mode does a related job (section 2.2).
- **Controls:** capture/freeze, grain size, playback speed or pitch, position/rotation, stretch.
- **Use:** freeze a bar on the last beat of a phrase and play with pitch and grain; release on the drop.

### 3.8 Pitch-shifted feedback delays, and when pitch is wanted

- **Two separate things get called "pitch":**
  1. **Repitch (varispeed) on delay-time change.** Changing the read position while audio is flowing changes the playback rate, so repeats bend in pitch. Ableton calls this Repitch and describes it as tape-like; the alternatives are Fade (crossfade between two delay times, no pitch bend but smearing or granular character on rapid changes) and Jump (immediate, clicks) ([Ableton manual](https://www.ableton.com/en/manual/live-audio-effect-reference/), [Gearspace thread](https://gearspace.com/board/electronic-music-instruments-and-electronic-music-production/998508-delays-repitch-jump-fade-abletonlive.html)). The Keinedelay and Traktor's Tape Delay and Ramp Delay lean into this on purpose.
  2. **Deliberate pitch shifting in the feedback path** (up or down a few semitones, or an octave in shimmer). This is a designed riser or sinking-tape effect.
- **When pitch is desirable:** as a build gesture you trigger and control (rising pitch over 4 to 8 bars, a "tape stop" release, a Spiral-up). DJ TechTools builds a pitch rise this way with a high-feedback delay in Repitch mode, automating delay time during the build ([DJ TechTools](https://djtechtools.com/2014/01/28/how-to-create-a-pitch-build-effect-in-ableton/)).
- **When pitch is undesirable:** when the DJ just wants a different echo length. Changing the beat division with a gliding read head sweeps the pitch of the existing tail. The fix is the crossfade approach (two read taps, equal-power crossfade) or clearing and restarting the tail, optionally with a short duck.

### 3.9 Stutter, beat-repeat and loop-roll builds

- **What:** Capture a beat fraction and repeat it, shrinking the fraction as the build proceeds. Pioneer Roll records the audio at the moment the effect turns on and repeats it per the beat fraction; Triplet Roll and Helix are variants ([Pioneer DJM-A9 list via search](https://www.manualslib.com/manual/3001872/Pioneer-Dj-Djm-A9.html?page=52)). Traktor Beatmasher 2 samples a bar and has gate, rotation (1/8 steps), length and warp; BeatSlicer re-orders slices; Bouncer repeats one-beat snippets at adjustable speed and pitch ([Traktor effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference)). One NI suggestion is to chain Beatmasher with Transpose Stretch and Reverb ([NI blog via search](https://blog.native-instruments.com/best-dj-effects/)).
- **Controls:** fraction, repeat count or length, gate/decay, pitch ramp.
- **Use:** halve the fraction each bar (1/2, 1/4, 1/8, 1/16), optionally pitch up, then cut on the one.

### 3.10 Riser and noise generators

- **What:** Synthesised noise or tone swept upward in filter and level, often with a reverb. Pioneer's RMX-1000 Build Up Scene FX (BPF Echo, Echo, Noise, Spiral Up, Reverb Up) and Break Down FX (HPF Echo, LPF Echo, Crush Echo, Spiral Down, Reverb Down) are the clearest hardware example; one big knob scales intensity and two further knobs tweak parameters, working like a macro ([MusicRadar](https://www.musicradar.com/reviews/tech/pioneer-rmx-1000-remix-station-561298), [DJWorx](https://djworx.com/review-pioneer-rmx-1000-remix-station/)). Serato offers Space and Noise channel FX ([Serato FX list](https://support.serato.com/hc/en-us/articles/10271055029263-DJ-FX-Channel-FX-list)); djay Crossfader FX has Sweeps and Risers ([djay manual](https://help.algoriddim.com/user-manual/djay-pro-mac/dj-tools/effects/crossfader-fx)).
- **Controls:** level, filter centre/Q, filter sweep curve, optional pitch.
- **Use:** open a high-pass or band-pass sweep with the level over 8 to 16 bars; cut hard on the drop.

### 3.11 One-knob "build" macros

- **What:** A single control (or a timed ramp) moves several parameters together.
  - **Pioneer RMX-1000 Scene FX:** one large knob per Build Up or Break Down effect (above). Release FX (Vinyl Brake, Echo, Back Spin) ends the effect with a lever whose depth sets the fade speed ([MusicRadar](https://www.musicradar.com/reviews/tech/pioneer-rmx-1000-remix-station-561298)).
  - **Serato:** Multi FX mode gives one knob per effect and lets you chain them; Serato's own tip chains Version Echo, Reverb and a HPF as a riser ([Serato Multi FX](https://support.serato.com/hc/en-us/articles/202304824-How-to-use-Multi-FX-in-Serato-DJ)). I could not find a Serato feature literally named "One Knob FX", so treat that name as a loose description of Multi FX and the Hex FX "Hex" dial. Hex FX is a DAW plugin whose single dial changes parameters across the whole chain (delay feedback, reverb tail and so on) and can be customised per preset ([Serato Hex FX](https://serato.com/hex-fx), [MusicTech](https://musictech.com/news/gear/serato-hex-fx-plugin-first-update/)).
  - **djay:** Crossfader FX applies chosen effects (Echos, Filters, Tremolos, Sweeps, Risers, Neural Mix) across a transition whose length is set in bars; the crossfader or a Transition button drives it. Neural Mix can put the effect on one stem only, for example just the drums ([djay manual](https://help.algoriddim.com/user-manual/djay-pro-mac/dj-tools/effects/crossfader-fx), [djay Pro 5 release](https://www.algoriddim.com/press_releases/447-algoriddim-unveils-djay-pro-5-with-next-generation-neural-mix-crossfader-fusion-and-fluid-beatgrid-)). djay "Neural Mix FX" as such is a stem feature; the build behaviour comes from Crossfader FX.
  - **Traktor:** Group Mode on each effect maps one knob to its most useful parameter; Ramp Delay glides between delay rates over 1/4 to 16 bars ([Traktor effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference)).
- **Common pattern:** monotonic ramp up (filter, level, feedback, rate), a hard release, optional tail, always phrase-aligned.

### 3.12 Dub and feedback technique (the human side)

- Feedback loops where the repeats are filtered and fed back to build a sustained "carpet" are the DJ dub staple; a safety filter and limiter protect the main out ([DJ TechTools dub delay](https://djtechtools.com/2011/04/25/ableton-tips-how-to-create-a-dub-delay/)).
- Push feedback to build tension, then cut the send or the source. The tail keeps ringing and you regain control ([Klementz](https://www.klementz.fr/en/dj-delay-for-live-mixing-the-right-technique/)). Filter the return (cut lows, tame highs) so the repeats do not pile up in the low-mids. Too much feedback masks snare, vocals and bass.

## 4. What deckforge already has (for context)

From `crates/common/src/lib.rs` and `crates/dsp/src/fx/`:

- Beat FX unit with kinds Delay, Echo, UpEcho, DownEcho, PingPong, Spiral, Reverb, Flanger, Phaser, Roll, Trans, FilterSweep; beat divisions 1/8 to 16 beats; percent-driven Reverb; shared building blocks `FdnReverb`, `PitchShifter`, `StereoRing`, `SwapGate`, `OnePole`, `Svf`, `Lfo`, all allocated up front.
- Color FX with Filter, Space, DubEcho, Crush, Noise.
- Engine has both decks and the beatgrid in memory (phrase-anchored bar lines were added recently), which is what makes beat-locked builds and look-ahead reverse swells possible.

**Likely source of the "goofy" pitch the user dislikes.** In `beat.rs` the echo's delay time follows its target by a slew-limited glide (`DELAY_SLEW` = 0.35 samples per sample). A moving read head at that slew means a playback rate of 0.65 to 1.35, roughly -7 to +5 semitones during the move, so the existing tail bends audibly whenever the beat division changes. That is exactly the Repitch behaviour described in 3.8. The proposals below therefore separate "retime" (clean) from "bend" (deliberate).

## 5. Design proposals

Shared rules for all five: tempo is read from the master clock; every time value is a beat division (including dotted and triplet) unless a free range is noted; all parameters are smoothed; output level is compensated so a full build stays within about 1.5 dB of the dry level (deckforge already enforces this for the echo family); every effect has a momentary trigger and a "release" that fades the tail over a chosen number of beats. Names are working names.

### 5.1 "Pulse Verb": tremolo-reverb (Revolo-inspired)

**Concept.** A reverb with a noise source and a beat-locked tremolo/gate after it, so the wash pulses and can accelerate. Original structure; it reuses `FdnReverb`.

**Signal flow.**

```
send (deck/master, post-fader)
  -> + noise (white/pink, level N)
  -> wet high-pass (HP) -> pre-delay (PD)
  -> FdnReverb (size/decay LEN, damping AIR, early density CHAR)
  -> tremolo VCA (depth D, rate RT, shape SH, phase PH, beat-locked)
  -> wet low-pass (optional) -> wet level -> mix (inline) or wet-only (send)
dry path untouched
```

**Parameters.**

| Param | Range | Default | Notes |
|---|---|---|---|
| Reverb level | 0 to 100 % | 40 % | Wet amount |
| Length (LEN) | 0.8 s to 12 s (log) | 4 s | Same scale as existing Reverb percent mapping |
| Character (CHAR) | 0 to 100 % | 50 % | Early density and size |
| Air (AIR) | 0 to 100 % | 60 % | High-frequency damping, inverted |
| Noise (N) | -inf to -18 dB | off | Level into the reverb; noise goes through HP so it follows the sweep |
| Wet high-pass (HP) | 40 Hz to 2 kHz (log) | 250 Hz | Keeps the wash off the bass |
| Depth (D) | 0 to 100 % | 0 % | 100 % = full gate |
| Rate (RT) | 1/2, 1/4, 1/4T, 1/8, 1/8T, 1/16, 1/16T, 1/32 beats, or Free 0.2 to 40 Hz | 1/8 | Locked to bar phase when synced |
| Shape (SH) | Sine, Triangle, Square-soft, Ramp-down ("duck") | Ramp-down | Ramp-down gives the sidechain-pump feel |
| Phase (PH) | 0 to 100 % of cycle | 0 | Aligns the dip to the kick when shape is Ramp-down |
| Sync | Beat / Tap / Free | Beat | Deckforge always has the grid, so no tap needed except for free-tempo material |

**Beat-sync.** The LFO phase is derived from the master beat position (`phase = frac(beat_pos / rate_in_beats) + PH`), so it re-locks after any seek or sync. Rate changes are quantised to the next grid line to avoid phase jumps; Free mode is not synced.

**Defaults and safe use.** Depth starts at 0 so engaging the effect is just a reverb. Noise is off. The wet high-pass is above the bass.

**Performance.** (a) Engage on the last bar of a phrase with Depth about 40 %, Rate 1/8; ride Depth up to 100 % and step Rate 1/8 -> 1/16 -> 1/32 over 16 bars; ride Noise in over the last 4 bars; cut on the downbeat and let the tail ring for one bar (release). (b) For a subtle groove: Rate 1/4, Ramp-down shape, Depth 30 %, so the reverb ducks on each kick.

### 5.2 "Dub Tape": deep, filterable delay

**Concept.** A tape-flavoured dub delay with filters inside the feedback loop, a momentary "throw" send, an explicit choice of how time changes are handled, and a freeze. Replaces gliding repitch as the default.

**Signal flow.**

```
send (throw gate T) -> input HP/LP (optional)
  -> delay line (StereoRing), read by two taps A/B (crossfade on time change)
  -> feedback path: HP (FHP) -> LP/band (FLP) -> soft saturation (tape drive) -> wow/flutter LFO -> back to input
  -> output filter (SVF: LP/BP/HP morph) -> wet level
  -> (optional) ping-pong width -> out
```

**Parameters.**

| Param | Range | Default | Notes |
|---|---|---|---|
| Time | 1/16, 1/8, 3/16 (dotted 1/8), 1/4, 3/8, 1/2, 3/4, 1, 2, 4 beats (+T variants) | 3/4 beat | Dotted default gives forward lean |
| Feedback | 0 to 108 % | 45 % | Above 100 % enters soft-clipped self-oscillation; "deep" zone |
| Tone: Feedback HP | 20 Hz to 1.5 kHz | 120 Hz | Stops bass buildup |
| Tone: Feedback LP | 800 Hz to 12 kHz | 4 kHz | Each repeat darker, like tape |
| Drive | 0 to 100 % | 25 % | Tape saturation in the loop; level-compensated |
| Wobble | 0 to 100 % | 15 % | Wow/flutter depth; up to about +-10 cents so it never reads as a pitch glitch |
| Time-change mode | Clean (crossfade), Tape (repitch), Reset (clear and restart) | Clean | See below |
| Throw | momentary / latch | momentary | Audio enters only while held |
| Freeze | button | off | Feedback fixed at 100 %, input closed; the loop is held |
| Width | 0 to 100 % | 30 % | Ping-pong amount |
| Wet level | 0 to 100 % | 40 % | |

**Time-change behaviour.**

- **Clean:** two read taps. On a time change tap B ramps to the new time (instantly, in the background) while a 30 to 80 ms equal-power crossfade moves the output from A to B. Pitch of the existing tail does not change; the repeats are just re-spaced. This is the answer to the pitch artefact in section 3.8.
- **Tape:** the read head glides (as Echo does today). Opt-in, for a dub "swoop".
- **Reset:** dip the wet path, clear the loop, restart at the new time (existing `SwapGate` behaviour).
- A visible glide-time control (0.05 to 2 beats) applies to Tape mode only.

**Beat-sync.** Time is a beat division converted to samples from the master tempo; tempo changes ramp the delay using the selected time-change mode.

**Performance.** (a) Throw the last word or snare with the send, push Feedback past 90 % while sweeping Feedback HP up so the carpet thins as it rises, then drop Throw and mute the source; the repeats carry the transition. (b) Hold Freeze on the last beat before a break, ride the output filter from LP to BP to HP across 4 bars. (c) Optional deliberate rise: switch to Tape and push Time down one division per bar for 4 bars.

**Safety.** A soft limiter or tanh stage after the saturator prevents runaway; a "panic" release (hold the send closed and fade feedback to 0 over 1 beat) is always one press away.

### 5.3 "Ascend": build/riser macro

**Concept.** One control (or an auto-ramp over N bars) that moves a whole chain from calm to peak, then a one-press release. Modelled on the common "single knob scales several things" idea (RMX-1000 Scene FX, Serato Multi FX chains, djay Crossfader FX bar-length ramps) without copying any of their implementations.

**Signal flow.**

```
send (master or deck) -> [Riser source: noise + optional tone] ---+
                      -> HPF sweep ---------------------------------+-> Dub Tape (feedback up, time tightens)
                      -> Pulse Verb (send up, tremolo accelerates) -> Halo (optional shimmer)
                      -> macro-driven dry duck (low cut on dry) -> master
```

Everything is built from the other proposals so there is only one DSP chain to maintain.

**Macro.** One value `M` from 0 to 1, either hand-driven (knob/fader) or auto-driven by a timeline of 4, 8, 16 or 32 bars (default 16). Each parameter follows its own curve of `M`:

| Target | At M = 0 | At M = 1 | Curve |
|---|---|---|---|
| Master high-pass on the whole mix | 20 Hz (off) | about 600 Hz | exponential in frequency |
| Dry-path low cut (optional "thin out") | off | -12 dB below 150 Hz | linear |
| Noise riser level | off | -14 dB | exponential |
| Noise band-pass centre | 400 Hz | 7 kHz | log |
| Delay feedback | 40 % | 92 % | ease-in |
| Delay division | 1 beat | 1/8 beat, stepping per bar in Clean mode | stepped, quantised |
| Reverb send | 10 % | 70 % | ease-in |
| Tremolo depth / rate | 0 %, 1/4 | 80 %, 1/32 | depth linear, rate stepped |
| Shimmer mix (if Halo on) | 0 | 35 % | ease-in |

**Beat and phrase behaviour.**

- Auto mode aligns its end to the next phrase boundary (the engine already draws phrase-anchored bar lines): "start now, peak on bar N". If fewer bars remain than selected, it compresses to fit or asks for a shorter length.
- Rate and delay-division steps happen on bar lines only, never mid-bar.
- Drop: a Release button (and a Release-on-downbeat option) jumps `M` to 0 in one beat, kills the noise instantly, lets the reverb and delay tails decay naturally (tail length option 0 to 2 bars), and clears the macro.
- Optional pre-drop gap: a "silence" option drops all wet, dry duck included, for the last 1/16 or 1/8 beat before the downbeat. This is a very strong tension device (see [Veena Studio](https://veena.studio/blog/transitions-and-fills) on silence); leave it off by default.

**Parameters exposed.** Macro `M` (0 to 1, hand), Length (4/8/16/32 bars, default 16), Mode (Hand / Auto), Land on (Phrase end / Next bar / Now+Length), Tail on release (0 to 2 bars, default 1), Intensity (scales all curves, 0 to 100 %, default 70 %), Noise on/off, Shimmer on/off, Pre-drop gap (off/1/16/1/8).

**Performance.** (a) Auto: pick 16 bars, press Ascend at the start of the last 16 bars of the outgoing track's phrase, mix in the new track underneath, press Release on the downbeat. (b) Hand: map the macro to a knob; ride it manually, press Release to drop. (c) For short transitions use 4 bars.

**Safety.** The macro-owned master high-pass and delay feedback have hard caps. Total output is level-compensated so the peak does not exceed the pre-engage level by more than 1.5 dB.

### 5.4 "Backwash": reverse reverb swell using look-ahead

**Concept.** Because deckforge can read the incoming track ahead of its playhead, it can make a true reverse-reverb swell that ends exactly on the incoming downbeat. This is not possible on hardware FX units, which only see audio as it passes.

**Signal flow (preview mode, preferred).**

```
read ahead N beats of incoming deck B (the audio that is about to play after the cue)
  -> reverse -> FdnReverb (offline / faster than real time, no dry) -> reverse
  -> tail-shaped envelope (fade-in, optional low-pass opening) -> swell buffer
schedule: play swell so its peak lands on B's first beat after cue
```

Fallback (no look-ahead available, e.g. live input): capture the last N beats of the master, reverse it, reverb it, and play the result at a chosen time as a pre-roll riser (a "reverse echo").

**Parameters.** Length 1, 2, 4 or 8 bars (default 2); Source (Incoming deck first beats / Outgoing last beats / Both); Reverb length 2 to 10 s (default 5 s); Pre-delay 0 to 200 ms; HP of swell 80 Hz to 2 kHz (default 300 Hz); Mirror (off / on: swell rises then breathes out); Level -18 to 0 dB (default -8 dB); Land on (Cue / Next downbeat).

**Beat-sync.** The swell start time is computed from tempo and the landing downbeat, so the start is quantised to a beat; the engine arms it and fires it when the playhead reaches `landing - length`. Rendering the swell happens off the audio thread when armed; the audio thread only plays a prepared buffer.

**Performance.** Cue the incoming track at its first big hit, arm Backwash with 2 bars, start mixing; the swell fades in under the outgoing track and the incoming hit lands on its peak. Pair with Ascend's release for a drop.

### 5.5 "Halo": shimmer / freeze wash (supporting effect)

**Concept.** A reverb with pitch-shifted feedback, plus a freeze, to give a rising or sustained glow behind builds. Reuses `PitchShifter` and `FdnReverb`.

**Signal flow.**

```
send -> pre-delay -> FdnReverb(feedback path contains PitchShifter ratio R, LP damping)
     -> wet HP/LP -> optional tremolo (shares Pulse Verb's LFO) -> out
Freeze: input closed, reverb feedback held near 100 % while pitch shifting is bypassed (prevents runaway)
```

**Parameters.**

| Param | Range | Default | Notes |
|---|---|---|---|
| Shift | -12 to +12 semitones, snap-to presets (+12, +7, +5, 0, -12) | +12 | Applies only when feedback greater than 0 (same rule as the reference shimmer) |
| Feedback | 0 to 90 % | 40 % | Hard cap to protect against instability |
| Length | 1 to 12 s | 5 s | |
| Damping | 0 to 100 % | 60 % | Keeps repeats from piling in the highs |
| Mix | 0 to 100 % | 30 % | |
| Freeze | button | off | |
| Mode | Single / Dual (up and down) | Single | |

**Beat-sync.** Not inherently beat-locked; Freeze and release snap to the grid; shift changes quantise to the next beat to avoid a pitch glitch mid-phrase.

**Performance.** Engage with Feedback at 20 %, ride it to 70 % across the last 8 bars while Mix rises; hit Freeze on the last beat before the drop and release on the one.

### 5.6 Suggested build order and cheapest wins

1. **Dub Tape Clean mode** as a fix to Echo/UpEcho/DownEcho time changes: it directly addresses the pitch artefact (section 4).
2. **Pulse Verb**: a small addition to the existing reverb path (a beat-locked VCA plus a noise source).
3. **Ascend macro** wired from existing and new parts.
4. **Halo** (reuses the pitch shifter) and **Backwash** (needs look-ahead plumbing and an offline reverse-render path).

Open questions to settle before building: where in the mixer the send/return sits (per deck versus master), the preferred controller mapping for Release and Throw on the DDJ-FLX4 (which has a Beat FX section and Sound Color FX knobs), and whether Ascend should replace the Color FX knob mapping while engaged.

## 6. Sources

Company and products:

- TEILE, [TEIL2 Revolo product page](https://teile.life/products/teil2-reverb) and [TEIL1 the keinedelay product page](https://teile.life/products/teil1).
- Sinesquares, [TEILE Revolo review](https://www.sinesquares.net/musicgear/teile-revolo-review).
- Steffen Sennert, [TEIL2 Revolo firmware and DSP](https://steffensennert.de/teile-elektronik-teil2-revolo/).
- Reverb.com, [Revolo listing](https://reverb.com/en-pl/item/93647465-teile-elektronik-revolo-dj-reverb-and-tremolo-effect); Perfect Circuit, [Keinedelay listing](https://www.perfectcircuit.com/teile-elektronik-teil1-keinedelay.html).
- MATRIXSYNTH, [Teil1 Keinedelay](https://www.matrixsynth.com/2018/10/teil1-keinedelay-playable-delay.html); Ivan Minuti, [Keinedelay](http://ivanminuti.com/journal/music/teil1-keinedelay-ckdlc).
- Sounds Market, [Revolo on Xone:92 demo (TikTok)](https://www.tiktok.com/@sounds_market/video/7572151017052130582).
- TEILE, [Revolo manual PDF](https://cdn.shopify.com/s/files/1/0949/7483/7078/files/TEIL2-Revolo-Manual.pdf) (poster, not text-extractable).

Techniques and DJ software:

- Native Instruments, [Traktor effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference) and [best DJ effects](https://blog.native-instruments.com/best-dj-effects/).
- Serato, [DJ-FX and Channel FX list](https://support.serato.com/hc/en-us/articles/10271055029263-DJ-FX-Channel-FX-list), [Multi FX](https://support.serato.com/hc/en-us/articles/202304824-How-to-use-Multi-FX-in-Serato-DJ), [Hex FX](https://serato.com/hex-fx), [MusicTech on Hex FX](https://musictech.com/news/gear/serato-hex-fx-plugin-first-update/).
- Algoriddim, [Crossfader FX manual](https://help.algoriddim.com/user-manual/djay-pro-mac/dj-tools/effects/crossfader-fx) and [djay Pro 5 release](https://www.algoriddim.com/press_releases/447-algoriddim-unveils-djay-pro-5-with-next-generation-neural-mix-crossfader-fusion-and-fluid-beatgrid-).
- Pioneer DJ, [DJM-A9 manual: Beat FX types](https://www.manualslib.com/manual/3001872/Pioneer-Dj-Djm-A9.html?page=52); [MusicRadar RMX-1000 review](https://www.musicradar.com/reviews/tech/pioneer-rmx-1000-remix-station-561298); [DJWorx RMX-1000 review](https://djworx.com/review-pioneer-rmx-1000-remix-station/).
- Ableton, [Live audio effect reference](https://www.ableton.com/en/manual/live-audio-effect-reference/); [Gearspace on repitch/jump/fade](https://gearspace.com/board/electronic-music-instruments-and-electronic-music-production/998508-delays-repitch-jump-fade-abletonlive.html).
- DJ TechTools, [dub delay in Ableton](https://djtechtools.com/2011/04/25/ableton-tips-how-to-create-a-dub-delay/) and [pitch build effect](https://djtechtools.com/2014/01/28/how-to-create-a-pitch-build-effect-in-ableton/).
- Klementz, [DJ delay for live mixing](https://www.klementz.fr/en/dj-delay-for-live-mixing-the-right-technique/).
- Valhalla DSP, [ValhallaShimmer controls](https://valhalladsp.com/2010/11/27/valhallashimmer-the-controls/) and [product page](https://valhalladsp.com/shop/reverb/valhalla-shimmer/).
- Wikipedia, [Gated reverb](https://en.wikipedia.org/wiki/Gated_reverb).
- Kern Audio, [reverse reverb](https://kernaudio.io/guides/reverb/reverse-reverb); Bedroom Producers Blog, [VAS Reverse Reverb](https://bedroomproducersblog.com/2026/09/13/vas-reverse-reverb/); Point Blank, [risers and transition FX](https://www.pointblankmusicschool.com/blog/designing-unique-risers-and-fx-for-transitions-to-level-up-your-tracks/); Veena Studio, [transitions and fills](https://veena.studio/blog/transitions-and-fills).

Several pages above (Pioneer manual text, Perfect Circuit, Reverb.com, NI blog, Kern Audio, Bedroom Producers Blog) were read through search-result summaries rather than a full fetch, so wording-level details should be re-checked against the originals before relying on them.
