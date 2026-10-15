# DDJ-400 hardware verification

The controller is connected to the development Mac. The official AlphaTheta MIDI list and the live CoreMIDI port names were read before implementing this mapping.

## Evidence collected

- CoreMIDI input port: `DDJ-400`.
- CoreMIDI output port: `DDJ-400`.
- CoreAudio output: `DDJ-400 Audio Out`, four output channels at 44.1 kHz.
- Live input included deck 1 pad `97 00 7F`, jog touch `90 36 7F`, jog motion around `B0 22 3F/41`, and high-resolution fader/tempo messages using CC pairs.
- Deckforge uses the published channel map: deck controls on channels 1/2, FX on 5, global mixer/browser on 7, pads on 8–11.
- The rebuilt app's diagnostics reported controller connected, input `DDJ-400`, output `DDJ-400`, and live MIDI traffic (986 input / 80 output messages at readback).
- The rebuilt app's audio status was `Running` on `DDJ-400 Audio Out` at 44100 Hz, four channels, master-and-phones routing (master 1/2, phones 3/4), with a 256-frame callback, zero underruns/overruns, and zero stream errors.
- `Headphone mix handled by controller hardware` was enabled. Preference persistence and hotplug fallback behavior are covered by desktop audio tests.

## Checklist

### Controller connection and MIDI mapping

- [x] Match the observed CoreMIDI port label `DDJ-400`.
- [x] Confirm representative live pad, jog, tempo and fader messages.
- [x] In Deckforge, confirm the toolbar and MIDI diagnostics report input and output `DDJ-400`.
- [ ] Confirm both decks' transport, cue, sync, tempo, mixer faders, trim, EQ and headphone cue controls.
- [ ] Confirm jog touch scratches, wheel-side motion bends, platter motion bends when untouched, and SHIFT + jog fast-searches.
- [ ] Confirm loop-in/out, 4-beat loop, reloop/exit, loop scaling, cue/loop call and SHIFT actions.
- [ ] Confirm all eight pad modes, both deck pad banks, and shifted pad actions.
- [ ] Confirm Beat FX select, channel target selector, beat controls, SHIFT parameter/page actions, LEVEL/DEPTH and Release FX.
- [ ] Confirm browser scroll/push, LOAD and SHIFT+LOAD eject actions.

### LED feedback

- [ ] PLAY/CUE/SYNC/loop/channel-cue LEDs follow engine state.
- [ ] Mode buttons show the active pad mode; hot-cue and FX pads reflect state.
- [ ] Loaded-deck illumination updates on the DDJ-400.
- [ ] Beat FX ON/OFF LED behavior matches the official NOTE ON flashing / NOTE OFF steady behavior.
- [ ] LEDs clear or resynchronize after controller disconnect/reconnect.

### Audio and headphones

- [x] Identify `DDJ-400 Audio Out` with four output channels at 44.1 kHz.
- [x] Confirm Deckforge selects it automatically while connected and routes Master to outputs 1/2 and Phones to 3/4.
- [ ] With music at low volume, confirm Master is heard on the speaker outputs and PFL cue is heard on headphones.
- [ ] Confirm physical headphone MIX and LEVEL knobs control the DDJ-400 headphone circuit when hardware headphone handling is enabled.
- [x] Confirm saved headphone preference behavior in tests; runtime defaults hardware handling on for DDJ-400 when no preference is saved.
- [ ] Unplug audio and confirm Deckforge restores the saved output or system default; reconnect and confirm DDJ-400 selection returns.

## References

- [AlphaTheta DDJ-400 MIDI message list](https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-400/DDJ-400_MIDI_Message_List_E1.pdf)
- [DDJ-400 Mac setup guide](https://forums.pioneerdj.com/hc/en-us/articles/360029307091-DDJ-400-Troubleshooting-Mac)
- [DDJ-400 MIDI mapping evidence](research/ddj-400-midi.md)
