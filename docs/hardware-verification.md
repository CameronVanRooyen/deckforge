# DDJ-FLX4 hardware verification checklist

The FLX4 was **not connected** while the MVP was built. Everything below is implemented and tested against simulated MIDI and virtual audio devices; tick each item on the real controller.

Tools: deckforge Settings → MIDI diagnostics (live in/out log), Settings → Audio (device, channels, callbacks, xruns), `cargo run -p deckforge-cli -- devices`.

## Connection
- [ ] App detects the controller (toolbar shows FLX4 connected); MIDI port names recorded here: ______
- [ ] Unplug while playing: app keeps running, status shows disconnected; replug: reconnects within ~2 s and LEDs resync.
- [ ] Audio device appears as `DDJ-FLX4` with 4 output channels at 44.1/48 kHz; auto-selected on first launch.
- [ ] Unplug audio while playing: status → lost, no crash; replug → resumes.

## Audio routing (headphones on, master to speakers at LOW volume)
- [ ] Deck A playing, fader up → heard on MASTER only.
- [ ] Deck B fader down, CUE (headphone) on → heard in headphones only, not on master.
- [ ] Crossfader fully to A does not mute deck B in headphones.
- [ ] HEADPHONES MIX knob: does it blend master into phones with deckforge sending pure cue (hardware-phones mode ON, the default)? If not, switch hardware-phones OFF in settings and re-test.
- [ ] MASTER LEVEL / HEADPHONES LEVEL knobs change volume in hardware (expected) — deckforge only displays them.

## Controls (watch MIDI log)
- [ ] PLAY/CUE both decks (CDJ cue: set when paused, preview while held, return on release).
- [ ] Jog: touch top + turn = scratch; edge = pitch bend; SHIFT + turn = fast search. Record ticks per revolution (sum of `value-0x40` over one turn): ____ (code assumes 720).
- [ ] TEMPO fader: direction (+ at bottom), soft takeover after SYNC is released, SHIFT+SYNC cycles range.
- [ ] Channel faders, crossfader, TRIM, EQ, CFX: smooth 14-bit response (check MSB/LSB order in log).
- [ ] Pads: HOT CUE set/trigger, SHIFT+pad delete; BEAT JUMP; SHIFT+BEAT JUMP = BEAT LOOP.
- [ ] LOOP IN/OUT, 4 BEAT/EXIT, CUE/LOOP CALL ◀ ▶ (halve/double while looping, beat jump otherwise).
- [ ] BROWSE scroll/push, LOAD 1/2 (with playing-deck safeguard).

## LEDs
- [ ] PLAY blinks when paused, solid when playing; CUE blinks off-cue.
- [ ] Hot cue pads light when set; mode buttons show the active pad mode (exclusive?).
- [ ] SYNC, headphone CUE, loop LEDs; VU meters follow channel level.
- [ ] LEDs turn off when quitting deckforge.

## Keepalive
- [ ] With keepalive on (default), faders/knobs report positions right after connect. To compare without it, set `Flx4::keepalive = false` in `apps/desktop/src-tauri/src/lib.rs` (no UI toggle yet).

## Phase 3 FX control (2026-10-14 build, not yet tried on the FLX4)
Run with a track playing at low volume and the MIDI diagnostics log open. Expected results are what the code does; tick or note what the hardware really does.
- [ ] FX ON/OFF LED: dark before deckforge connects; with Beat FX off it should light steadily, with it on it should blink (official footnote, polarity unverified). If inverted, note it.
- [ ] PAD FX 1 (deck 1, then deck 2): the button lights at once and the pads light; pads 1-4 hold Echo / Ping Pong / Reverb / Roll on that deck only (the other deck stays dry); the held pad blinks.
- [ ] Hold pad 1 and press BEAT ▶ / ◀: the echo time changes (the FX display between the deck cards shows the pad's beats); turn LEVEL/DEPTH while holding: nothing until the knob reaches about 70 %, then the pad's depth follows.
- [ ] Pads 5-8: Echo Out, Backspin, Vinyl Brake, Roll Out on that deck, sound returns after the length.
- [ ] SHIFT + PAD FX 1 pads: Flanger, Phaser, Trans, Filter Sweep, Down Echo, Up Echo, Spiral, Delay. Release a pad after letting go of SHIFT: the effect stops. Do the shifted pad LEDs show while SHIFT is held?
- [ ] SHIFT + PAD FX 1 = PAD FX 2: pads set 1/8 ... 8 beats (Reverb: 10 ... 100 %) and switch the Beat FX on while held; SHIFT + pad picks Echo, Ping Pong, Reverb, Roll, Flanger, Phaser, Trans, Filter Sweep.
- [ ] Switch pad mode while holding a Pad FX pad: the effect stops. Back to HOT CUE: hot cues work as before.
- [ ] SHIFT + LEVEL/DEPTH: moves the effect's second parameter (Echo: feedback) after the knob catches up; the screen shows the parameter strip, ghost marker and arrow while it waits. Let go of SHIFT: LEVEL/DEPTH waits again until it meets the level.
- [ ] SHIFT + BEAT ▶ / ◀: pages the parameter (one page per press, not two).
- [ ] SHIFT + FX ON/OFF: the Beat FX stops and the Release FX chosen on screen plays once per press on the CH SELECT target; try all three types and CH 1 / CH 2 / 1&2.
- [ ] SMART CFX: LED lights; each CFX knob does nothing until it passes the centre (LED blinks meanwhile), then left/right play the macro. SHIFT + SMART CFX: next preset (the FX display names it), knobs re-arm.
- [ ] Do pad LEDs accept host blinking at ~4 Hz cleanly?
