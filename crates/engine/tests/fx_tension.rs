use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use deckforge_common::{
    beat_fx_params, Beatgrid, BeatFxKind, FxTarget, FxTrigger, TrackAudio, BEAT_FX_MAX_PARAMS, HOT_CUE_SLOTS,
};
use deckforge_engine::{create_engine, Command, EngineHandle, LoadRequest, MixerParam, RtEngine, Routing};

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const SR: u32 = 48_000;
const CH: usize = 4;
const BLOCK: usize = 64;
const GRID: Beatgrid = Beatgrid { bpm: 120.0, first_beat_secs: 0.0 };
const BAR_SECS: f64 = 2.0;

fn track_of(secs: f64, mut sample: impl FnMut(usize) -> f32) -> Arc<TrackAudio> {
    let n = (secs * SR as f64) as usize;
    Arc::new(TrackAudio::new(SR, (0..n).flat_map(|i| [sample(i); 2]).collect()))
}

fn tone(hz: f64, amp: f32, secs: f64) -> Arc<TrackAudio> {
    track_of(secs, |i| amp * (std::f64::consts::TAU * hz * i as f64 / SR as f64).sin() as f32)
}

fn white(amp: f32, secs: f64) -> Arc<TrackAudio> {
    let mut state = 0x1234_5678u32;
    track_of(secs, move |_| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        amp * ((state >> 8) as f32 / 8_388_608.0 - 1.0)
    })
}

fn rig(track: Arc<TrackAudio>, phrases: &[f64]) -> (EngineHandle, RtEngine) {
    let (h, mut rt) = create_engine(SR);
    rt.reconfigure(SR, Routing::for_channel_count(CH));
    let request = Box::new(LoadRequest { track, token: 1, grid: Some(GRID), cue_secs: Some(0.0), hot_cues: [None; HOT_CUE_SLOTS] });
    assert!(h.send(Command::Load { deck: 0, request }));
    if !phrases.is_empty() {
        assert!(h.send(Command::SetPhraseStarts { deck: 0, starts: phrases.into() }));
    }
    h.send(Command::Mixer { channel: 0, param: MixerParam::Fader(1.0) });
    h.send(Command::MasterLevel(0.8));
    h.send(Command::BeatFxTarget(FxTarget::A));
    h.send(Command::Play { deck: 0 });
    (h, rt)
}

struct Capture {
    left: Vec<f32>,
    positions: Vec<f64>,
    progress: Vec<Option<f32>>,
    held: Vec<bool>,
}

impl Capture {
    fn new() -> Self {
        Self { left: Vec::new(), positions: Vec::new(), progress: Vec::new(), held: Vec::new() }
    }

    fn frame_at_track_secs(&self, secs: f64) -> usize {
        self.positions.iter().position(|p| *p >= secs).map_or(self.left.len(), |block| block * BLOCK)
    }
}

fn advance(h: &EngineHandle, rt: &mut RtEngine, cap: &mut Capture, secs: f64) {
    let mut buf = vec![0.0f32; BLOCK * CH];
    for _ in 0..((secs * SR as f64) as usize / BLOCK) {
        rt.process(&mut buf, CH);
        cap.left.extend(buf.chunks(CH).map(|f| f[0]));
        let snap = h.snapshot();
        cap.positions.push(snap.decks[0].position_secs);
        cap.progress.push(snap.beat_fx.ascend_progress);
        cap.held.push(snap.beat_fx.held);
        h.collect_garbage();
    }
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn amplitude_at(x: &[f32], hz: f64) -> f64 {
    let w = std::f64::consts::TAU * hz / SR as f64;
    let (mut c, mut s) = (0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        c += *v as f64 * (w * i as f64).cos();
        s += *v as f64 * (w * i as f64).sin();
    }
    2.0 * (c * c + s * s).sqrt() / x.len() as f64
}

fn configure_ascend(h: &EngineHandle, length_choice: f32, land_on: f32) {
    h.send(Command::BeatFxKind(BeatFxKind::Ascend));
    h.send(Command::BeatFxLevel(0.5));
    h.send(Command::BeatFxParam { index: 0, value: length_choice });
    h.send(Command::BeatFxParam { index: 1, value: land_on });
}

fn first_progress_one(cap: &Capture) -> Option<usize> {
    cap.progress.iter().position(|p| *p == Some(1.0))
}

#[test]
fn ascend_ends_on_the_phrase_start_nearest_the_nominal_end_within_ten_milliseconds() {
    let (h, mut rt) = rig(tone(100.0, 0.4, 120.0), &[0.0, 18.0, 40.0]);
    configure_ascend(&h, 1.0, 0.0);
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 1.0);
    h.send(Command::BeatFxEnabled(true));
    advance(&h, &mut rt, &mut cap, 20.0);
    let block = first_progress_one(&cap).expect("the ramp completes");
    let error_ms = (cap.positions[block] - 18.0) * 1_000.0;
    eprintln!("ascend with phrases: ramp end at {:.3} s track time ({error_ms:+.2} ms from the 18 s phrase)", cap.positions[block]);
    assert!(error_ms.abs() <= 10.0, "{error_ms} ms");
    assert!(cap.held.last().copied().unwrap_or(false), "holds the peak");
    let snap = h.snapshot().beat_fx;
    assert_eq!(snap.ascend_landing_bar, Some(10));
    assert!((snap.ascend_bars.unwrap() - 8.5).abs() < 0.1, "{:?}", snap.ascend_bars);
}

#[test]
fn ascend_falls_back_to_four_bar_lines_from_the_grid_when_there_are_no_phrases() {
    let (h, mut rt) = rig(tone(100.0, 0.4, 120.0), &[]);
    configure_ascend(&h, 1.0, 0.0);
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 1.0);
    h.send(Command::BeatFxEnabled(true));
    advance(&h, &mut rt, &mut cap, 20.0);
    let block = first_progress_one(&cap).expect("the ramp completes");
    let error_ms = (cap.positions[block] - 16.0) * 1_000.0;
    eprintln!("ascend without phrases: ramp end {error_ms:+.2} ms from the 16 s four-bar line");
    assert!(error_ms.abs() <= 10.0, "{error_ms} ms");
    assert_eq!(h.snapshot().beat_fx.ascend_landing_bar, Some(9));
}

#[test]
fn ascend_can_land_on_the_nearest_bar_line_instead() {
    let (h, mut rt) = rig(tone(100.0, 0.4, 120.0), &[0.0, 18.0, 40.0]);
    configure_ascend(&h, 1.0, 1.0);
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 1.7);
    h.send(Command::BeatFxEnabled(true));
    advance(&h, &mut rt, &mut cap, 20.0);
    let block = first_progress_one(&cap).expect("the ramp completes");
    let landing = cap.positions[block];
    eprintln!("bar landing at {landing:.3} s");
    assert!(((landing / BAR_SECS) - (landing / BAR_SECS).round()).abs() * BAR_SECS < 0.01);
    assert!((landing - 17.7).abs() <= 1.01, "nominal end 17.7 s");
}

#[test]
fn ascend_release_cuts_hard_on_the_next_downbeat_of_the_phrase_anchored_bars() {
    let (h, mut rt) = rig(tone(100.0, 0.4, 120.0), &[0.0, 18.0, 40.0]);
    configure_ascend(&h, 1.0, 0.0);
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 0.9);
    let dry = amplitude_at(&cap.left[cap.frame_at_track_secs(0.3)..cap.frame_at_track_secs(0.7)], 100.0);
    h.send(Command::BeatFxEnabled(true));
    advance(&h, &mut rt, &mut cap, 19.8);
    assert!(cap.held.last().copied().unwrap_or(false));
    let thin = amplitude_at(&cap.left[cap.frame_at_track_secs(19.2)..cap.frame_at_track_secs(19.6)], 100.0);
    h.send(Command::BeatFxTrigger(FxTrigger::Release));
    advance(&h, &mut rt, &mut cap, 4.0);
    let downbeat = cap.frame_at_track_secs(22.0);
    let before = amplitude_at(&cap.left[downbeat - 2_400..downbeat - 240], 100.0);
    let after = amplitude_at(&cap.left[downbeat + 2_400..downbeat + 4_800], 100.0);
    eprintln!("100 Hz: dry {dry:.3}, held {thin:.3}, just before the downbeat {before:.3}, just after {after:.3}");
    assert!(thin < 0.1 * dry && before < 0.1 * dry, "thinned to the end");
    assert!(after > 0.7 * dry, "the full range is back on the downbeat");
    assert!(!h.snapshot().beat_fx.held && h.snapshot().beat_fx.ascend_progress.is_none());
}

#[test]
fn ascend_release_now_does_not_wait_for_the_downbeat() {
    let (h, mut rt) = rig(tone(100.0, 0.4, 120.0), &[0.0, 18.0, 40.0]);
    configure_ascend(&h, 0.0, 0.0);
    h.send(Command::BeatFxParam { index: 2, value: 1.0 });
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 0.9);
    let dry = amplitude_at(&cap.left[cap.frame_at_track_secs(0.3)..cap.frame_at_track_secs(0.7)], 100.0);
    h.send(Command::BeatFxEnabled(true));
    advance(&h, &mut rt, &mut cap, 9.5);
    assert!(cap.held.last().copied().unwrap_or(false));
    let released_at = cap.positions.len() * BLOCK;
    h.send(Command::BeatFxTrigger(FxTrigger::Release));
    advance(&h, &mut rt, &mut cap, 0.5);
    let after = amplitude_at(&cap.left[released_at + 4_800..released_at + 7_200], 100.0);
    assert!(after > 0.7 * dry, "immediately back: {after} vs {dry}");
}

fn envelope_peak(x: &[f32], window: usize) -> usize {
    let mut best = (0.0f32, 0usize);
    let mut at = 0;
    while at + window <= x.len() {
        let e = rms(&x[at..at + window]);
        if e > best.0 {
            best = (e, at + window);
        }
        at += window / 4;
    }
    best.1
}

fn backwash_error_ms(tempo: f64, key_lock: bool, target: FxTarget) -> f64 {
    let (h, mut rt) = rig(white(0.2, 120.0), &[]);
    h.send(Command::BeatFxTarget(target));
    h.send(Command::SetTempo { deck: 0, ratio: tempo });
    h.send(Command::SetKeyLock { deck: 0, enabled: key_lock });
    h.send(Command::BeatFxKind(BeatFxKind::Backwash));
    h.send(Command::BeatFxBeats(2.0));
    h.send(Command::BeatFxLevel(1.0));
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 3.4);
    let mut reference = Capture::new();
    let (h0, mut rt0) = rig(white(0.2, 120.0), &[]);
    h0.send(Command::SetTempo { deck: 0, ratio: tempo });
    h0.send(Command::SetKeyLock { deck: 0, enabled: key_lock });
    advance(&h0, &mut rt0, &mut reference, 3.4);
    let before = cap.left.len();
    h.send(Command::BeatFxEnabled(true));
    advance(&h, &mut rt, &mut cap, 4.0);
    advance(&h0, &mut rt0, &mut reference, 4.0);
    let landing_secs = (1.0 + (cap.positions[before / BLOCK] / BAR_SECS).floor()) * BAR_SECS;
    let landing = cap.frame_at_track_secs(landing_secs);
    let swell: Vec<f32> = cap.left.iter().zip(&reference.left).map(|(a, b)| a - b).collect();
    let window = &swell[before..landing + 4_800];
    let silent_before = rms(&swell[..before]);
    assert!(silent_before < 1e-6, "nothing happens until it is switched on: {silent_before}");
    let at = before + envelope_peak(window, 960);
    (at as f64 - landing as f64) / 48.0
}

#[test]
fn backwash_peaks_within_ten_milliseconds_of_the_next_downbeat() {
    for (tempo, key_lock, target) in [(1.0, false, FxTarget::A), (1.06, false, FxTarget::A), (0.94, true, FxTarget::A), (1.0, false, FxTarget::Master)] {
        let error = backwash_error_ms(tempo, key_lock, target);
        eprintln!("backwash tempo {tempo} key lock {key_lock} {target:?}: envelope peak {error:+.2} ms from the downbeat");
        assert!(error.abs() <= 10.0, "tempo {tempo} key lock {key_lock} {target:?}: {error} ms");
    }
}

#[test]
fn rendering_and_playing_a_backwash_swell_never_allocates_on_the_audio_thread() {
    let (h, mut rt) = rig(white(0.2, 120.0), &[0.0, 16.0]);
    h.send(Command::BeatFxKind(BeatFxKind::Backwash));
    h.send(Command::BeatFxBeats(4.0));
    h.send(Command::BeatFxLevel(1.0));
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 1.0);
    h.send(Command::BeatFxEnabled(true));
    let mut buf = vec![0.0f32; BLOCK * CH];
    let before = ALLOCATIONS.with(Cell::get);
    for _ in 0..(8 * SR as usize / BLOCK) {
        rt.process(&mut buf, CH);
    }
    let allocations = ALLOCATIONS.with(Cell::get) - before;
    assert_eq!(allocations, 0, "no heap allocation while the swell renders and plays");
}

#[test]
fn ascend_and_the_other_effects_never_allocate_on_the_audio_thread_either() {
    let (h, mut rt) = rig(white(0.2, 120.0), &[0.0, 16.0]);
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 0.5);
    let mut buf = vec![0.0f32; BLOCK * CH];
    for kind in [BeatFxKind::PulseVerb, BeatFxKind::DubTape, BeatFxKind::Halo, BeatFxKind::Ascend, BeatFxKind::Backwash] {
        h.send(Command::BeatFxKind(kind));
        h.send(Command::BeatFxLevel(1.0));
        h.send(Command::BeatFxEnabled(true));
        rt.process(&mut buf, CH);
        let before = ALLOCATIONS.with(Cell::get);
        for b in 0..(3 * SR as usize / BLOCK) {
            if b == 40 {
                h.send(Command::BeatFxTrigger(FxTrigger::FreezeOn));
                h.send(Command::BeatFxParam { index: 0, value: 1.0 });
            }
            rt.process(&mut buf, CH);
        }
        let allocations = ALLOCATIONS.with(Cell::get) - before;
        h.send(Command::BeatFxEnabled(false));
        for _ in 0..(SR as usize / BLOCK) {
            rt.process(&mut buf, CH);
        }
        assert_eq!(allocations, 0, "{kind:?}");
    }
}

#[test]
fn sub_parameters_and_triggers_reach_the_snapshot_and_survive_kind_changes_and_a_rate_rebuild() {
    let (h, mut rt) = rig(white(0.1, 30.0), &[]);
    let mut cap = Capture::new();
    h.send(Command::BeatFxKind(BeatFxKind::DubTape));
    h.send(Command::BeatFxParam { index: 0, value: 88.0 });
    h.send(Command::BeatFxParam { index: 4, value: 2.0 });
    advance(&h, &mut rt, &mut cap, 0.1);
    let snap = h.snapshot().beat_fx;
    assert_eq!((snap.kind, snap.params[0], snap.params[4]), (BeatFxKind::DubTape, 88.0, 2.0));
    assert_eq!(snap.params.len(), BEAT_FX_MAX_PARAMS);
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    advance(&h, &mut rt, &mut cap, 0.1);
    assert_eq!(&h.snapshot().beat_fx.params[..2], &[42.0, 50.0], "another kind has its own values");
    h.send(Command::BeatFxKind(BeatFxKind::DubTape));
    advance(&h, &mut rt, &mut cap, 0.1);
    assert_eq!(h.snapshot().beat_fx.params[0], 88.0);
    h.send(Command::BeatFxParam { index: 0, value: 500.0 });
    h.send(Command::BeatFxParam { index: 1, value: f32::NAN });
    advance(&h, &mut rt, &mut cap, 0.1);
    let params = h.snapshot().beat_fx.params;
    assert_eq!((params[0], params[1]), (105.0, 50.0), "clamped and garbage ignored");

    h.send(Command::BeatFxEnabled(true));
    h.send(Command::BeatFxTrigger(FxTrigger::FreezeOn));
    advance(&h, &mut rt, &mut cap, 0.1);
    assert!(h.snapshot().beat_fx.held, "freeze shows as held");
    h.send(Command::BeatFxTrigger(FxTrigger::FreezeOff));
    advance(&h, &mut rt, &mut cap, 0.1);
    assert!(!h.snapshot().beat_fx.held);

    rt.reconfigure(44_100, Routing::for_channel_count(CH));
    advance(&h, &mut rt, &mut cap, 0.1);
    let params = h.snapshot().beat_fx.params;
    assert_eq!((params[0], params[4]), (105.0, 2.0), "survives a sample rate rebuild");
    assert_eq!(beat_fx_params(BeatFxKind::DubTape).len(), 7);
}

#[test]
fn beats_snap_to_the_options_of_the_selected_kind() {
    let (h, mut rt) = rig(white(0.1, 30.0), &[]);
    let mut cap = Capture::new();
    h.send(Command::BeatFxKind(BeatFxKind::Backwash));
    h.send(Command::BeatFxBeats(0.5));
    advance(&h, &mut rt, &mut cap, 0.1);
    assert_eq!(h.snapshot().beat_fx.beats, 1.0, "Backwash offers 1, 2 and 4 beats");
    h.send(Command::BeatFxBeats(16.0));
    advance(&h, &mut rt, &mut cap, 0.1);
    assert_eq!(h.snapshot().beat_fx.beats, 4.0);
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    h.send(Command::BeatFxBeats(16.0));
    advance(&h, &mut rt, &mut cap, 0.1);
    assert_eq!(h.snapshot().beat_fx.beats, 16.0);
    h.send(Command::BeatFxKind(BeatFxKind::PulseVerb));
    advance(&h, &mut rt, &mut cap, 0.1);
    assert_eq!(h.snapshot().beat_fx.beats, 4.0, "switching kinds folds the division into the new kind's options");
}

#[test]
fn phrase_starts_are_swapped_in_without_touching_the_audio_thread_allocator_and_old_ones_are_collected() {
    let (h, mut rt) = rig(white(0.1, 30.0), &[0.0, 8.0]);
    let mut buf = vec![0.0f32; BLOCK * CH];
    rt.process(&mut buf, CH);
    let before = ALLOCATIONS.with(Cell::get);
    h.send(Command::SetPhraseStarts { deck: 0, starts: vec![0.0, 4.0, 12.0].into_boxed_slice() });
    let marker = ALLOCATIONS.with(Cell::get);
    rt.process(&mut buf, CH);
    assert_eq!(ALLOCATIONS.with(Cell::get), marker, "swapping them in allocates nothing");
    assert!(marker > before, "the box was allocated by the sender");
    assert!(h.collect_garbage() >= 1, "the old starts come back for the control thread to drop");
}

#[test]
fn a_new_track_drops_the_previous_phrase_anchors() {
    let (h, mut rt) = rig(white(0.1, 30.0), &[0.0, 8.0, 16.0]);
    configure_ascend(&h, 0.0, 0.0);
    let request = Box::new(LoadRequest { track: white(0.1, 30.0), token: 2, grid: Some(GRID), cue_secs: Some(0.0), hot_cues: [None; HOT_CUE_SLOTS] });
    h.send(Command::Load { deck: 0, request });
    h.send(Command::Play { deck: 0 });
    let mut cap = Capture::new();
    advance(&h, &mut rt, &mut cap, 0.5);
    h.send(Command::BeatFxEnabled(true));
    advance(&h, &mut rt, &mut cap, 9.0);
    let block = first_progress_one(&cap).expect("the ramp completes");
    let landing = cap.positions[block];
    assert!((landing - 8.0).abs() < 0.02, "four bars from 0.5 s on the fresh grid's own 4-bar line (8 s), not an old phrase: {landing}");
}
