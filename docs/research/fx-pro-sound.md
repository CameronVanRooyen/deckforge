# Professional FX sound: Pioneer/rekordbox behaviour, pro references and DSP recipes

Research brief for deckforge. Covers Echo, Space, Reverb, Noise and the rest of the Beat FX and Sound Color FX set.
Complements `fx-tension.md` (tension/build effect proposals) and `fx-flx4-control.md` (FLX4 control scheme); this document is about *how the existing effects should sound and behave*.

Evidence labels: **[Manual]** official manual text, **[Review]** press or reviewer, **[Forum]** user or support post, **[Ref]** DSP paper or vendor design note, **[Inf]** my inference or rule of thumb (not sourced). Numbers marked [Inf] are starting points to be tuned by ear.

## 0. Summary of findings

1. **Pitch bend on beat-division change is a deliberate tape-style trait that Pioneer only ships where it wants it.** The DJM-900NXS2 manual says pitch changes with delay time for **Spiral** (and Helix); it says nothing of the sort for Echo, Delay or Ping Pong [Manual]. The DJM-A9 added a tape-style Echo that pitches when you change timing, and A9 owners complain about the "weird pitching" and "spiralling" when jumping between divisions [Review/Forum]. The user's request (pitchless 1/4, 1/2, 3/4, 1) therefore matches NXS2/rekordbox expectations. Cause in deckforge: `DELAY_SLEW = 0.35` samples per sample in `beat.rs` moves the read head at 0.65x or 1.35x speed, i.e. -7.5 / +5.2 semitones, for about one second on a 1/4 to 1 beat change at 128 BPM. Fix: dual-tap crossfade (section 4.1).
2. **Pioneer's Space is not a shimmer, but its colour knob band-limits the *wet* only** (CCW: mid+low, CW: mid+high) [Manual]. deckforge's Space also high-passes the *dry* signal up to 1.8 kHz and dips it 45 percent (`run_space`). That thins the low end of the track, which is exactly what the user does not want. Keep the dry path untouched, band-limit only the wet, and add a +12 semitone shimmer voice on the bright side.
3. **Professional-sounding wet signals share six traits**: high-passed wet (200-600 Hz), frequency-dependent decay (HF decays faster), slow decorrelated modulation, stereo decorrelation, ducking by the dry signal, and a nonlinearity or limiter in any feedback loop. Pioneer, Traktor and Ableton all expose at least the first two; the others are what separates "plug-in default" from "mix-ready".
4. **Pioneer reverb does not build tension by itself either**; it is a static send with a % size and an X-PAD filter [Manual]. DJs create tension by riding level, filter and cut. The tension has to be designed in: rising RT60, rising wet high-pass, beat-phased swell, and a hard cut at the downbeat (section 4.3, and `fx-tension.md`).
5. **FX FREQUENCY (LOW/MID/HI)** is the single most-recommended Pioneer technique for clean echo and reverb: turn the LOW band off so kick and bass stay dry [Review]. A band-limited wet path (LR4 split or wet HPF) delivers the same result.
6. **The Xone:96 has no built-in effects** (sends/returns only); its reputation rests on the state-variable filter with resonance and the CRUNCH pre-filter drive [Review]. Use it as the model for the Filter effects, not for echo/reverb.

## 1. deckforge today (relevant constants, for orientation only)

| Item | Current value | Source in repo |
|---|---|---|
| Echo delay-time change | slewed 0.35 samples/sample (Spiral 0.5) | `beat.rs: DELAY_SLEW`, `run_echo` |
| Echo feedback | 0.55 + 0.3-0.34 x level, ceiling 0.95 | `echo_style` |
| Echo loop filters | 1-pole LP 4.5 kHz, 1-pole HP 120 Hz | `echo_style`, `FEEDBACK_HP_HZ` |
| Echo dry duck | 0.34 x level (static, not signal-driven) | `echo_style` |
| Reverb | 12-line FDN, 2x4 allpass diffusers, 1-pole damping, 12 slow mods up to 1.6 ms, predelay | `reverb.rs` |
| Reverb % to RT60 | 1 s to 11.5 s exponential | `reverb_rt60_for_percent` |
| Space | RT60 3.5-9.5 s, wet HP 140-540 Hz, **dry HP to 1.8 kHz and dry dip 0.45** | `color.rs: run_space` |
| Dub Echo | fixed 0.75 beat, LP 3.8 kHz, wow 0.7 Hz / 0.25 ms | `run_dub_echo` |
| Noise | white noise via SVF, ref-RMS normalised, 3.5 s tail | `color.rs` |

The FDN skeleton (mutually prime delays 23-66 ms, Householder-style feedback, per-line damping, decorrelated stereo taps) is sound. The shortfalls are in what surrounds it: wet filtering, frequency-dependent decay shape, ducking, shimmer, and tension control.

## 2. How Pioneer DJM / rekordbox behaves

### 2.1 Signal flow

- DJM-900NXS2 block diagram: Trim, EQ and Sound Color FX sit before the channel fader; **Beat FX come after the channel fader** [Manual]. The manual states repeatedly that lowering the channel fader or cutting the input still leaves the echo, delay, reverb or Spiral tail audible. That is how the "echo out" works.
- Space and Dub Echo are reported as **post-fader** Sound Color FX (the others are pre-fader) [Forum] ([Pioneer forum thread](https://forums.pioneerdj.com/hc/en-us/community/posts/115000174526-Echo-effect-has-no-volume-fader-control) and rekordbox hardware docs). Same tail-survives-the-fader behaviour.
- On rekordbox the Release FX are post-fader and not audible in headphone cue [Forum].
- DJM-V10/A9 added a separate **Send FX** bus (Short Delay, Long Delay, Dub Echo, Reverb; parameters Size/Feedback, Time, Tone, Master Mix; post-fader, parallel by default, insertable) [Review]. Tone sweeps "dark and broody" to "light and airy". This is the closest hardware analogue of what deckforge's Space/Dub Echo should be.

### 2.2 Beat FX (DJM-900NXS2 manual; DJM-A9 and V10 differences noted)

Controls: BEAT arrows (parameter 1), TIME knob (2), LEVEL/DEPTH knob (3), X-PAD (4). The DDJ-FLX4 only has BEAT arrows and LEVEL/DEPTH; it has no TIME knob or X-PAD [Manual: FLX4], so division stepping is the *only* time control on the user's hardware.

| Effect | Manual description | BEAT | TIME | LEVEL/DEPTH | X-PAD |
|---|---|---|---|---|---|
| Delay | one repeat on the beat fraction | 1/16-16 beats | 1-4000 ms | dry/echo balance | sets delay time |
| Echo | repeats, gradually attenuated; tail continues with fader down | 1/16-16 beats | 1-4000 ms (older units: 1/8-16) | dry/echo balance; Pioneer reply says it is a wet/dry control [Forum] | delay time |
| Ping Pong | repeats alternate L/R with different timing, attenuated | 1/16-16 beats | 1-4000 ms | balance | delay time |
| Spiral | reverberation-like effect; **pitch changes when delay time is changed** | 1/16-16 beats | 10-4000 ms | feedback **and** balance | delay time |
| Reverb | reverberation | sets extent 1-100 % | 1-100 % | dry/effect balance | filter cutoff of the reverb output |
| Trans | gate: sound cut per beat fraction | 1/16-16 beats | 10-16000 ms | duty and balance | cut time |
| Filter | cutoff moves per beat fraction (LFO) | 1/16-64 beats | 10-32000 ms | amount (CCW = dry) | LFO speed micro-variation |
| Flanger / Phaser | one cycle per beat fraction | 1/16-64 beats | 10-32000 ms | amount (CCW = dry) | cycle micro-variation |
| Roll | records input at ON, loops it | 1/16-16 beats | 10-4000 ms | balance with the roll | effect time |
| Slip Roll | as Roll, **re-records when the time changes** | 1/16-16 beats | 10-4000 ms | balance | effect time |
| Pitch | pitch shift | -50 to +100 % | same | same (CCW = original) | same |
| Helix | loop sampler; **changing effect time changes the pitch of what was recorded**; LEVEL/DEPTH controls record/overlay ratio, fully right freezes | 1/16-16 beats | 10-4000 ms | overlay / record | pitch via time, springs back on release |
| Vinyl Brake | speed drops until stop | 1/16-16 beats | 10-4000 ms | speed | time |

Source: [DJM-900NXS2 manual, "Types of BEAT FX"](https://c3.zzounds.com/media/DJM-900NXS2_manual_EN-0703f22a06f23c5041e63da3db269aa6.pdf) (text extracted locally from the PDF), the [DJM-TOUR1 manual](https://manuals.plus/m/a7678589c8c0f1bb7105f35557cbd7b7ad8bc0a42537f5af1dfde2c1928531fc) for Pitch, and [DJM-A9 manual page 52](https://www.manualslib.com/manual/3001872/Pioneer-Dj-Djm-A9.html?page=52) (seen only via search excerpts).

Observations that matter for deckforge:

- Only Spiral, Helix, Pitch and Vinyl Brake are documented as pitch-changing. Echo, Delay, Ping Pong and Reverb are not.
- Pioneer Reverb's "percent" is a combined size/amount knob (1-100 %); the X-PAD does not trigger a bigger tail, it filters the reverb output. DJ TechTools describes using it for sweeps and accents on snares, claps and hats [Review: [DJ TechTools](https://djtechtools.com/amp/2016/04/04/3-techniques-pioneer-djm-900nxs2s-eq-effects/)].
- The older (DJM-500 era) mixers produced "audio chaos" when the delay time was turned while the effect was sounding, as the effect fell out of time [Forum: [Ableton forum thread on the DJM-500](https://forum.ableton.com/viewtopic.php?t=40210), via search summary]. Hence the later quantised-division design.
- DJM-V10 adds **Shimmer** to Beat FX (reviewers: a reverb with a choir-in-a-church tone) and per-band **FX FREQUENCY** buttons whose states are saved per effect [Review: [Digital DJ Tips V10](https://www.digitaldjtips.com/reviews/pioneer-dj-djm-v10-mixer/)]. The A9 reverb screen shows how deep the affected frequencies reach.

### 2.3 What happens on a beat-division change while active

No manual specifies it for Echo/Delay/Ping Pong. Evidence:

- NXS2 manual: pitch change is called out for Spiral/Helix only [Manual].
- DJM-A9: Echo and Ping Pong act as analog-tape style; the X-PAD slide produces tape-delay pitching; reviewers say "echo now produces a tape-delay effect when you change the timing" [Review: [We Are Crossfader](https://wearecrossfader.co.uk/blog/pioneer-dj-djm-a9/), [DJ Gym](https://www.djgym.co.uk/post/djm-900nxs2-vs-djm-a9)]. A9 owners posted that jumping between timings causes "a weird pitching effect" and that quick switching between the 1/3 timings produces unwanted "spiralling" sounds [Forum: [Pioneer thread](https://forums.pioneerdj.com/hc/en-us/community/posts/18751628091545-DJM-A9-weird-Beat-FX-behaviour-timing), via search summary].
- No source describes whether rekordbox software Echo glides or crossfades; the user's report that rekordbox does not bend is consistent with an instant or crossfaded switch.

**Conclusion**: treat "pitchless" as the default for Echo, Delay, Ping Pong and Up/Down Echo; keep a pitch-gliding mode only for Spiral and optional deliberate gestures (section 4.1).

### 2.4 LEVEL/DEPTH mapping and tails

- Counter-clockwise end is dry/original only; the Pioneer forum reports that on the NXS2 echo the first half of travel brings the wet in with the dry unchanged, and past 12 o'clock the original starts to fade [Forum, via search summary]. deckforge's "dry duck x level" is consistent with this.
- Spiral and Reverb knobs also feed feedback/decay, not just mix [Manual].
- Echo-out: with the channel faded or the input cut, "the delay sounds are faded out according to the track's tempo" [Manual: DJM-S9/NXS2 text]. Typical DJ settings quoted by [Sound System](https://www.sound-system.fr/en/pioneer-dj-effects-mastering-djm-and-xdj-in-2026/): Echo 60-80 % at 1/4 [Inf by author], Reverb 30-50 % at 1 beat, Delay 50-70 % at 1/2 or 1, Trans 80-100 % at 1/8 or 1/16. Echo-out sustain of roughly 2-4 beats.
- What the hardware does with the tail when you press OFF is not documented in the manuals I could read; community reports vary. Recommended: input closes immediately, tail keeps decaying (deckforge's `tail_left` model).

### 2.5 Sound Color FX (NXS2 manual; rekordbox/XDJ manuals say the same)

| Effect | COLOR CCW | COLOR CW | PARAMETER |
|---|---|---|---|
| Space | reverb on mid and low | reverb on mid and high | feedback (decay) up clockwise |
| Dub Echo | echo on mid only | echo on high only | feedback up clockwise |
| Sweep | gate, tightens | band-pass, narrows | CCW: gate shape; CW: centre frequency |
| Noise | lowers cutoff of the filter the white noise passes through | raises it | **noise volume**; quality via EQ/ISO |
| Crush | increases distortion | crush then high-pass | amount of crush |
| Filter | LPF cutoff down | HPF cutoff up | resonance |

Sources: [NXS2 manual](https://c3.zzounds.com/media/DJM-900NXS2_manual_EN-0703f22a06f23c5041e63da3db269aa6.pdf); [XDJ-AZ Sound Color FX](https://downloads.support.alphatheta.com/manuals/all-in-one-dj-systems/XDJ-AZ/html/en/000COV_en/Sound_Color_FX/Sound_Color_FX.htm). Centre is bypass on all; DJM-A9 added centre-lock to avoid over-sweeping [Review]. The A9 Crush is reportedly harsher than the NXS2's (a reviewer would avoid it in a club) [Review: DJ Gym], so "less harsh" is a defensible deckforge goal.

### 2.6 FX FREQUENCY, X-PAD, SEND/RETURN, Release FX, Smart CFX

- **FX FREQUENCY HI/MID/LOW**: lit buttons choose the band(s) the Beat FX apply to; the unselected bands stay dry [Manual: NXS2 step 5; V10 steps]. DJ TechTools: for Echo, Ping Pong, Spiral and Helix, deselect LOW; it gets "overwhelming" otherwise. Phaser can go lows-only for a gritty bassline. Crossover frequencies are not given in the manuals I read (they list only isolator reference points) [Inf: use LR4 at about 250 Hz and 3.5 kHz].
- **X-PAD**: touch turns the effect on, release turns it off; ON/OFF pressed during touch latches. Position changes the time/amount parameter. On Reverb it is the output filter [Manual].
- **SEND/RETURN**: NXS2 offers AUX (parallel, mix as desired) and INSERT (series). V10 gives built-in parallel effects with Master Mix and the option to route the wet-only signal into another channel to be EQ'd and filtered like any input [Review]. This is the conceptual model for an "FX return channel" in deckforge.
- **Release FX** (FLX4: SHIFT + BEAT FX ON/OFF): Vinyl Brake, Echo (echo-out), Back Spin in rekordbox; on the XDJ-RX3 more types, including build-up and ducking [Forum/Review: [We Are Crossfader](https://wearecrossfader.co.uk/blog/ddj-400-hidden-features/)].
- **Smart CFX** (FLX4): eight macro presets (Phantom Echo, Reflect Echo, Mobius Echo, Vaporize, Noise Chopper, Cyber Jet, Cyber Pitch, Twister) that combine two or more effects on one knob [Review/Forum: [Pioneer support](https://support.pioneerdj.com/hc/en-us/articles/12185533301913-When-I-m-using-rekordbox-for-Mac-Windows-with-the-unit-certain-effects-turn-off-after-I-turn-on-Smart-CFX-Why)].

## 3. Other pro references and what makes them sound professional

| System | Relevant effects | What makes it sound pro |
|---|---|---|
| **Allen & Heath Xone:92/96** | No built-in FX on the 96 (2 stereo sends, 4 returns, master insert). Xone:92 introduced dual VCF filters (LP/HP/BP, combinable) with resonance; 96 adds CRUNCH drive before the filter | State-variable, resonance fed back to just under self-oscillation, silent filter switching, drive before filter [Review: [Allen & Heath](https://www.allen-heath.com/hardware/xone-series/xone92/), [DJ Mag](https://djmag.com/features/how-allen-heaths-xone92-wrote-new-blueprint-dj-mixers)]. Traktor's Filter:92 models it with LFO and RES controls [Ref]. |
| **Traktor** | Reverb (HP, LP, SIZE, FRZ), Iceverb (ICING/COLOR/SIZE, filter outside the loop), Delay T3 (FILTER bipolar, FEEDB, RATE, FRZ, FR.R free-run), Ramp Delay (DURATION of rate ramp 1/4-16 bars, FB+ to 90 %), Bouncer, Beatmasher 2, Gater (NOISE, SHAPE hold/decay, RATE 1/4-1/32, MTE) | HP and LP **inside the effect loop**; freeze closes the input and holds the tail; free-run delay explicitly warns of pitch/rhythm skips; Gater adds hiss to mark the pattern; Ramp Delay makes the change an *event* with a ramp. [Ref: [Traktor effect reference](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference)] |
| **Serato** | iZotope-based Tape Echo, Spiral Echo Up/Down, Space Verb Lift/Drop, Filter Echo, Bass Ducker, Noise Sweep; Pro 3.2 adds Vast Reverb, Spiral Echo, Infinity Tone; Hex FX lists Tape Delay (Mix, Drive, Feedback, Tone, Inertia), Ping Pong Delay (Width, Filter), Inverse/Pitch Reverb (Pre-Delay, Decay, Density, Tone) | Delay "Inertia" and "Drive" parameters (smoothing and saturation); reverb tone, density and pre-delay; Bass Ducker as a named effect. [Ref: [Serato DJ-FX list](https://support.serato.com/hc/en-us/articles/10271055029263-DJ-FX-Channel-FX-list), [Effect List](https://support.serato.com/hc/en-us/articles/11349213839247-Effect-List)] |
| **Denon / Engine DJ** | Main FX (Echo Out, Swell Verb, Hold Echo, Riser, Reverb Rise/Drop, Reverse Reverb) with a **Frequency** parameter on every effect; Touch FX (Filter Echo, Filter Reverb, LFO Echo); Sweep FX (Dub Echo, Noise, Wash Out, Gate); X1850 manual: Hall Echo, FX parameter knob "freezes" reverb | Per-effect frequency focus, named build/release effects, filter-then-echo combos on one XY pad. [Ref: [Engine DJ 3.1](https://enginedj.com/news/articles/engine-dj-v3-1-0-now-available-touch-fx-25-new-and-enhanced-main-fx)] |
| **Ableton Live (studio reference)** | Delay: Repitch / Fade / Jump time-change modes, band-pass in the delay, LFO. Echo: Gate, **Ducking**, Noise, **Wobble**, Repitch toggle, reverb location pre/post/in-loop | Same trait list: time-change mode choice, in-loop filter, ducking, noise/wobble character. [Ref: [Live 12 manual](https://www.ableton.com/en/manual/live-audio-effect-reference/)] |

## 4. DSP techniques

### 4.1 Pitchless delay-time change

Pitch deviation of a delay line follows the rate of change of the delay time: read-speed = 1 - d(delay)/dt (delay in samples per sample). Tape-style slewing is therefore a pitch bend by definition.

| Method | Behaviour | Use when |
|---|---|---|
| **A. Dual-tap crossfade (recommended default)** | Two read heads on the same ring. On a division change, head B is set to the new delay and the output crossfades A to B; then A is retired. Equivalent to Ableton's Fade mode. | Any beat-division change on Echo, Delay, Ping Pong, Up/Down Echo, Dub Echo tempo jumps |
| **B. Two independent delay lines** | Old line's input closes (fast 20 ms ramp) and its feedback keeps running so old repeats decay at the old spacing; new line opens at the new spacing; outputs crossfade over about one repeat. Costs 2x memory. | When old repeats must finish naturally and the new rhythm should start cleanly; gives the most "mixer-like" sound |
| **C. Tape slew (repitch)** | Smooth change of delay time, pitch follows. Equivalent to Ableton Repitch, Traktor Free Run. | Spiral/Helix/Vinyl Brake, deliberate X-PAD-style gestures, Dub Echo throws |
| **D. Instant jump** | New delay immediately | Clicks if the previous repeat is still decaying [Ref: Ableton]; avoid |

Crossfade design (A):

- **Length**: 30-60 ms, default 40 ms; clamp to at most half of the new delay time for very short delays (floor 8 ms). Shorter fades click on low frequencies; longer fades smear transients and are audible as a swell.
- **Curve**: the two taps carry the same source at different lags, so they are partially correlated; use equal-gain Hann (`w_new = sin^2(pi t/2)`, `w_old = cos^2(...)`), which keeps level where the signals are in phase and dips only slightly where they are not.
- **Quantise the trigger** to the next 1/16 note boundary (or immediately if the effect was off). A change that lands on the grid hides the crossfade under a transient.
- **Feedback path**: write the *crossfaded* output back into the line; this makes repeats after the change consistent. The first one or two repeats after the change contain a mix of old and new spacing; that is normal and sounds like the echo "catching up".
- **Retriggers**: if a new change arrives during a fade, start the new fade from the current mix (three taps for at most one fade length; or snap the older tap to the target at zero gain).
- **Tempo changes** (pitch fader, sync): treat small continuous drifts (under about 3 %/s) with slewing at <= 0.002 samples/sample (about 3 cents), larger jumps as a division change.
- **Test**: feed a 1 kHz sine with echo on, change 1/4 to 1 beat and 1 to 1/2, measure instantaneous frequency of the wet via Hilbert or zero-crossing: deviation should stay within +-5 cents and the output max sample-to-sample jump must not exceed the dry signal's by more than 6 dB.

Tape-style gesture parameters (if kept): limit slew to 0.02-0.08 samples/sample, i.e. +-35 to +-140 cents, with an exponential approach (time constant 80-250 ms), and make it opt-in via the X-PAD-like control or hold-SHIFT. The current 0.35 is far past "musical": 12 log2(0.65) = -7.5 st and 12 log2(1.35) = +5.2 st; Spiral at 0.5 gives -12 / +7 st.

### 4.2 Reverb design for DJ use

**Algorithm choice** (all exist in deckforge's toolbox or are cheap to add):

| Topology | Character | Notes |
|---|---|---|
| **Dattorro/Griesinger plate** | Dense, bright, quick build, great on drums and vocals; short to medium decays | Paper defaults [Ref: [Dattorro 1997](https://ccrma.stanford.edu/~dattorro/EffectDesignPart1.pdf), Table 1]: input diffusion 0.75/0.625, decay diffusion 0.70/0.50 (second one = decay + 0.15, floor 0.25, ceiling 0.5), input bandwidth 0.9995 (a one-pole LP on the input; 0.9999999 = full bandwidth), damping 0.0005 (0 = none; raise toward 0.3-0.7 for darker), tank modulated allpasses at 672 and 908 samples (at 29.761 kHz) with about 8 samples peak excursion at roughly 1 Hz (paper's own example uses EXCURSION 16). Tank is unity-gain, so decay = 1 freezes it [Ref: Vinjar port, via search]. Use magnitude truncation or flush-to-zero in recursive paths to avoid noise-floor limit cycles. |
| **FDN (Jot)** | Smooth, flexible RT, easy frequency-dependent decay; what deckforge uses | 8-16 lines, mutually prime delays 20-80 ms, orthogonal (Householder/Hadamard) feedback matrix, per-line gain `g_i = 10^(-3 d_i / (fs RT60))` (deckforge's `exp(-6.9078 d/(rt fs))` is this), per-line absorption filter for HF decay |
| **Hall (long, dark)** | Large, slow bloom | FDN with RT_HF/RT_LF about 0.4-0.6, pre-delay 20-40 ms, early-reflection tapped delay (Moorer-style) before the tank [Ref: [CCRMA early reflections](https://ccrma.stanford.edu/realsimple/Reverb/Early_Reflections.html)] |

**Frequency-dependent decay (FDN, per line)**: want RT_L at DC and RT_H at Nyquist. With one-pole `H(z) = g (1-p)/(1 - p z^-1)`, `g = 10^(-3 d/(fs RT_L))`, `g_H = 10^(-3 d/(fs RT_H))`, set `p = (g - g_H)/(g + g_H)`. Typical ratios [Inf, consistent with Valhalla guidance that plates are bright and old hall units rolled off around 8-10 kHz]: plate RT_H/RT_L 0.8-1.0, hall 0.4-0.6, DJ "wash" 0.6. deckforge's current damping is a fixed one-pole mix (`damping*s + (1-damping)*y`); adding the RT-aware coefficient makes the tail darken *as it ages*, which is the biggest single cue of "expensive" reverb.

**Avoiding metallic tails** [Ref: [Valhalla DSP](https://valhalladsp.com/2011/01/21/reverbs-diffusion-allpass-delays-and-metallic-artifacts/), [ValhallaPlate diffusion](https://valhalladsp.com/2015/11/18/valhallaplate-diffusion-and-presets/)]:

- Cascaded allpasses behave like combs on periodic input (vocals, synth leads); coefficients of 0.7-0.99 give a fast attack and higher density but also the metallic ring. Keep input allpass coefficients 0.5-0.65 (deckforge 0.64 is at the upper edge), use 3-4 stages, and let the diffusion knob scale them.
- Modulate: slow (0.1-1 Hz) and small (<= 0.5 ms total) but on *every* line with uncorrelated phase and rates (deckforge already does). Back the depth off as RT60 grows, since long tails pass through the modulators many more times and detune [Ref: [Valhalla general reverb tips](https://valhalladsp.wordpress.com/2009/08/02/more-general-reverb-tips/)].
- Consider a simple spectral check on the tail (peak-to-median of 1/3-octave levels < 10 dB over 200 ms) as a regression test.

**High-pass on the reverb send/return** [Ref/[Inf]]: common mix guidance puts the HPF on reverb and delay sends anywhere between 100 and 600 Hz; 300 Hz with a gentle 6 dB/oct slope and a matching 8-10 kHz low-pass is a common starting point, 600 Hz in the "Abbey Road trick", 500 Hz in some producers' habits ([Audio Issues](https://www.audio-issues.com/music-mixing/this-simple-technique-makes-your-reverb-sit-perfectly-in-the-mix/), [Soundfly](https://flypaper.soundfly.com/produce/the-abbey-road-trick-how-to-eq-reverb-sends-to-free-up-space-in-a-mix/), [Sweetwater](https://www.sweetwater.com/insync/effect-tip-use-a-high-pass-filter-before-reverb/)). For a dance-music DJ context: **input HPF 250-350 Hz at 12 dB/oct (default 300 Hz), in-loop HPF about 150 Hz, output LPF 8-12 kHz, and an optional post-return HPF at 150-200 Hz** because reverbs regenerate low energy from the mids. Let the user's "Low" band-button (FX FREQUENCY style) move the input HPF down to 40 Hz for a rare "boomy" setting.

**Pre-delay**: 10-25 ms keeps the dry transient intact (deckforge default OK). Synced pre-delay at 1/32 note (about 58 ms at 128 BPM) works for "throw" reverbs on vocals. Zero pre-delay for swells.

**Gated reverb**: heavy reverb into a gate keyed from the dry; hold about 0.5 s then fast release; the nonlinear AMS RMX16 "NonLin2" program imitated this with a shaped decay [Ref: [Wikipedia](https://en.wikipedia.org/wiki/Gated_reverb)]. For DJ use: gate keyed by a beat-grid trigger rather than the dry (the dry is a full mix); hold = 1/2 or 1 beat, release 20-50 ms, optionally a 5-10 ms attack ramp to remove clicks. Pioneer's closest equivalent: Trans on a reverb tail.

**Reverse-swell reverb**: true reverse reverb needs look-ahead (reverse the input segment, reverberate, reverse the result). With a beat grid a cheap approximation is available: ramp the wet return gain exponentially across the last beat (or two) before the downbeat while the send is held, and cut both at the downbeat. See `fx-tension.md` section 5.4 for the look-ahead variant.

**Shimmer** [Ref: [Valhalla notes](https://valhalladsp.com/shimmer/ValhallaShimmerNotes.pdf), [Valhalla Eno/Lanois recipe](https://valhalladsp.com/2010/05/11/enolanois-shimmer-sound-how-it-is-made/) via search summary]: loop = input + feedback, then reverb, then output; the pitch shifter (+12 semitones, granular delay-line shifter with two Hann-windowed read heads 180 degrees apart moving at 2x) sits **in the feedback path** taking reverb output to the reverb input. Because it re-shifts each pass, one octave of shift is enough. Starting values for DJ use:

- Shift +12 st (optional second voice +7 or +19 at -6 dB for a "choir" colour), grain window 40-60 ms (deckforge `SHIFT_WINDOW_SECS` 0.04 already).
- Loop gain through the shifter 0.30-0.50 (Valhalla suggests 0.5 or more for the classic sound; DJ material wants less), with `tanh`-type soft clip or limiter in the loop (the original used an analog desk, which limited naturally).
- In the shifter path: HPF 600-1200 Hz before the shift (keeps lows out) and LPF 10-12 kHz after; plus a high-shelf of +3 to +5 dB at 6 kHz on the output.
- Mix shimmer voice 6-9 dB below the plain tail so it reads as air, not as a synth pad.
- Costs: one shifter per channel, i.e. same as Up/Down Echo; `pitch.rs` already provides the primitive.

**Freeze/infinite reverb** [Ref: Traktor Reverb/Iceverb "closes the input and leaves the output open"; Delay "FRZ pushes feedback to maximum"; Vinjar port: decay = 1]: close the input with a 5-20 ms ramp, set loop gain to 0.9995-1.0 (use 0.9999 so DC and low-frequency energy cannot build), keep the in-loop HPF (150 Hz) and LPF (6-8 kHz) so the frozen tail does not get loud or boomy, keep modulation running so it breathes rather than sustaining a static tone, and gently ramp the wet gain on release (200-400 ms). Iceverb's trick of leaving SIZE live while frozen gives pitch glides; deckforge could tie it to a deliberate gesture.

**Ducking reverb** [Ref: [Soundfly](https://flypaper.soundfly.com/produce/hold-up-can-you-sidechain-reverb/), [MusicTech](https://musictech.com/tutorials/using-sidechain-compression-to-duck-a-vocal-delay/), [Ableton Echo ducking](https://www.ableton.com/en/manual/live-audio-effect-reference/)]: compressor on the wet return keyed from the dry input. Attack 5-15 ms (1-5 ms is more aggressive), release 120-300 ms (or tempo-timed at 1/8-1/4 note), ratio 4:1-8:1, **3-8 dB** of gain reduction. For DJ material (a dense full mix) use a gentler 3-5 dB, with a key signal band-limited to 150 Hz - 5 kHz so the kick does not pump the wet. Implement as an envelope follower (attack 8 ms, release 200 ms) on the dry mono sum driving a smoothed gain on the wet return; far cheaper than a real compressor. It lets reverb and echo sit louder (larger "wet" feeling) without clouding the mix: the bloom appears in the gaps and breakdowns, which is where the emotion lives.

### 4.3 Tension and build techniques

Producers' building blocks [Ref: [Unison risers](https://unison.audio/how-to-create-risers/), [EDMProd build-ups](https://www.edmprod.com/ultimate-guide-build-ups/), [Attack Magazine endless riser](https://www.attackmagazine.com/technique/tutorials/how-to-make-an-endless-riser/), [Ready for Masterclass](https://readyformasterclass.com/three-types-of-risers-that-make-builds-feel-massive/)]:

| Technique | Concrete recipe |
|---|---|
| **White-noise riser** | Noise through a band-pass swept ~200 Hz to 8 kHz over the build; level from silence to 0 dB following an accelerating (convex) curve; HPF 150 Hz on the noise layer; stereo width opens during the rise; light reverb to soften the harshness |
| **HPF sweep** | Dry HPF from ~25 Hz to 300-600 Hz across the build; removing lows makes the return of bass at the drop feel bigger without any gain change |
| **Filter + resonance build** | LPF/HPF with resonance rising to Q 4-8 (just under self-oscillation) as cutoff rises; Xone-style filter with drive before it |
| **Reverb wash-out** | Hall send at 100 % wet; send amount and decay grow through the build (some producers start at a few ms of decay and extend); **cut the tail at the drop** by automating wet to zero: the sudden dryness makes everything feel closer and punchier |
| **Gated tremolo on tails** | Tremolo/gate on the reverb return at 1/8 then 1/16 then 1/32 note across the build (Traktor Gater: rate 1/4-1/32, shape hold/decay); adds rhythmic energy without more level |
| **Echo-out** | Close the input, let feedback ring 2-4 beats; optionally pitch the repeats up or drive a rising HPF in the loop; end with a gap |
| **Pitch riser / Shepard** | Several sine or saw voices an octave apart, each with a bell-shaped amplitude envelope over log-frequency, all gliding at the same rate, so the sound seems to rise without leaving the register [Ref: [Shepard tone](https://en.wikipedia.org/wiki/Shepard_tone)]; Attack uses +12 st pitch-bend voices staggered and crossfaded, with distortion before the reverb; a noise version uses ~8 band-pass filters on white noise with saw-LFO pitch and raised-sine volume LFOs |
| **Dynamics/space tricks** | Narrow stereo toward mono during the build, snap to full width at the drop; do not make the drop louder, lower the build; leave a 1-beat gap or half-bar silence before the drop; keep riser pitch under about 4-6 kHz fundamental to avoid fatigue |

Mapping to deckforge's Reverb: expose a **tension macro** on LEVEL (or Release-hold) rather than a static mix:

| Parameter | At 0 | At 1 (peak tension) | Curve |
|---|---|---|---|
| Wet send | -inf | 0 dB re dry RMS | convex, `t^2` (accelerates) |
| RT60 | 1.2 s | 6-8 s | exponential in t |
| Wet input HPF | 250 Hz | 1.5-2.5 kHz | log in t |
| Dry HPF (optional, "thin") | off | 120-250 Hz | log in t, only when macro latched |
| Wet width | 100 % | 100 % (narrow the *dry* sides 20 % if you can) | linear |
| On release | | wet gain to zero in 15-40 ms; dry restored over 1 beat | hard cut; snap |

This mirrors the "growing decay then cut at the drop" approach and is the direct answer to "reverb doesn't build tension".

### 4.4 Dub/tape echo character versus clean digital

| Aspect | Clean digital (Pioneer Echo) | Tape/dub echo |
|---|---|---|
| Loop filtering | gentle LP (4-6 kHz), HP 100-200 Hz | stronger band-pass: HPF 150-600 Hz, LPF 2.5-4 kHz, so repeats "melt into the distance" [Ref: [UA RE-201 manual](https://help.uaudio.com/hc/en-us/articles/33538271334164-Roland-RE-201-Space-Echo-Manual)] |
| Saturation | none or tiny | soft clip (tanh) inside the loop, drive about 1.5-3x; compressive, rounds repeats |
| Time stability | exact | wow 0.3-1.5 Hz and flutter 5-15 Hz delay-time modulation; defaults in open implementations 0.5 Hz wow / 8 Hz flutter [Ref: [HISE Faust example](https://forum.hise.audio/topic/11185/retro-80s-tape-wow-flutter-with-faust/20)]; the Space Echo model uses band-limited random drift below about 1 Hz [Ref: [Downing and Terjesen](https://hajim.rochester.edu/ece/sites/zduan/teaching/ece472/projects/2016/Downing_Terjesen_paper.pdf)] |
| Feedback | below 1, hard ceiling | at or above 1 with a limiter: the RE-201 self-oscillates at high Intensity; the loop band-limits so the howl sits in the mids |
| Time change | crossfade or jump | pitch glide (speed change) |
| Stereo | ping-pong or mono | mono tape with stereo spread added by short offsets |

**Wow/flutter amounts** (delay-time modulation, sine, depth A seconds at rate f gives peak speed error 2 pi f A): the existing 0.7 Hz and 0.25 ms is about +-0.11 % (about 2 cents), i.e. a good "well-kept machine" setting. A "worn" setting: wow 0.6 Hz at 0.8-1.2 ms (+-0.3-0.45 %, 5-8 cents) plus flutter 9 Hz at 0.03 ms (+-0.17 %). Quality classes: hi-fi tape +-0.12 %, third-class +-0.35 % [Ref: [Wikipedia wow and flutter](https://en.wikipedia.org/wiki/Wow_and_flutter_measurement)]. Use opposite-phase modulation on L and R to widen. Random delay-time modulation above about 1 % starts to sound like detuning rather than tape [Ref: VCV forum summary].

**Feedback above 100 %**: allowed only with a bound: `y = tanh(drive x)/tanh(drive)` or a soft limiter in the loop, plus the band-pass above, so the loop gain exceeds 1 only in a narrow mid band and saturation clamps amplitude. Offer it as the top 10-15 % of the Dub Echo PARAMETER range; slew the feedback back to 0.6 on release over 150-300 ms so the howl does not hang.

**Feedback vs repeats** (how long the tail rings; repeats to fall by 40 dB):

| Feedback | 0.5 | 0.6 | 0.7 | 0.8 | 0.9 |
|---|---|---|---|---|---|
| Repeats to -40 dB | 6.6 | 9.0 | 12.9 | 20.6 | 43.7 |

At 1/2 beat and 128 BPM (234 ms per repeat), 0.6 gives a 2.1 s tail, 0.7 about 3 s, 0.8 about 4.8 s, 0.9 about 10 s. Pioneer-style "2-4 beats" echo-out sits between 0.55 and 0.7 at 1/2 to 1 beat. deckforge's floor of 0.55 rising to 0.85-0.95 gives very long tails at high level; keep 0.9+ for "dub throw" only.

## 5. Per-effect parameter sets and recommendations

Parameter ranges below are *musical ranges to stay in*; values are starting points [Inf] unless a manual figure is quoted.

### 5.1 Echo (Beat FX)

- **Divisions** offered by FLX4/rekordbox steps: 1/16, 1/8, 1/4, 1/2, 3/4, 1, 2, 4, 8, 16 beats (the user quoted 1/4, 1/2, 3/4, 1 "bar"; confirm they mean beats as Pioneer does; the design holds either way). Odd divisions (3/4) give the familiar dotted-eighth rhythm.
- **Division change**: dual-tap crossfade 40 ms, quantised to 1/16 (section 4.1). No pitch deviation beyond +-5 cents.
- **LEVEL**: 0 dry; at 0.5 about equal dry/wet; above 0.5 the original ducks to about 0.7 (keeps current `dry_duck` idea, but make the duck signal-driven: duck the *wet* 3-5 dB while dry is loud, and let it bloom in gaps).
- **Feedback** `fb = 0.40 + 0.35 x level^1.2` (0.40-0.75), hard ceiling 0.92 during normal operation, higher only for Freeze/Dub throw. Echo-out tail 2-4 beats.
- **Loop filters**: HPF 150-250 Hz and LPF 3.5-6 kHz, both **12 dB/oct** (2-pole SVF); the current 1-pole LP means highs stay bright for many repeats, which sounds digital.
- **Saturation**: `tanh` soft clip with 1.2-1.5x drive in the loop (subtle). More for Dub.
- **Stereo**: for non-ping-pong echo, offset R by +0.3 to +1.0 ms and modulate L/R delay times in antiphase at 0.3-0.5 Hz with +-0.1 ms; Ping Pong with a 1/16 offset between sides is the "wide" option.
- **Band**: default FX FREQUENCY LOW off (wet HPF about 150-200 Hz).
- **Off behaviour**: close input in 5-10 ms, tail rings out at the set feedback; "kill" on SHIFT+OFF.

### 5.2 Delay, Ping Pong, Up Echo, Down Echo

- **Delay**: single repeat at the division; keep feedback 0-0.15 (one dominant repeat, optional faint second). LP 8-9 kHz fine.
- **Ping Pong**: feedback 0.45-0.75, alternate sides, LP 5 kHz, width 100 %; apply the same crossfade. Pioneer's description: attenuated repeats with *different* delay timing left and right; use L = division, R = 0.75 x division (dotted feel) or equal with alternation, selectable.
- **Up/Down Echo (DJM-850 style; pitch shift plus delay)**: shift step +-1 semitone per repeat (current +-2 climbs 10 semitones over five repeats and sounds cartoonish), HPF 200-250 Hz in the loop so shifted lows do not pile up, LP 7 kHz (Up) / 3 kHz (Down). These are *intentionally* pitched; **do not** apply the pitch-glide to the division change.

### 5.3 Spiral

- Pioneer: reverberation-like with pitch change on time change [Manual]. Keep the slew (0.15-0.5 samples/sample is fine) because it *is* the feature, but add 2-3 allpass diffusers with 0.5-0.6 gain (present), 0.3 ms mod at 0.35 Hz (present). Add an input HPF 200 Hz so the "tunnel" does not muddy the bass. Feedback 0.55-0.85.

### 5.4 Reverb (Beat FX)

- **Percent (1-100)**: RT60 0.8 s to 7 s on a log curve (deckforge's 11.5 s top end is best reserved for Freeze); Pioneer's % is size+amount combined.
- **Level**: wet amount (equal-power-ish to avoid a hole at 50 %), dry stays at 1.0 until 0.6 then eases to 0.7.
- **Pre-delay** 12-20 ms default; 0 on swell.
- **Input HPF** 300 Hz (range 100-600 Hz) 12 dB/oct; input LPF 10 kHz; output post-HPF 150 Hz.
- **Decay shape**: RT_H/RT_L about 0.55 at 4 kHz; slight HF boost at the start (plate-like) for clarity.
- **Diffusion** input allpass 0.5-0.62; tank diffusion fixed.
- **Modulation** 0.25-0.4 ms depth at 0.15-0.6 Hz per line, reducing to 60 % depth above 5 s RT60.
- **Ducking** 4 dB, release 200 ms.
- **X-PAD analogue**: a bipolar filter on the wet (Pioneer: sets filter cutoff of the reverb output): negative = LP sweep, positive = HP sweep; also assignable to the tension macro.
- **Tension macro / Freeze / Gate**: section 4.3 and 4.2. Cut-at-drop behaviour: on release (or SHIFT+OFF) hard-gate the wet in 20 ms.
- **Band**: LOW off by default via the wet HPF.

### 5.5 Flanger and Phaser (brief)

- Flanger: delay 0.4-4.4 ms sweep, feedback 0.55 (present, fine), one cycle per beat fraction (Pioneer 1/16-64 beats), sine or triangle LFO, optional negative feedback for the hollow sound; LEVEL CCW = dry. Use a lows-only high-pass of the wet at 100 Hz to avoid bass flanging unless the user enables LOW.
- Phaser: 6 stages is fine; 300-3500 Hz sweep; feedback 0.4-0.55; cycle beat-synced. DJ TechTools: lows-only phaser is a legitimate bassline-grit technique; offer as a band option rather than a separate effect.

### 5.6 Roll and Trans

- Roll: capture length 1/16-1 beat, loop seam crossfade 1-3 ms (present 64 samples ~ 1.3 ms is right). Build mode: halve the division each beat (1/2 then 1/4 then 1/8 then 1/16) while level and HPF rise; Slip Roll re-records on time change per the manual.
- Trans: gate with 1.5 ms smoothing (present), duty from LEVEL (Pioneer), 1/16-1/2 beat; optional hiss like Traktor Gater NOISE.

### 5.7 Filter Sweep (Beat FX Filter) and Sound Color Filter

- State-variable, 12 dB/oct, cutoff 80 Hz - 16 kHz exponential, resonance Q 0.7-6 (Xone:92 feel, "just short of self-oscillation" at the top); smooth coefficient changes at <= 5 ms; optional pre-filter drive (Xone:96 CRUNCH) as a second knob.
- Sound Color Filter: CCW LPF 20 kHz down to about 80 Hz, CW HPF up to about 8 kHz, 3 % dead zone at centre, PARAMETER = resonance (Q 0.7-8) [Manual: resonance on PARAMETER]; compensate gain by about 2-3 dB at high Q.

### 5.8 Space (Sound Color FX)

Pioneer: reverb, CCW mid+low, CW mid+high, PARAMETER feedback [Manual]. The user wants almost no effect on the low end and a shimmer on the highs.

- **Dry path untouched**: no dry HPF, no dry dip beyond 1 dB (remove `filter_dry` high-passing and `SPACE_DRY_DIP` on the bright side).
- **Bright side (CW)**: wet input HPF 500 Hz (knob 0.1) to 2.5 kHz (knob 1.0), 12 dB/oct, then a second HPF at 400 Hz after the tank; wet LPF 14-16 kHz; high-shelf +3 to +4 dB at 6 kHz. Net low-end contribution should be below -30 dB under 200 Hz.
- **Shimmer voice**: +12 st in the loop as in 4.2 at 0.35-0.5 loop gain, scaled by knob position; second voice (+7 st) appears above 0.7.
- **Decay**: RT60 2.5 s at low knob to 7 s at full (current 3.5-9.5 s is long for a Sound Color effect; PARAMETER scales within it), pre-delay 15-25 ms, ducking 4 dB.
- **Level**: wet -14 dB at 10 % travel to -5 dB at 100 % (relative to dry RMS); fade-in slope as present (40/turn) avoids pops near the dead zone.
- **Dark side (CCW)**: keep as is but HPF the wet at 150-200 Hz and LPF 3.5-5 kHz so the "mid+low" bloom is warm rather than muddy.
- **Off**: post-fader style tail, `SPACE_TAIL_SECS` 14 s is fine but let the shimmer feedback fade first (it dies faster) so the tail does not end on a synth-like drone.

### 5.9 Dub Echo (Sound Color FX)

Pioneer: delay slightly after the original, several attenuated repeats; CCW mid-only, CW high-only; PARAMETER feedback [Manual].

- Time 3/4 beat default (dotted eighth; present), alternative 1/2 beat; changes handled by crossfade, except tempo drift which slews.
- Feedback 0.45 at PARAMETER 0 to 0.90 at 0.85, then 0.90 to 1.05 across the top 15 % with `tanh` limiter in the loop (self-oscillation "dub throw"), slewed back on release.
- Colour: CCW band-pass 400 Hz - 2.5 kHz; CW HPF 600 Hz - 1.5 kHz plus LPF 6 kHz; both 12 dB/oct inside the loop so each repeat narrows; plain LP 3.8 kHz (present) is fine for the neutral position.
- Character: tanh drive 2x, wow 0.6 Hz at 0.3-0.6 ms and flutter 9 Hz at 0.03 ms (antiphase L/R).
- Duck the wet 3 dB under dry; cut dry slightly (0.5 dB) at max.

### 5.10 Crush

Pioneer: CCW distortion, CW crush then HPF, PARAMETER amount [Manual]. The A9 version is reported as harsher [Review], so aim for musical:

- Bit depth 16 to 5 bits, sample-rate reduction 48 kHz to about 6-8 kHz (zero-order hold, aliasing is part of the sound), no dither.
- Post-crush LP at 9-12 kHz (12 dB/oct) and DC block; output level compensation, dry/wet in parallel (not 100 % wet until past 0.9).
- CW HPF 80 Hz to 1.2 kHz with the knob; CCW: add `tanh` drive 1x to 6x ahead of the crush.
- Smooth parameter changes (10-20 ms) to avoid zipper noise when the bit depth steps.

### 5.11 Noise

Pioneer: white noise generated internally, mixed through a filter whose cutoff follows COLOR; PARAMETER = noise volume; EQ/ISO shapes the noise quality [Manual]. Goofy noise is usually flat hiss at constant level.

- **Spectrum**: blend white with pink (about -3 dB/oct tilt, 50/50) so it is less hissy; HPF at 150 Hz always (avoid low rumble clashing with bass) [Ref: Unison risers].
- **Filter**: SVF band-pass (Q 1-3) rather than a simple HP/LP; cutoff sweeps 200 Hz - 8 kHz following COLOR on an exponential curve (CCW low, CW high), plus slow +-4 % cutoff LFO at 0.3 Hz for movement.
- **Level**: amplitude follows `t^2` (accelerating) on PARAMETER, peak about -9 to -6 dB below dry RMS (deckforge normalises to a reference RMS already); do not exceed -4 dB.
- **Stereo**: two decorrelated generators (present), slow stereo widen with level.
- **Ducking**: key from the dry so noise stays under vocals/leads: 3-4 dB duck.
- **Tail**: on release, close the band-pass cutoff downward over 1-1.5 s while level fades (a falling "whoosh") instead of an immediate hiss cut; NOISE_TAIL_SECS 3.5 is fine for the fade, but cutoff-closing is the missing character.
- **Option**: Traktor-style Gater hiss variant (rhythmic 1/8-1/16 tremolo on the noise) as an alternate mode.

## 6. Implementation recommendations, ranked by impact on the user's complaints

| Rank | Change | Complaint addressed | Effort | Acceptance test |
|---|---|---|---|---|
| 1 | **Dual-tap crossfade for beat-division changes** on Echo, Delay, Ping Pong, Up/Down Echo (40 ms Hann fade, quantise to 1/16, keep slewing only for Spiral and Dub tempo drift) | Echo pitch bend | small (ring buffer already exists; `swap`/`SwapGate` infrastructure helps) | 1 kHz sine, change 1/4 to 1 to 1/2: wet instantaneous frequency within +-5 cents; no click |
| 2 | **Space dry-path fix + wet-only band shaping**; remove dry HPF and dip on the bright side; wet HPF 500 Hz to 2.5 kHz | Space low-end | small | Space CW at 1.0: dry transfer within 1 dB at 80 Hz; wet energy below 200 Hz < -30 dB |
| 3 | **Space shimmer voice** (+12 st in loop, loop gain 0.35-0.5, tanh limiter, HPF/LPF in loop) | "Almost a shimmer" | medium (reuse `PitchShifter`) | tail spectrum shows energy at 2x frequency of a pure tone input; stays finite under 60 s of full-scale noise |
| 4 | **Reverb tension macro** (rising wet, RT60, wet HPF; beat-phased swell; hard cut on release) and **Freeze** | Reverb tension | medium | automated test: at t=1 wet HPF 2 kHz, RT60 >= 6 s; release gates wet to -60 dB within 40 ms |
| 5 | **Reverb/Space quality pass**: RT-aware damping (RT_H/RT_L 0.55), input HPF 300 Hz / LPF 10 kHz, post-HPF 150 Hz, modulation depth scaled by RT60, input allpass 0.55-0.62 | "Goofy", unprofessional | medium | measured RT60 at 4 kHz about 0.55 x RT60 at 250 Hz; tail spectral peak-to-median < 10 dB; low-end wet leakage < -25 dB below 150 Hz |
| 6 | **Dry-keyed ducking** on reverb/Space/echo wet (3-6 dB, attack 8 ms, release 200 ms) | "Goofy", muddy | small-medium | with continuous dry the wet RMS is 3-5 dB lower than in gaps |
| 7 | **Echo loop character**: 2-pole HPF/LPF in loop, tanh saturation, stereo decorrelation, wow/flutter on Dub | Echo/Dub "digital" or goofy | small | each repeat loses 1-2 dB more at 5 kHz than at 1 kHz; repeats stay finite at fb 0.95 |
| 8 | **Noise voicing** (pink blend, band-pass sweep, accelerating level, ducking, falling tail) | Noise goofy | small | spectral tilt about -3 dB/oct at neutral; level curve t^2; tail cutoff falls over 1.2 s |
| 9 | **FX FREQUENCY-style band control** (LR4 split at 250 Hz and 3.5 kHz, LOW off by default for Echo/Reverb) | Muddy echo/reverb, parity with DJM | medium | selected-band bypass leaves unselected bands within 0.5 dB and linear-phase-aligned (LR4 sums flat) |
| 10 | **Gated reverb, reverse-swell approximation, noise-riser Shepard mode** | Emotion, builds | medium-large | audition via `fx-demos` |
| 11 | **Release/tail policy** consistent across effects (input closes, tail decays, SHIFT kills) and loudness match (<= 1.5 dB of dry, as already done for the echo family) | Professional feel | small | existing `fx.rs` tests extended |

Why this order: items 1-3 resolve three of the four explicit complaints directly; 4 resolves the fourth; 5-8 target the diffuse "goofy" feedback, where the biggest single wins are wet high-pass, aging (darkening) tails and ducking.

Suggested listening tests via `fx-demos` after each step: (a) drum loop + sub bass, reverb at 40 %, LOW band off versus on; (b) sustained pad, shimmer on/off; (c) vocal-like gaps, ducking on/off; (d) echo with division sweep 1/4, 1/2, 3/4, 1, 1/2 at 128 BPM; (e) 8-bar build with the tension macro, cut at bar 9.

## 7. Open questions and caveats

- The Pioneer manuals do not define reverb RT60 versus %, FX FREQUENCY crossover frequencies, or what OFF does to a ringing echo; the figures here are design proposals.
- I could not retrieve DJM-A9/V10 manual pages directly (hosting blocked); A9/V10 details come from reviews, search excerpts, and the NXS2 manual text extracted locally. Treat the A9 Echo tape behaviour as reported, not verified.
- The user wrote "1 bar" for the largest of their four divisions; Pioneer counts in beats. Clarify before fixing the mapping table in the UI.
- Shimmer, Traktor and Valhalla designs are cited for technique only; implement from the algorithms described (two-head granular shifter, in-loop filters, FDN), not from any vendor code.

## 8. Sources

**Pioneer / AlphaTheta / rekordbox**
- DJM-900NXS2 Operating Instructions (PDF) - https://c3.zzounds.com/media/DJM-900NXS2_manual_EN-0703f22a06f23c5041e63da3db269aa6.pdf
- DJM-900NXS2 manual, ManualsLib "Types of BEAT FX" - https://www.manualslib.com/manual/1275366/Pioneer-Djm-900nxs2.html?page=15
- DJM-A9 Instruction Manual, Beat FX types and settings (page 52) - https://www.manualslib.com/manual/3001872/Pioneer-Dj-Djm-A9.html?page=52
- DJM-V10 manual, using Beat FX - https://www.manualslib.com/manual/1824795/Pioneer-Dj-Djm-V10.html?page=59
- DJM-V10 effects feature page - https://www.pioneerdj.com/en/product/features/mixer/djm-v10-effects/
- DJM-TOUR1 manual (Pitch/Trans/Roll figures) - https://manuals.plus/m/a7678589c8c0f1bb7105f35557cbd7b7ad8bc0a42537f5af1dfde2c1928531fc
- XDJ-AZ Sound Color FX - https://downloads.support.alphatheta.com/manuals/all-in-one-dj-systems/XDJ-AZ/html/en/000COV_en/Sound_Color_FX/Sound_Color_FX.htm
- DDJ-FLX4 Instruction Manual - https://assets.ctfassets.net/q602vtcuu3w3/2vz7GMLHawLZBK6gWpCP9V/26bd62eef3eca3714ddb1d3396d30f7b/DDJ-FLX4_manual_EN.pdf
- Pioneer DJ support: Smart CFX effect behaviour - https://support.pioneerdj.com/hc/en-us/articles/12185533301913-When-I-m-using-rekordbox-for-Mac-Windows-with-the-unit-certain-effects-turn-off-after-I-turn-on-Smart-CFX-Why
- Pioneer forum: DJM-A9 weird Beat FX timing - https://forums.pioneerdj.com/hc/en-us/community/posts/18751628091545-DJM-A9-weird-Beat-FX-behaviour-timing
- Pioneer forum: Echo effect and the channel fader - https://forums.pioneerdj.com/hc/en-us/community/posts/115000174526-Echo-effect-has-no-volume-fader-control

**Reviews and DJ technique**
- DJ TechTools, 3 techniques for the DJM-900NXS2's EQ-based effects - https://djtechtools.com/amp/2016/04/04/3-techniques-pioneer-djm-900nxs2s-eq-effects/
- We Are Crossfader, DJM-A9 review - https://wearecrossfader.co.uk/blog/pioneer-dj-djm-a9/
- DJ Gym, DJM-900NXS2 vs DJM-A9 - https://www.djgym.co.uk/post/djm-900nxs2-vs-djm-a9
- Digital DJ Tips, DJM-V10 review - https://www.digitaldjtips.com/reviews/pioneer-dj-djm-v10-mixer/
- Digital DJ Tips, using echo when DJing - https://www.digitaldjtips.com/how-to-use-echo-when-djing/
- Sound System blog, Pioneer DJ effects guide - https://www.sound-system.fr/en/pioneer-dj-effects-mastering-djm-and-xdj-in-2026/
- DJ Knowledge Base, Effects - https://pestrela.github.io/dj_kb/effects/
- We Are Crossfader, DDJ-400 hidden features (Release FX) - https://wearecrossfader.co.uk/blog/ddj-400-hidden-features/

**Other pro platforms**
- Allen & Heath Xone:92 - https://www.allen-heath.com/hardware/xone-series/xone92/
- DJ Mag, how the Xone:92 wrote a blueprint - https://djmag.com/features/how-allen-heaths-xone92-wrote-new-blueprint-dj-mixers
- Xone:96 User Guide - https://support.allen-heath.com/hc/en-gb/articles/43053562464017-Xone-96-User-Guide
- Traktor Pro effect reference - https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/effect-reference
- Serato DJ-FX and Channel FX list - https://support.serato.com/hc/en-us/articles/10271055029263-DJ-FX-Channel-FX-list
- Serato Effect List (Hex FX parameters) - https://support.serato.com/hc/en-us/articles/11349213839247-Effect-List
- Engine DJ 3.1 release (Main FX, Touch FX, Frequency parameter) - https://enginedj.com/news/articles/engine-dj-v3-1-0-now-available-touch-fx-25-new-and-enhanced-main-fx
- Ableton Live 12 audio effect reference (Delay, Echo) - https://www.ableton.com/en/manual/live-audio-effect-reference/

**DSP and production technique**
- Dattorro, "Effect Design Part 1: Reverberator and Other Filters", JAES 45(9), 1997 - https://ccrma.stanford.edu/~dattorro/EffectDesignPart1.pdf
- Valhalla DSP, reverbs, allpass delays and metallic artifacts - https://valhalladsp.com/2011/01/21/reverbs-diffusion-allpass-delays-and-metallic-artifacts/
- Valhalla DSP, ValhallaPlate on diffusion - https://valhalladsp.com/2015/11/18/valhallaplate-diffusion-and-presets/
- Valhalla DSP, more general reverb tips - https://valhalladsp.wordpress.com/2009/08/02/more-general-reverb-tips/
- Valhalla DSP, Eno/Lanois shimmer - https://valhalladsp.com/2010/05/11/enolanois-shimmer-sound-how-it-is-made/
- ValhallaShimmer notes (PDF) - https://valhalladsp.com/shimmer/ValhallaShimmerNotes.pdf
- CCRMA, early reflections - https://ccrma.stanford.edu/realsimple/Reverb/Early_Reflections.html
- Wikipedia, gated reverb - https://en.wikipedia.org/wiki/Gated_reverb
- Wikipedia, Shepard tone - https://en.wikipedia.org/wiki/Shepard_tone
- Downing and Terjesen, real-time digital modeling of the Roland Space Echo - https://hajim.rochester.edu/ece/sites/zduan/teaching/ece472/projects/2016/Downing_Terjesen_paper.pdf
- Chowdhury, real-time physical modelling for analog tape machines (DAFx-19) - https://www.dafx.de/paper-archive/2019/DAFx2019_paper_3.pdf
- UA RE-201 Space Echo manual - https://help.uaudio.com/hc/en-us/articles/33538271334164-Roland-RE-201-Space-Echo-Manual
- Wikipedia, wow and flutter measurement - https://en.wikipedia.org/wiki/Wow_and_flutter_measurement
- Audio Issues, reverb in the mix (HPF 300 Hz / LPF 8 kHz) - https://www.audio-issues.com/music-mixing/this-simple-technique-makes-your-reverb-sit-perfectly-in-the-mix/
- Soundfly, Abbey Road trick - https://flypaper.soundfly.com/produce/the-abbey-road-trick-how-to-eq-reverb-sends-to-free-up-space-in-a-mix/
- Sweetwater, high-pass before reverb - https://www.sweetwater.com/insync/effect-tip-use-a-high-pass-filter-before-reverb/
- Soundfly, sidechaining reverb - https://flypaper.soundfly.com/produce/hold-up-can-you-sidechain-reverb/
- MusicTech, ducking a vocal delay - https://musictech.com/tutorials/using-sidechain-compression-to-duck-a-vocal-delay/
- Unison, how to create risers - https://unison.audio/how-to-create-risers/
- EDMProd, build-up guide - https://www.edmprod.com/ultimate-guide-build-ups/
- Attack Magazine, how to make an endless riser - https://www.attackmagazine.com/technique/tutorials/how-to-make-an-endless-riser/
- Ready for Masterclass, three types of risers - https://readyformasterclass.com/three-types-of-risers-that-make-builds-feel-massive/
